use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::Engine as _;
use http_body_util::BodyExt;
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;

use ed25519_dalek::{Signer, SigningKey};
use stellar_strkey::ed25519::PublicKey;
use verdant_backend::farmers::chain::StubChain;
use verdant_backend::{AppState, app, connect, migrate};

fn keypair(n: u8) -> (SigningKey, String) {
    let payload = [n; 32];
    let signing = SigningKey::from_bytes(&payload);
    let public = PublicKey::from_payload(signing.verifying_key().as_bytes())
        .unwrap()
        .to_string()
        .to_string();
    (signing, public)
}

fn test_address(n: u8) -> String {
    keypair(n).1
}

fn test_id(n: u8) -> String {
    format!("va:farmer:{}", test_address(n))
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
    sqlx::query("TRUNCATE auth_challenges, auth_sessions, farmers, documents")
        .execute(&pool)
        .await
        .expect("clean auth and farmers tables");
    let chain = Arc::new(StubChain::new());
    AppState::new(pool, chain)
}

/// Establishes a session for keypair `n` and returns a bearer token.
async fn establish_session(state: &AppState, n: u8) -> String {
    let (signing, addr) = keypair(n);

    let challenge = app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/challenge")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "address": addr }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(challenge.status(), StatusCode::OK);
    let challenge = body_to_json(challenge).await;

    let domain = challenge["domain"].as_str().unwrap();
    let nonce = challenge["nonce"].as_str().unwrap();
    let timestamp = challenge["timestamp"].as_str().unwrap();
    let signature = base64::engine::general_purpose::STANDARD.encode(
        signing
            .try_sign(sep40_message(domain, &addr, nonce, timestamp).as_bytes())
            .unwrap()
            .to_bytes(),
    );

    let verify = app(state.clone())
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
    assert_eq!(verify.status(), StatusCode::OK);
    let verify = body_to_json(verify).await;
    verify["token"].as_str().unwrap().to_string()
}

fn add_bearer(builder: axum::http::request::Builder, token: &str) -> axum::http::request::Builder {
    builder.header("authorization", format!("Bearer {token}"))
}

#[tokio::test]
async fn register_farmer_returns_201_with_va_id() {
    let state = test_state().await;
    let token = establish_session(&state, 1).await;
    let addr = test_address(1);
    let id = test_id(1);
    let body = json!({
        "address": addr,
        "metadata": { "name": "Ada Farm Cooperative", "region": "Niger" }
    });
    let response = app(state)
        .oneshot(
            add_bearer(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/farmers/register")
                    .header("content-type", "application/json"),
                &token,
            )
            .body(Body::from(body.to_string()))
            .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let json = body_to_json(response).await;
    assert_eq!(json["id"], id);
    assert_eq!(json["address"], addr);
    assert!(json["registered"].as_bool().unwrap());
    assert!(json["createdLedger"].as_i64().is_some());
    assert_eq!(json["metadata"]["name"], "Ada Farm Cooperative");
    assert_eq!(json["metadata"]["region"], "Niger");
}

#[tokio::test]
async fn register_duplicate_returns_409() {
    let state = test_state().await;
    let token = establish_session(&state, 2).await;
    let addr = test_address(2);
    let body = json!({
        "address": addr,
        "metadata": { "name": "Ada Farm Cooperative" }
    });
    let req = |token: &str| {
        add_bearer(
            Request::builder()
                .method("POST")
                .uri("/api/v1/farmers/register")
                .header("content-type", "application/json"),
            token,
        )
        .body(Body::from(body.to_string()))
        .unwrap()
    };
    app(state.clone()).oneshot(req(&token)).await.unwrap();
    let response = app(state).oneshot(req(&token)).await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let json = body_to_json(response).await;
    assert_eq!(json["error"], "farmer already registered");
}

#[tokio::test]
async fn get_farmer_returns_200() {
    let state = test_state().await;
    let token = establish_session(&state, 3).await;
    let addr = test_address(3);
    let id = test_id(3);
    let body = json!({
        "address": addr,
        "metadata": { "name": "Ada Farm Cooperative" }
    });
    app(state.clone())
        .oneshot(
            add_bearer(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/farmers/register")
                    .header("content-type", "application/json"),
                &token,
            )
            .body(Body::from(body.to_string()))
            .unwrap(),
        )
        .await
        .unwrap();

    let response = app(state)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/farmers/{addr}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_to_json(response).await;
    assert_eq!(json["id"], id);
}

#[tokio::test]
async fn get_farmer_with_presentation_form() {
    let state = test_state().await;
    let token = establish_session(&state, 4).await;
    let addr = test_address(4);
    let id = test_id(4);
    let body = json!({
        "address": addr,
        "metadata": { "name": "Ada Farm Cooperative" }
    });
    app(state.clone())
        .oneshot(
            add_bearer(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/farmers/register")
                    .header("content-type", "application/json"),
                &token,
            )
            .body(Body::from(body.to_string()))
            .unwrap(),
        )
        .await
        .unwrap();

    let response = app(state)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/farmers/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_to_json(response).await;
    assert_eq!(json["id"], id);
}

#[tokio::test]
async fn get_unknown_farmer_returns_404() {
    let state = test_state().await;
    let response = app(state)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/v1/farmers/{}", test_address(99)))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn update_metadata_returns_200_with_new_hash() {
    let state = test_state().await;
    let token = establish_session(&state, 5).await;
    let addr = test_address(5);
    let body = json!({
        "address": addr,
        "metadata": { "name": "Ada Farm Cooperative", "region": "Niger" }
    });
    app(state.clone())
        .oneshot(
            add_bearer(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/farmers/register")
                    .header("content-type", "application/json"),
                &token,
            )
            .body(Body::from(body.to_string()))
            .unwrap(),
        )
        .await
        .unwrap();

    let update_body = json!({
        "metadata": { "name": "Ada Farm Cooperative", "region": "Zinder", "district": "Mirriah" }
    });
    let response = app(state)
        .oneshot(
            add_bearer(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/api/v1/farmers/{addr}/metadata"))
                    .header("content-type", "application/json"),
                &token,
            )
            .body(Body::from(update_body.to_string()))
            .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_to_json(response).await;
    assert_eq!(json["metadata"]["region"], "Zinder");
    assert_eq!(json["metadata"]["district"], "Mirriah");
}

#[tokio::test]
async fn update_metadata_wrong_actor_returns_401() {
    let state = test_state().await;
    let token = establish_session(&state, 6).await;
    let addr = test_address(6);
    let body = json!({
        "address": addr,
        "metadata": { "name": "Ada Farm Cooperative" }
    });
    app(state.clone())
        .oneshot(
            add_bearer(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/farmers/register")
                    .header("content-type", "application/json"),
                &token,
            )
            .body(Body::from(body.to_string()))
            .unwrap(),
        )
        .await
        .unwrap();

    // A session for a *different* address cannot update this farmer.
    let other_token = establish_session(&state, 60).await;
    let update_body = json!({ "metadata": { "name": "Hacker" } });
    let response = app(state)
        .oneshot(
            add_bearer(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/api/v1/farmers/{addr}/metadata"))
                    .header("content-type", "application/json"),
                &other_token,
            )
            .body(Body::from(update_body.to_string()))
            .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn register_missing_bearer_returns_401() {
    let state = test_state().await;
    let addr = test_address(7);
    let body = json!({
        "address": addr,
        "metadata": { "name": "Ada Farm Cooperative" }
    });
    let response = app(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/farmers/register")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn register_invalid_address_returns_400() {
    let state = test_state().await;
    let token = establish_session(&state, 8).await;
    let body = json!({
        "address": "not-a-valid-stellar-key",
        "metadata": { "name": "Ada Farm Cooperative" }
    });
    let response = app(state)
        .oneshot(
            add_bearer(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/farmers/register")
                    .header("content-type", "application/json"),
                &token,
            )
            .body(Body::from(body.to_string()))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn register_empty_name_returns_400() {
    let state = test_state().await;
    let token = establish_session(&state, 9).await;
    let addr = test_address(9);
    let body = json!({
        "address": addr,
        "metadata": { "name": "" }
    });
    let response = app(state)
        .oneshot(
            add_bearer(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/farmers/register")
                    .header("content-type", "application/json"),
                &token,
            )
            .body(Body::from(body.to_string()))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn register_provided_metadata_hash_mismatch_returns_400() {
    let state = test_state().await;
    let token = establish_session(&state, 10).await;
    let addr = test_address(10);
    let body = json!({
        "address": addr,
        "metadata": { "name": "Ada Farm Cooperative" },
        "metadataHash": "deadbeef"
    });
    let response = app(state)
        .oneshot(
            add_bearer(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/farmers/register")
                    .header("content-type", "application/json"),
                &token,
            )
            .body(Body::from(body.to_string()))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let json = body_to_json(response).await;
    assert!(json["error"].as_str().unwrap().contains("metadataHash"));
}

async fn body_to_json(response: axum::http::Response<axum::body::Body>) -> serde_json::Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

async fn register_farmer(state: AppState, n: u8, name: &str, region: &str) {
    let token = establish_session(&state, n).await;
    let addr = test_address(n);
    let body = json!({
        "address": addr,
        "metadata": { "name": name, "region": region }
    });
    let response = app(state)
        .oneshot(
            add_bearer(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/farmers/register")
                    .header("content-type", "application/json"),
                &token,
            )
            .body(Body::from(body.to_string()))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn search_farmers_by_name_substring() {
    let state = test_state().await;
    register_farmer(state.clone(), 11, "Sahel Grain Cooperative", "Tillabéri").await;
    register_farmer(state.clone(), 12, "Zinder Dairy Union", "Zinder").await;

    let response = app(state)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/farmers?q=dairy")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_to_json(response).await;
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["name"], "Zinder Dairy Union");
    assert_eq!(items[0]["address"], test_address(12));
    assert_eq!(items[0]["id"], test_id(12));
    assert_eq!(items[0]["region"], "Zinder");
    assert_eq!(json["pagination"]["total"], 1);
    assert_eq!(json["pagination"]["totalPages"], 1);
}

#[tokio::test]
async fn search_farmers_by_region_case_insensitive() {
    let state = test_state().await;
    register_farmer(state.clone(), 13, "Sahel Grain Cooperative", "Tillabéri").await;
    register_farmer(state.clone(), 14, "Zinder Dairy Union", "zinder").await;

    let response = app(state)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/farmers?q=ZINDER")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_to_json(response).await;
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["name"], "Zinder Dairy Union");
}

#[tokio::test]
async fn search_farmers_no_match_returns_empty() {
    let state = test_state().await;
    register_farmer(state.clone(), 15, "Sahel Grain Cooperative", "Tillabéri").await;

    let response = app(state)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/farmers?q=nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_to_json(response).await;
    assert_eq!(json["items"].as_array().unwrap().len(), 0);
    assert_eq!(json["pagination"]["total"], 0);
    assert_eq!(json["pagination"]["totalPages"], 1);
}

#[tokio::test]
async fn search_farmers_empty_q_returns_all() {
    let state = test_state().await;
    register_farmer(state.clone(), 16, "Sahel Grain Cooperative", "Tillabéri").await;
    register_farmer(state.clone(), 17, "Zinder Dairy Union", "Zinder").await;

    let response = app(state)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/farmers")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_to_json(response).await;
    assert_eq!(json["items"].as_array().unwrap().len(), 2);
    assert_eq!(json["pagination"]["total"], 2);
    assert_eq!(json["pagination"]["pageSize"], 20);
}

#[tokio::test]
async fn search_farmers_pagination_clamps_page_size() {
    let state = test_state().await;
    register_farmer(state.clone(), 18, "Sahel Grain Cooperative", "Tillabéri").await;
    register_farmer(state.clone(), 19, "Zinder Dairy Union", "Zinder").await;

    let response = app(state)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/farmers?q=&page=1&pageSize=500")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_to_json(response).await;
    assert_eq!(json["pagination"]["pageSize"], 100);
    assert_eq!(json["pagination"]["totalPages"], 1);
    assert_eq!(json["items"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn search_farmers_pages_results() {
    let state = test_state().await;
    for n in 21..25u8 {
        register_farmer(state.clone(), n, &format!("Farmer {n}"), "Niger").await;
    }

    let response = app(state)
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/farmers?page=1&pageSize=2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_to_json(response).await;
    assert_eq!(json["items"].as_array().unwrap().len(), 2);
    assert_eq!(json["pagination"]["total"], 4);
    assert_eq!(json["pagination"]["totalPages"], 2);
}
