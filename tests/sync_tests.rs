use axum::{
    body::{to_bytes, Body},
    http::StatusCode,
};
use chrono::Utc;
use tower::ServiceExt;
use uuid::Uuid;

mod common;
use common::*;

fn make_route_data() -> serde_json::Value {
    serde_json::json!({
        "name": Some("Test Route".to_string()),
        "description": Some("Test Description".to_string()),
        "geojson": {
            "type": "FeatureCollection",
            "features": [{
                "type": "Feature",
                "geometry": {
                    "type": "LineString",
                    "coordinates": [[0.0, 0.0], [1.0, 1.0]]
                },
                "properties": {}
            }]
        },
        "metadata": {"distance": 1000.0},
        "notes": Some("Test notes".to_string())
    })
}

#[tokio::test]
async fn test_sync_push_single_route_success() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;
    let sketch_id = create_test_sketch(&app, &token, "Test Sketch").await;
    let route_id = create_test_route(&app, &token, sketch_id, "Test Route").await;

    let body = serde_json::json!({
        "client_id": Uuid::new_v4(),
        "changes": [{
            "route_id": route_id,
            "base_version": 1,
            "data": {
                "name": Some("Updated Route Name".to_string()),
                "description": Some("Updated".to_string()),
                "geojson": {
                    "type": "FeatureCollection",
                    "features": []
                },
                "metadata": {},
                "notes": serde_json::Value::Null
            }
        }]
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/sync/push")
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

    assert!(body_json["accepted"].as_array().unwrap().contains(&route_id.to_string().into()));
    assert!(body_json["conflicts"].as_array().unwrap().is_empty());

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_sync_push_conflict_detection() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;
    let sketch_id = create_test_sketch(&app, &token, "Test Sketch").await;
    let route_id = create_test_route(&app, &token, sketch_id, "Test Route").await;

    let body = serde_json::json!({
        "client_id": Uuid::new_v4(),
        "changes": [{
            "route_id": route_id,
            "base_version": 0,
            "data": make_route_data()
        }]
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/sync/push")
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

    assert!(body_json["accepted"].as_array().unwrap().is_empty());
    assert_eq!(body_json["conflicts"].as_array().unwrap().len(), 1);
    assert_eq!(body_json["conflicts"][0]["route_id"], route_id.to_string());
    assert_eq!(body_json["conflicts"][0]["local_version"], 0);
    assert_eq!(body_json["conflicts"][0]["server_version"], 1);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_sync_push_nonexistent_route_returns_conflict() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;

    let fake_route_id = Uuid::new_v4();
    let body = serde_json::json!({
        "client_id": Uuid::new_v4(),
        "changes": [{
            "route_id": fake_route_id,
            "base_version": 1,
            "data": make_route_data()
        }]
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/sync/push")
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

    assert!(body_json["conflicts"].as_array().unwrap().len() > 0);
    assert_eq!(body_json["conflicts"][0]["server_version"], 0);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_sync_push_empty_changes() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;

    let body = serde_json::json!({
        "client_id": Uuid::new_v4(),
        "changes": []
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/sync/push")
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

    assert!(body_json["accepted"].as_array().unwrap().is_empty());
    assert!(body_json["conflicts"].as_array().unwrap().is_empty());

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_sync_push_multiple_routes_mixed_results() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;
    let sketch_id = create_test_sketch(&app, &token, "Test Sketch").await;

    let route1 = create_test_route(&app, &token, sketch_id, "Route 1").await;
    let route2 = create_test_route(&app, &token, sketch_id, "Route 2").await;
    let fake_route = Uuid::new_v4();

    let body = serde_json::json!({
        "client_id": Uuid::new_v4(),
        "changes": [
            {
                "route_id": route1,
                "base_version": 1,
                "data": make_route_data()
            },
            {
                "route_id": route2,
                "base_version": 0,
                "data": make_route_data()
            },
            {
                "route_id": fake_route,
                "base_version": 1,
                "data": make_route_data()
            }
        ]
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/sync/push")
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

    assert_eq!(body_json["accepted"].as_array().unwrap().len(), 1);
    assert_eq!(body_json["conflicts"].as_array().unwrap().len(), 2);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_sync_pull_initial_sync() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;
    let sketch_id = create_test_sketch(&app, &token, "Test Sketch").await;
    create_test_route(&app, &token, sketch_id, "Route 1").await;
    create_test_route(&app, &token, sketch_id, "Route 2").await;

    let body = serde_json::json!({
        "client_id": Uuid::new_v4(),
        "last_synced_at": None::<String>,
        "known_versions": {}
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/sync/pull")
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

    assert_eq!(body_json["updated"].as_array().unwrap().len(), 2);
    assert!(body_json["server_time"].is_string());

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_sync_pull_with_known_versions_filters_out_duplicates() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;
    let sketch_id = create_test_sketch(&app, &token, "Test Sketch").await;
    let route_id = create_test_route(&app, &token, sketch_id, "Route 1").await;

    let body = serde_json::json!({
        "client_id": Uuid::new_v4(),
        "last_synced_at": None::<String>,
        "known_versions": {
            route_id.to_string(): 1
        }
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/sync/pull")
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

    assert!(body_json["updated"].as_array().unwrap().is_empty());

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_sync_pull_includes_deleted_routes() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;
    let sketch_id = create_test_sketch(&app, &token, "Test Sketch").await;
    let route_id = create_test_route(&app, &token, sketch_id, "Route to Delete").await;

    let before_delete = Utc::now().to_rfc3339();

    let _ = app
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

    let body = serde_json::json!({
        "client_id": Uuid::new_v4(),
        "last_synced_at": before_delete,
        "known_versions": {}
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/sync/pull")
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

    assert!(body_json["deleted"].as_array().unwrap().contains(&route_id.to_string().into()));

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_sync_resolve_conflict_success() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;
    let sketch_id = create_test_sketch(&app, &token, "Test Sketch").await;
    let route_id = create_test_route(&app, &token, sketch_id, "Test Route").await;

    let body = serde_json::json!({
        "resolution": "client_wins",
        "base_version": 1,
        "data": {
            "name": Some("Resolved Route Name".to_string()),
            "description": Some("Resolved Description".to_string()),
            "geojson": {"type": "FeatureCollection", "features": []},
            "metadata": {},
            "notes": Some("Resolved notes".to_string())
        }
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/api/sync/resolve/{}", route_id))
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

    assert_eq!(body_json["resolved"], true);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_sync_resolve_conflict_version_mismatch() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;
    let sketch_id = create_test_sketch(&app, &token, "Test Sketch").await;
    let route_id = create_test_route(&app, &token, sketch_id, "Test Route").await;

    let body = serde_json::json!({
        "resolution": "client_wins",
        "base_version": 999,
        "data": make_route_data()
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/api/sync/resolve/{}", route_id))
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::from(body.to_string()))
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_sync_resolve_conflict_nonexistent_route() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;

    let fake_route_id = Uuid::new_v4();
    let body = serde_json::json!({
        "resolution": "client_wins",
        "base_version": 1,
        "data": make_route_data()
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("POST")
                .uri(format!("/api/sync/resolve/{}", fake_route_id))
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::from(body.to_string()))
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_sync_push_unauthorized() {
    let app = create_test_app().await;

    let body = serde_json::json!({
        "client_id": Uuid::new_v4(),
        "changes": []
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/sync/push")
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
async fn test_sync_pull_unauthorized() {
    let app = create_test_app().await;

    let body = serde_json::json!({
        "client_id": Uuid::new_v4(),
        "last_synced_at": None::<String>,
        "known_versions": {}
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/sync/pull")
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
async fn test_sync_cannot_push_to_other_users_route() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code1 = create_invite_code(&app, &admin_token).await;
    let invite_code2 = create_invite_code(&app, &admin_token).await;

    let (_, token1) = create_test_user(&app, "user1@example.com", "user1", "Password123!", Some(&invite_code1)).await;
    let (_, token2) = create_test_user(&app, "user2@example.com", "user2", "Password123!", Some(&invite_code2)).await;

    let sketch_id = create_test_sketch(&app, &token1, "User 1's Sketch").await;
    let route_id = create_test_route(&app, &token1, sketch_id, "User 1's Route").await;

    let body = serde_json::json!({
        "client_id": Uuid::new_v4(),
        "changes": [{
            "route_id": route_id,
            "base_version": 1,
            "data": make_route_data()
        }]
    });

    let response = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/sync/push")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token2))
                .body(Body::from(body.to_string()))
                .unwrap(),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(body_json["accepted"].as_array().unwrap().is_empty());
    assert_eq!(body_json["conflicts"].as_array().unwrap().len(), 1);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}

#[tokio::test]
async fn test_sync_version_increments_after_push() {
    let app = create_test_app().await;
    let admin_token = create_admin_user(&app).await;
    let invite_code = create_invite_code(&app, &admin_token).await;

    let (_, token) = create_test_user(&app, "test@example.com", "testuser", "Password123!", Some(&invite_code)).await;
    let sketch_id = create_test_sketch(&app, &token, "Test Sketch").await;
    let route_id = create_test_route(&app, &token, sketch_id, "Test Route").await;

    let body = serde_json::json!({
        "client_id": Uuid::new_v4(),
        "changes": [{
            "route_id": route_id,
            "base_version": 1,
            "data": make_route_data()
        }]
    });

    let _ = app
        .router
        .clone()
        .oneshot(make_request(
            axum::http::Request::builder()
                .method("POST")
                .uri("/api/sync/push")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {}", token))
                .body(Body::from(body.to_string()))
                .unwrap(),
        ))
        .await
        .unwrap();

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

    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(body_json["version"], 2);

    cleanup_test_db(&app.db_pool, &app.db_url).await;
}
