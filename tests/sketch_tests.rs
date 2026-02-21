use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use uuid::Uuid;

mod common;
use common::*;

// ============================================================================
// SKETCH TESTS
// ============================================================================

#[tokio::test]
async fn test_create_sketch_success() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;

    let body = serde_json::json!({
        "name": "My Hiking Route",
        "description": "A beautiful hiking route through the mountains"
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("POST")
                .uri("/api/sketches")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::from(body.to_string()))
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(body_json["id"].is_string());
    assert_eq!(body_json["name"], "My Hiking Route");
    assert_eq!(body_json["route_count"], 0);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_create_sketch_empty_name() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;

    let body = serde_json::json!({
        "name": "",
        "description": "Test description"
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("POST")
                .uri("/api/sketches")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::from(body.to_string()))
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_create_sketch_long_name() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;

    let body = serde_json::json!({
        "name": "a".repeat(300),
        "description": "Test description"
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("POST")
                .uri("/api/sketches")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::from(body.to_string()))
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_list_sketches_success() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;

    create_test_sketch(&app, &token, "Sketch 1").await;
    create_test_sketch(&app, &token, "Sketch 2").await;
    create_test_sketch(&app, &token, "Sketch 3").await;

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("GET")
                .uri("/api/sketches")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(body_json.is_array());
    assert_eq!(body_json.as_array().unwrap().len(), 3);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_get_sketch_success() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;

    let sketch_id = create_test_sketch(&app, &token, "Test Sketch").await;

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("GET")
                .uri(format!("/api/sketches/{}", sketch_id))
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(body_json["id"], sketch_id.to_string());
    assert_eq!(body_json["name"], "Test Sketch");

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_get_sketch_not_found() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;

    let fake_id = Uuid::new_v4();

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("GET")
                .uri(format!("/api/sketches/{}", fake_id))
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_get_sketch_wrong_user() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code1 = create_invite_code(&app, &admin_token).await;
    let invite_code2 = create_invite_code(&app, &admin_token).await;

    let (_, token1) = create_test_user(&app, "user1@example.com", "user1", "Password123!", Some(&invite_code1)).await;
    let (_, token2) = create_test_user(&app, "user2@example.com", "user2", "Password123!", Some(&invite_code2)).await;

    let sketch_id = create_test_sketch(&app, &token1, "User 1's Sketch").await;

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("GET")
                .uri(format!("/api/sketches/{}", sketch_id))
                .header("authorization", format!("Bearer {}", token2))
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_update_sketch_success() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;

    let sketch_id = create_test_sketch(&app, &token, "Original Name").await;

    let body = serde_json::json!({
        "name": "Updated Name",
        "description": "Updated description",
        "is_public": true
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/sketches/{}", sketch_id))
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::from(body.to_string()))
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(body_json["name"], "Updated Name");
    assert_eq!(body_json["is_public"], true);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_delete_sketch_success() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;

    let sketch_id = create_test_sketch(&app, &token, "Sketch to Delete").await;

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/sketches/{}", sketch_id))
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("GET")
                .uri(format!("/api/sketches/{}", sketch_id))
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}
