use axum::{
    body::{to_bytes, Body},
    http::StatusCode,
};
use tower::ServiceExt;

mod common;
use common::*;

#[tokio::test]
async fn test_create_route_success() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;
    let sketch_id = create_test_sketch(&app, &token, "Test Sketch").await;

    let body = serde_json::json!({
        "name": "Mountain Trail",
        "description": "A challenging mountain trail",
        "geojson": {
            "type": "FeatureCollection",
            "features": [
                {
                    "type": "Feature",
                    "geometry": {
                        "type": "LineString",
                        "coordinates": [[0.0, 0.0], [1.0, 1.0], [2.0, 2.0]]
                    },
                    "properties": {}
                }
            ]
        },
        "metadata": {"distance": 5000.0}
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/api/sketches/{}/routes", sketch_id))
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
    assert_eq!(body_json["name"], "Mountain Trail");
    assert_eq!(body_json["version"], 1);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_list_routes_success() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;
    let sketch_id = create_test_sketch(&app, &token, "Test Sketch").await;

    create_test_route(&app, &token, sketch_id, "Route 1").await;
    create_test_route(&app, &token, sketch_id, "Route 2").await;

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("GET")
                .uri(format!("/api/sketches/{}/routes", sketch_id))
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(body_json.as_array().unwrap().len(), 2);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_get_route_success() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;
    let sketch_id = create_test_sketch(&app, &token, "Test Sketch").await;
    let route_id = create_test_route(&app, &token, sketch_id, "Test Route").await;

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("GET")
                .uri(format!("/api/routes/{}", route_id))
                .header("authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(body_json["id"], route_id.to_string());

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_update_route_success() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;
    let sketch_id = create_test_sketch(&app, &token, "Test Sketch").await;
    let route_id = create_test_route(&app, &token, sketch_id, "Test Route").await;

    let body = serde_json::json!({
        "name": "Updated Route Name",
        "description": "Updated description"
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("PUT")
                .uri(format!("/api/routes/{}", route_id))
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

    assert_eq!(body_json["name"], "Updated Route Name");
    assert_eq!(body_json["version"], 2);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_delete_route_success() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;
    let sketch_id = create_test_sketch(&app, &token, "Test Sketch").await;
    let route_id = create_test_route(&app, &token, sketch_id, "Route to Delete").await;

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("DELETE")
                .uri(format!("/api/routes/{}", route_id))
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
            axum::http::Request::builder()
                .method("GET")
                .uri(format!("/api/routes/{}", route_id))
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
async fn test_get_route_wrong_user() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code1 = create_invite_code(&app, &admin_token).await;
    let invite_code2 = create_invite_code(&app, &admin_token).await;

    let (_, token1) = create_test_user(&app, "user1@example.com", "user1", "Password123!", Some(&invite_code1)).await;
    let (_, token2) = create_test_user(&app, "user2@example.com", "user2", "Password123!", Some(&invite_code2)).await;

    let sketch_id = create_test_sketch(&app, &token1, "User 1's Sketch").await;
    let route_id = create_test_route(&app, &token1, sketch_id, "User 1's Route").await;

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("GET")
                .uri(format!("/api/routes/{}", route_id))
                .header("authorization", format!("Bearer {}", token2))
                .body(Body::empty())
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}
