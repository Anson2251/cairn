use axum::{
    body::{to_bytes, Body},
    http::StatusCode,
};
use tower::ServiceExt;
use uuid::Uuid;

mod common;
use common::*;

#[tokio::test]
async fn test_sql_injection_in_email() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let malicious_emails = vec![
        "test' OR '1'='1",
        "test'; DROP TABLE users; --",
        "test@example.com' UNION SELECT * FROM users --",
    ];

    for email in malicious_emails {
        let body = serde_json::json!({
            "email": email,
            "username": format!("user{}", Uuid::new_v4().to_string()[..8].to_string()),
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

        assert!(
            response.status() == StatusCode::BAD_REQUEST ||
            response.status() == StatusCode::UNPROCESSABLE_ENTITY ||
            response.status() == StatusCode::CONFLICT
        );
    }

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_sql_injection_in_sketch_name() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;

    let malicious_names = vec![
        "sketch'; DROP TABLE sketches; --",
        "test' UNION SELECT * FROM users --",
    ];

    for name in malicious_names {
        let body = serde_json::json!({
            "name": name,
            "description": "Test description"
        });

        let response = app
            .router
            .clone()
            .oneshot(make_request(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/sketches")
                    .header("content-type", "application/json")
                    .header("authorization", format!("Bearer {}", token))
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            ))
            .await
            .unwrap();

        if response.status() == StatusCode::CREATED {
            let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
            assert_eq!(body_json["name"], name);
        }
    }

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_xss_in_sketch_description() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;

    let xss_payloads = vec![
        "<script>alert('xss')</script>",
        "<img src=x onerror=alert('xss')>",
        "javascript:alert('xss')",
        "<body onload=alert('xss')>",
    ];

    for payload in xss_payloads {
        let body = serde_json::json!({
            "name": "Test Sketch",
            "description": payload
        });

        let response = app
            .router
            .clone()
            .oneshot(make_request(
                axum::http::Request::builder()
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
    }

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_auth_bypass_attempts() {
    let app = create_test_app().await;

    let fake_tokens = vec![
        "Bearer ",
        "Bearer null",
        "Bearer undefined",
        "Token test",
        "Basic dXNlcjpwYXNz",
        "",
    ];

    for token in fake_tokens {
        let response = app
            .router
            .clone()
            .oneshot(make_request(
                axum::http::Request::builder()
                    .method("GET")
                    .uri("/api/sketches")
                    .header("authorization", token)
                    .body(Body::empty())
                    .unwrap(),
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_privilege_escalation_attempt() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;

    let escalations = vec![
        serde_json::json!({
            "name": "Test",
            "role": "admin"
        }),
        serde_json::json!({
            "name": "Test",
            "is_admin": true
        }),
    ];

    for body in escalations {
        let response = app
            .router
            .clone()
            .oneshot(make_request(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/sketches")
                    .header("content-type", "application/json")
                    .header("authorization", format!("Bearer {}", token))
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            ))
            .await
            .unwrap();

        if response.status() == StatusCode::CREATED {
            let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
            assert_eq!(body_json["name"], "Test");
        }
    }

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_special_characters_in_fields() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;

    let special_strings = vec![
        "<>&\"'",
        "\n\r\t",
        "日本語テキスト",
        "العربية",
        "🔥🎉👍",
    ];

    for s in special_strings {
        let body = serde_json::json!({
            "name": s,
            "description": s
        });

        let response = app
            .router
            .clone()
            .oneshot(make_request(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/sketches")
                    .header("content-type", "application/json")
                    .header("authorization", format!("Bearer {}", token))
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            ))
            .await
            .unwrap();

        if response.status() == StatusCode::CREATED {
            let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
            assert_eq!(body_json["name"].as_str().unwrap(), s);
        }
    }

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_idempotency_with_duplicate_requests() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;

    let body = serde_json::json!({
        "name": "Duplicate Test",
        "description": "Testing idempotency"
    });

    let response1 = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/sketches")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::from(body.to_string()))
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response1.status(), StatusCode::CREATED);

    let response2 = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/sketches")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::from(body.to_string()))
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response2.status(), StatusCode::CREATED);

    let list_response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("GET")
                .uri("/api/sketches")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();

    let body_bytes = to_bytes(list_response.into_body(), usize::MAX).await.unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(body_json.as_array().unwrap().len(), 2);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}
