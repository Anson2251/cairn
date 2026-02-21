use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use tower::ServiceExt;

mod common;
use common::*;

#[tokio::test]
async fn test_validate_invite_success() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("POST")
                .uri("/api/admin/invites")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", admin_token))
                .body(Body::from(r#"{"count":1}"#))
                .unwrap(),
        ))
        .await
        .unwrap();

    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    let code = body_json["codes"][0]["code"].as_str().unwrap();
    let cairn_name = body_json["codes"][0]["cairn_name"].as_str().unwrap();

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("GET")
                .uri(format!("/api/invite/{}/validate", code))
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(body_json["valid"], true);
    assert_eq!(body_json["cairn_name"], cairn_name);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_validate_invite_invalid_code() {
    let app = create_test_app().await;

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("GET")
                .uri("/api/invite/INVALID-CODE-123/validate")
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_validate_invite_already_used() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("GET")
                .uri(format!("/api/invite/{}/validate", invite_code))
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_create_invites_as_admin() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("POST")
                .uri("/api/admin/invites")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", admin_token))
                .body(Body::from(r#"{"count":5}"#))
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(body_json["codes"].as_array().unwrap().len(), 5);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_create_invites_as_regular_user() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("POST")
                .uri("/api/admin/invites")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::from(r#"{"count":1}"#))
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_list_invites_as_admin() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;

    let _ = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("POST")
                .uri("/api/admin/invites")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", admin_token))
                .body(Body::from(r#"{"count":3}"#))
                .unwrap(),
        ))
        .await
        .unwrap();

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("GET")
                .uri("/api/admin/invites")
                .header("authorization", format!("Bearer {}", admin_token))
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(body_json.is_array());
    assert!(body_json.as_array().unwrap().len() >= 3);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_revoke_invite_success() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("POST")
                .uri("/api/admin/invites")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", admin_token))
                .body(Body::from(r#"{"count":1}"#))
                .unwrap(),
        ))
        .await
        .unwrap();

    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    let invite_id = body_json["codes"][0]["id"].as_str().unwrap();

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/admin/invites/{}", invite_id))
                .header("authorization", format!("Bearer {}", admin_token))
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_revoke_invite_already_used() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("POST")
                .uri("/api/admin/invites")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", admin_token))
                .body(Body::from(r#"{"count":1}"#))
                .unwrap(),
        ))
        .await
        .unwrap();

    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    let invite_code = body_json["codes"][0]["code"].as_str().unwrap();
    let invite_id = body_json["codes"][0]["id"].as_str().unwrap();

    create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(invite_code)).await;

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/admin/invites/{}", invite_id))
                .header("authorization", format!("Bearer {}", admin_token))
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_invite_code_format() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("POST")
                .uri("/api/admin/invites")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", admin_token))
                .body(Body::from(r#"{"count":1}"#))
                .unwrap(),
        ))
        .await
        .unwrap();

    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    let code = body_json["codes"][0]["code"].as_str().unwrap();

    assert!(code.starts_with("CAIRN-"));
    let parts: Vec<&str> = code.split('-').collect();
    assert_eq!(parts.len(), 4);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_invite_code_sequence_increment() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("POST")
                .uri("/api/admin/invites")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", admin_token))
                .body(Body::from(r#"{"count":5}"#))
                .unwrap(),
        ))
        .await
        .unwrap();

    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    let sequences: Vec<i64> = body_json["codes"].as_array().unwrap()
        .iter()
        .map(|c| c["sequence"].as_i64().unwrap())
        .collect();

    for i in 1..sequences.len() {
        assert_eq!(sequences[i], sequences[i-1] + 1);
    }

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}
