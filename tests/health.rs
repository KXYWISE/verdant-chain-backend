use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use verdant_backend::{AppState, app, connect, migrate};

async fn test_state() -> AppState {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for tests");
    let pool = connect(&database_url)
        .await
        .expect("connect to test database");
    migrate(&pool).await.expect("run migrations");
    AppState::new(pool)
}

#[tokio::test]
async fn health_returns_ok_with_database_up() {
    let state = test_state().await;
    let response = app(state)
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("send request");

    assert_eq!(response.status(), StatusCode::OK);

    let body = response
        .into_body()
        .collect()
        .await
        .expect("read body")
        .to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).expect("valid json");
    assert_eq!(json["status"], "ok");
    assert_eq!(json["database"], "up");
}

#[tokio::test]
async fn unknown_route_returns_not_found() {
    let state = test_state().await;
    let response = app(state)
        .oneshot(
            Request::builder()
                .uri("/does-not-exist")
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("send request");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
