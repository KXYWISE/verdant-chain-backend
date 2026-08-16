use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;

use stellar_strkey::ed25519::PublicKey;
use verdant_backend::farmers::chain::StubChain;
use verdant_backend::{AppState, app, connect, migrate};

fn test_address(n: u8) -> String {
    let payload = [n; 32];
    PublicKey::from_payload(&payload)
        .unwrap()
        .to_string()
        .to_string()
}

fn test_id(n: u8) -> String {
    format!("va:farmer:{}", test_address(n))
}

async fn test_state() -> AppState {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for tests");
    let pool = connect(&database_url)
        .await
        .expect("connect to test database");
    migrate(&pool).await.expect("run migrations");
    let chain = Arc::new(StubChain::new());
    AppState::new(pool, chain)
}

fn add_actor(builder: axum::http::request::Builder, address: &str) -> axum::http::request::Builder {
    builder.header("x-verdant-actor", address)
}

#[tokio::test]
async fn register_farmer_returns_201_with_va_id() {
    let state = test_state().await;
    let addr = test_address(1);
    let id = test_id(1);
    let body = json!({
        "address": addr,
        "metadata": { "name": "Ada Farm Cooperative", "region": "Niger" }
    });
    let response = app(state)
        .oneshot(
            add_actor(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/farmers/register")
                    .header("content-type", "application/json"),
                &addr,
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
    let addr = test_address(2);
    let body = json!({
        "address": addr,
        "metadata": { "name": "Ada Farm Cooperative" }
    });
    let req = |actor: &str| {
        Request::builder()
            .method("POST")
            .uri("/api/v1/farmers/register")
            .header("content-type", "application/json")
            .header("x-verdant-actor", actor)
            .body(Body::from(body.to_string()))
            .unwrap()
    };
    app(state.clone()).oneshot(req(&addr)).await.unwrap();
    let response = app(state).oneshot(req(&addr)).await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let json = body_to_json(response).await;
    assert_eq!(json["error"], "farmer already registered");
}

#[tokio::test]
async fn get_farmer_returns_200() {
    let state = test_state().await;
    let addr = test_address(3);
    let id = test_id(3);
    let body = json!({
        "address": addr,
        "metadata": { "name": "Ada Farm Cooperative" }
    });
    app(state.clone())
        .oneshot(
            add_actor(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/farmers/register")
                    .header("content-type", "application/json"),
                &addr,
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
    let addr = test_address(4);
    let id = test_id(4);
    let body = json!({
        "address": addr,
        "metadata": { "name": "Ada Farm Cooperative" }
    });
    app(state.clone())
        .oneshot(
            add_actor(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/farmers/register")
                    .header("content-type", "application/json"),
                &addr,
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
    let addr = test_address(5);
    let body = json!({
        "address": addr,
        "metadata": { "name": "Ada Farm Cooperative", "region": "Niger" }
    });
    app(state.clone())
        .oneshot(
            add_actor(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/farmers/register")
                    .header("content-type", "application/json"),
                &addr,
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
            add_actor(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/api/v1/farmers/{addr}/metadata"))
                    .header("content-type", "application/json"),
                &addr,
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
    let addr = test_address(6);
    let body = json!({
        "address": addr,
        "metadata": { "name": "Ada Farm Cooperative" }
    });
    app(state.clone())
        .oneshot(
            add_actor(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/farmers/register")
                    .header("content-type", "application/json"),
                &addr,
            )
            .body(Body::from(body.to_string()))
            .unwrap(),
        )
        .await
        .unwrap();

    let update_body = json!({ "metadata": { "name": "Hacker" } });
    let response = app(state)
        .oneshot(
            add_actor(
                Request::builder()
                    .method("PUT")
                    .uri(format!("/api/v1/farmers/{addr}/metadata"))
                    .header("content-type", "application/json"),
                "GOTHERXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
            )
            .body(Body::from(update_body.to_string()))
            .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn register_missing_actor_returns_401() {
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
    let body = json!({
        "address": "not-a-valid-stellar-key",
        "metadata": { "name": "Ada Farm Cooperative" }
    });
    let response = app(state)
        .oneshot(
            add_actor(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/farmers/register")
                    .header("content-type", "application/json"),
                "not-a-valid-stellar-key",
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
    let addr = test_address(8);
    let body = json!({
        "address": addr,
        "metadata": { "name": "" }
    });
    let response = app(state)
        .oneshot(
            add_actor(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/farmers/register")
                    .header("content-type", "application/json"),
                &addr,
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
    let addr = test_address(9);
    let body = json!({
        "address": addr,
        "metadata": { "name": "Ada Farm Cooperative" },
        "metadataHash": "deadbeef"
    });
    let response = app(state)
        .oneshot(
            add_actor(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/farmers/register")
                    .header("content-type", "application/json"),
                &addr,
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
