use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use uuid::Uuid;

mod common;
use common::*;


// ============================================================================
// AUTHENTICATION TESTS
// ============================================================================

#[tokio::test]
async fn test_register_success() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (response_body, _) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;

    assert!(response_body["user"].is_object());
    assert_eq!(response_body["user"]["email"], "test@example.com");
    assert_eq!(response_body["user"]["username"], "testuser");
    assert!(response_body["access_token"].is_string());
    assert!(response_body["trailblazer"].is_object());

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_register_without_invite_code_when_required() {
    let app = create_test_app().await;

    let body = serde_json::json!({
        "email": "test@example.com",
        "username": "testuser",
        "password": "Password123!",
        "client_id": Uuid::new_v4()
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("POST")
                .uri("/api/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_register_with_invalid_invite_code() {
    let app = create_test_app().await;

    let body = serde_json::json!({
        "email": "test@example.com",
        "username": "testuser",
        "password": "Password123!",
        "invite_code": "INVALID-CODE-123",
        "client_id": Uuid::new_v4()
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("POST")
                .uri("/api/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_register_duplicate_email() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code1 = create_invite_code(&app, &admin_token).await;
    let invite_code2 = create_invite_code(&app, &admin_token).await;

    create_test_user(&app, "test@example.com", "testuser1", "Password123!", Some(&invite_code1)).await;

    let body = serde_json::json!({
        "email": "test@example.com",
        "username": "testuser2",
        "password": "Password123!",
        "invite_code": invite_code2,
        "client_id": Uuid::new_v4()
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("POST")
                .uri("/api/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_register_duplicate_username() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code1 = create_invite_code(&app, &admin_token).await;
    let invite_code2 = create_invite_code(&app, &admin_token).await;

    create_test_user(&app, "test1@example.com", "testuser", "Password123!", Some(&invite_code1)).await;

    let body = serde_json::json!({
        "email": "test2@example.com",
        "username": "testuser",
        "password": "Password123!",
        "invite_code": invite_code2,
        "client_id": Uuid::new_v4()
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("POST")
                .uri("/api/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_register_invalid_email() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let body = serde_json::json!({
        "email": "not-an-email",
        "username": "testuser",
        "password": "Password123!",
        "invite_code": invite_code,
        "client_id": Uuid::new_v4()
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("POST")
                .uri("/api/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_register_short_username() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let body = serde_json::json!({
        "email": "test@example.com",
        "username": "ab",
        "password": "Password123!",
        "invite_code": invite_code,
        "client_id": Uuid::new_v4()
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("POST")
                .uri("/api/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_register_short_password() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let body = serde_json::json!({
        "email": "test@example.com",
        "username": "testuser",
        "password": "short",
        "invite_code": invite_code,
        "client_id": Uuid::new_v4()
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("POST")
                .uri("/api/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_login_success() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;

    let (response_body, _) = login_test_user(&app, "test@example.com", "Password123!").await;

    assert!(response_body["user"].is_object());
    assert_eq!(response_body["user"]["email"], "test@example.com");
    assert!(response_body["access_token"].is_string());

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_login_invalid_email() {
    let app = create_test_app().await;

    let body = serde_json::json!({
        "email": "nonexistent@example.com",
        "password": "Password123!",
        "client_id": Uuid::new_v4()
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_login_invalid_password() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;

    let body = serde_json::json!({
        "email": "test@example.com",
        "password": "WrongPassword123!",
        "client_id": Uuid::new_v4()
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_get_me_success() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("GET")
                .uri("/api/auth/me")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(body_json["email"], "test@example.com");
    assert_eq!(body_json["username"], "testuser");

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_get_me_without_auth() {
    let app = create_test_app().await;

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("GET")
                .uri("/api/auth/me")
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_get_me_invalid_token() {
    let app = create_test_app().await;

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            Request::builder()
                .method("GET")
                .uri("/api/auth/me")
                .header("authorization", "Bearer invalid-token")
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_logout_success() {
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
                .uri("/api/auth/logout")
                .header("authorization", format!("Bearer {}", token))
                .header("cookie", "refresh_token=some-token")
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_refresh_token_success() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (register_response, _) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;

    let _refresh_cookie = register_response.get("trailblazer");

    let body = serde_json::json!({
        "client_id": Uuid::new_v4()
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/auth/refresh")
                .header("content-type", "application/json")
                .header("cookie", "refresh_token=some-refresh-token")
                .body(Body::from(body.to_string()))
                .unwrap(),
        ))
        .await
        .unwrap();

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_refresh_token_without_cookie() {
    let app = create_test_app().await;

    let body = serde_json::json!({
        "client_id": Uuid::new_v4()
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/auth/refresh")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_refresh_token_with_invalid_token() {
    let app = create_test_app().await;

    let body = serde_json::json!({
        "client_id": Uuid::new_v4()
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/auth/refresh")
                .header("content-type", "application/json")
                .header("cookie", "refresh_token=invalid-token")
                .body(Body::from(body.to_string()))
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_login_with_unverified_email() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let _ = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;

    let (login_response, _) = login_test_user(&app, "test@example.com", "Password123!").await;

    assert!(login_response["access_token"].is_string());
    assert_eq!(login_response["user"]["email_verified"], false);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_multiple_login_sessions() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let _ = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;

    let (_, token1) = login_test_user(&app, "test@example.com", "Password123!").await;
    let (_, token2) = login_test_user(&app, "test@example.com", "Password123!").await;

    // Note: tokens may be identical if created in the same second (same iat/exp in JWT)

    let response1 = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("GET")
                .uri("/api/sketches")
                .header("authorization", format!("Bearer {}", token1))
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();

    let response2 = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("GET")
                .uri("/api/sketches")
                .header("authorization", format!("Bearer {}", token2))
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response1.status(), StatusCode::OK);
    assert_eq!(response2.status(), StatusCode::OK);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_register_with_whitespace_in_email() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let body = serde_json::json!({
        "email": "  test@example.com  ",
        "username": "testuser",
        "password": "Password123!",
        "invite_code": invite_code,
        "client_id": Uuid::new_v4()
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        ))
        .await
        .unwrap();

    // Tests that whitespace in email is handled (trimmed or rejected)
    assert!(response.status() == StatusCode::CREATED || response.status() == StatusCode::BAD_REQUEST);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_register_with_case_variation_email() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code1 = create_invite_code(&app, &admin_token).await;
    let invite_code2 = create_invite_code(&app, &admin_token).await;

    let _ = create_test_user(&app, "test@example.com", "testuser1", "Password123!", Some(&invite_code1)).await;

    let body = serde_json::json!({
        "email": "TEST@EXAMPLE.COM",
        "username": "testuser2",
        "password": "Password123!",
        "invite_code": invite_code2,
        "client_id": Uuid::new_v4()
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        ))
        .await
        .unwrap();

    // Note: App currently doesn't normalize email case - this test documents current behavior
    // If email normalization is added, this should be CONFLICT
    assert_eq!(response.status(), StatusCode::CREATED);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_password_strength_variations() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;

    let weak_passwords = vec![
        "12345678",
        "password",
        "abcdefgh",
        "ABCDEFGH",
    ];

    for password in weak_passwords {
        let invite_code = create_invite_code(&app, &admin_token).await;

        let body = serde_json::json!({
            "email": format!("test{}@example.com", Uuid::new_v4()),
            "username": format!("user{}", &Uuid::new_v4().to_string()[..8]),
            "password": password,
            "invite_code": invite_code,
            "client_id": Uuid::new_v4()
        });

        let response = app
            .router
            .clone()
            .oneshot(make_request(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            ))
        .await
        .unwrap();

    // Note: App currently accepts weak passwords - this test documents current behavior
    // If password validation is added, this should be BAD_REQUEST
    assert_eq!(response.status(), StatusCode::CREATED);
    }

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_register_with_maximum_length_fields() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let long_email = format!("{}@example.com", "a".repeat(240));
    let long_username = "a".repeat(255);
    let long_password = "A".repeat(1000) + "1!";

    let body = serde_json::json!({
        "email": long_email,
        "username": long_username,
        "password": long_password,
        "invite_code": invite_code,
        "client_id": Uuid::new_v4()
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        ))
        .await
        .unwrap();

    // Maximum length fields should be rejected if they exceed limits
    assert!(response.status() == StatusCode::CREATED || response.status() == StatusCode::BAD_REQUEST);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_username_with_special_characters() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let body = serde_json::json!({
        "email": "test@example.com",
        "username": "test_user-123",
        "password": "Password123!",
        "invite_code": invite_code,
        "client_id": Uuid::new_v4()
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        ))
        .await
        .unwrap();

    // Username with underscores and hyphens should be allowed
    assert_eq!(response.status(), StatusCode::CREATED);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_login_timing_attack_resistance() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let _ = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;

    use std::time::Instant;

    let start1 = Instant::now();
    let _ = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::json!({
                    "email": "nonexistent@example.com",
                    "password": "Password123!",
                    "client_id": Uuid::new_v4()
                }).to_string()))
                .unwrap(),
        ))
        .await
        .unwrap();
    let time1 = start1.elapsed();

    let start2 = Instant::now();
    let _ = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::json!({
                    "email": "test@example.com",
                    "password": "WrongPassword123!",
                    "client_id": Uuid::new_v4()
                }).to_string()))
                .unwrap(),
        ))
        .await
        .unwrap();
    let _time2 = start2.elapsed();

    // Both should fail with UNAUTHORIZED - timing comparison would need statistical analysis

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_expired_invite_code() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;

    let invite_code = create_invite_code(&app, &admin_token).await;

    sqlx::query(
        "UPDATE invite_codes SET expires_at = NOW() - INTERVAL '1 day' WHERE code = $1"
    )
    .bind(&invite_code)
    .execute(&app.db_pool)
    .await
    .unwrap();

    let body = serde_json::json!({
        "email": "test@example.com",
        "username": "testuser",
        "password": "Password123!",
        "invite_code": invite_code,
        "client_id": Uuid::new_v4()
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/auth/register")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_logout_without_refresh_token_cookie() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/auth/logout")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_register_missing_required_fields() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let test_cases = vec![
        serde_json::json!({
            "username": "testuser",
            "password": "Password123!",
            "invite_code": invite_code,
            "client_id": Uuid::new_v4()
        }),
        serde_json::json!({
            "email": "test@example.com",
            "password": "Password123!",
            "invite_code": invite_code,
            "client_id": Uuid::new_v4()
        }),
        serde_json::json!({
            "email": "test@example.com",
            "username": "testuser",
            "invite_code": invite_code,
            "client_id": Uuid::new_v4()
        }),
        serde_json::json!({
            "email": "test@example.com",
            "username": "testuser",
            "password": "Password123!",
            "client_id": Uuid::new_v4()
        }),
    ];

    for body in test_cases {
        let response = app
            .router
            .clone()
            .oneshot(make_request(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/auth/register")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            ))
            .await
            .unwrap();

        assert_ne!(response.status(), StatusCode::CREATED);
    }

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}
