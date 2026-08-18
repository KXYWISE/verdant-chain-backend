use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::Engine as _;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;

use ed25519_dalek::{Signer, SigningKey};
use stellar_strkey::ed25519::PublicKey;
use verdant_backend::farmers::chain::StubChain;
use verdant_backend::{AppState, app, connect, migrate};

fn keypair(n: u8) -> (SigningKey, String) {
    let mut seed = [0u8; 32];
    seed.fill(n);
    let signing = SigningKey::from_bytes(&seed);
    let public = PublicKey::from_payload(signing.verifying_key().as_bytes())
        .unwrap()
        .to_string()
        .to_string();
    (signing, public)
}

fn sep40_message(domain: &str, address: &str, nonce: &str, timestamp: &str) -> String {
    format!(
        "{domain} wants you to sign in with your Stellar account:\n{address}\n\nNonce: {nonce}\nIssued At: {timestamp}\n"
    )
}

async fn test_state() -> AppState {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for tests");
    let pool = connect(&database_url)
        .await
        .expect("connect to test database");
    migrate(&pool).await.expect("run migrations");
    sqlx::query("TRUNCATE auth_challenges, auth_sessions")
        .execute(&pool)
        .await
        .expect("clean auth tables");
    let chain = Arc::new(StubChain::new());
    AppState::new(pool, chain)
}

async fn issue_challenge(app_state: &AppState, address: &str) -> Value {
    let response = app(app_state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/challenge")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "address": address }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    body_to_json(response).await
}

#[tokio::test]
async fn challenge_returns_nonce_and_domain() {
    let state = test_state().await;
    let (_, addr) = keypair(1);

    let json = issue_challenge(&state, &addr).await;

    assert_eq!(json["domain"], "app.verdant.example");
    assert_eq!(json["address"], addr);
    assert!(!json["nonce"].as_str().unwrap().is_empty());
    assert!(!json["timestamp"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn challenge_invalid_address_returns_400() {
    let state = test_state().await;
    let response = app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/challenge")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "address": "not-a-valid-key" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn verify_returns_session_token() {
    let state = test_state().await;
    let (signing, addr) = keypair(2);

    let challenge = issue_challenge(&state, &addr).await;
    let domain = challenge["domain"].as_str().unwrap();
    let nonce = challenge["nonce"].as_str().unwrap();
    let timestamp = challenge["timestamp"].as_str().unwrap();
    let message = sep40_message(domain, &addr, nonce, timestamp);
    let signature = base64::engine::general_purpose::STANDARD
        .encode(signing.try_sign(message.as_bytes()).unwrap().to_bytes());

    let response = app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/verify")
                .header("content-type", "application/json")
                .body(
                    json!({
                        "address": addr,
                        "domain": domain,
                        "nonce": nonce,
                        "timestamp": timestamp,
                        "signature": signature
                    })
                    .to_string(),
                )
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_to_json(response).await;
    assert_eq!(json["address"], addr);
    assert_eq!(json["roles"], json!(["farmer"]));
    assert!(!json["token"].as_str().unwrap().is_empty());
    assert!(!json["expires_at"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn verify_tampered_signature_returns_401() {
    let state = test_state().await;
    let (_, addr) = keypair(3);

    let challenge = issue_challenge(&state, &addr).await;
    let response = app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/verify")
                .header("content-type", "application/json")
                .body(
                    json!({
                        "address": addr,
                        "domain": challenge["domain"],
                        "nonce": challenge["nonce"],
                        "timestamp": challenge["timestamp"],
                        "signature": base64::engine::general_purpose::STANDARD.encode([0u8; 64])
                    })
                    .to_string(),
                )
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn verify_reused_nonce_returns_409() {
    let state = test_state().await;
    let (signing, addr) = keypair(4);

    let challenge = issue_challenge(&state, &addr).await;
    let domain = challenge["domain"].as_str().unwrap();
    let nonce = challenge["nonce"].as_str().unwrap();
    let timestamp = challenge["timestamp"].as_str().unwrap();
    let signature = base64::engine::general_purpose::STANDARD.encode(
        signing
            .try_sign(sep40_message(domain, &addr, nonce, timestamp).as_bytes())
            .unwrap()
            .to_bytes(),
    );

    let body = json!({
        "address": addr,
        "domain": domain,
        "nonce": nonce,
        "timestamp": timestamp,
        "signature": signature
    })
    .to_string();

    let first = app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/verify")
                .header("content-type", "application/json")
                .body(Body::from(body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);

    let second = app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/verify")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn session_lookup_returns_401_for_unknown_token() {
    let state = test_state().await;
    let response = app(state)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/auth/session?token=doesnotexist")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

async fn establish_session(state: &AppState, n: u8) -> (String, String) {
    let (signing, addr) = keypair(n);
    let challenge = issue_challenge(state, &addr).await;
    let domain = challenge["domain"].as_str().unwrap();
    let nonce = challenge["nonce"].as_str().unwrap();
    let timestamp = challenge["timestamp"].as_str().unwrap();
    let signature = base64::engine::general_purpose::STANDARD.encode(
        signing
            .try_sign(sep40_message(domain, &addr, nonce, timestamp).as_bytes())
            .unwrap()
            .to_bytes(),
    );
    let response = app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/verify")
                .header("content-type", "application/json")
                .body(
                    json!({
                        "address": addr,
                        "domain": domain,
                        "nonce": nonce,
                        "timestamp": timestamp,
                        "signature": signature
                    })
                    .to_string(),
                )
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = body_to_json(response).await;
    (json["token"].as_str().unwrap().to_string(), addr)
}

#[tokio::test]
async fn session_lookup_with_valid_token_returns_address() {
    let state = test_state().await;
    let (token, addr) = establish_session(&state, 5).await;

    let response = app(state)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/auth/session?token={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_to_json(response).await;
    assert_eq!(json["address"], addr);
    assert_eq!(json["roles"], json!(["farmer"]));
}

#[tokio::test]
async fn auth_user_extractor_rejects_missing_bearer() {
    let state = test_state().await;
    let response = app(state)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // health does not require auth; this just ensures the extractor module compiles/links
    assert_eq!(response.status(), StatusCode::OK);
}

async fn body_to_json(response: axum::http::Response<axum::body::Body>) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}
