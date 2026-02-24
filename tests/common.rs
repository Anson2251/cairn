use axum::{
    body::{to_bytes, Body},
    extract::connect_info::ConnectInfo,
    http::Request,
    Router,
};
use serde::Serialize;
use sqlx::{migrate::MigrateDatabase, PgPool, Postgres};
use std::net::SocketAddr;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

use cairn::{
    auth::jwt::JwtManager,
    config::AppConfig,
    create_router, db,
    AppState,
};

pub fn get_test_database_url() -> String {
    let db_name = format!("cairn_test_{}", Uuid::new_v4().to_string().replace("-", "_"));
    format!("postgresql://postgres@localhost:5432/{}", db_name)
}

pub struct TestApp {
    pub router: Router,
    pub db_pool: PgPool,
    pub db_url: String,
}

pub async fn setup_test_db() -> (PgPool, String) {
    let db_url = get_test_database_url();
    Postgres::create_database(&db_url).await.unwrap();

    let pool = db::create_pool(&cairn::config::DatabaseConfig {
        url: db_url.clone(),
    })
    .await
    .unwrap();

    db::run_migrations(&pool).await.unwrap();

    (pool, db_url)
}

pub async fn cleanup_test_db(pool: &PgPool, db_url: &str) {
    // Delete all data first
    sqlx::query("DELETE FROM sync_log").execute(pool).await.ok();
    sqlx::query("DELETE FROM routes").execute(pool).await.ok();
    sqlx::query("DELETE FROM sketches").execute(pool).await.ok();
    sqlx::query("DELETE FROM refresh_tokens").execute(pool).await.ok();
    sqlx::query("DELETE FROM invite_codes").execute(pool).await.ok();
    sqlx::query("DELETE FROM users").execute(pool).await.ok();

    // Drop the database
    Postgres::drop_database(db_url).await.ok();
}

pub fn make_request(builder: axum::http::Request<Body>) -> axum::http::Request<Body> {
    let (parts, body) = builder.into_parts();
    let mut request = Request::from_parts(parts, body);
    request
        .extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 12345))));
    request
}

pub async fn create_test_app() -> TestApp {
    let (db_pool, db_url) = setup_test_db().await;

    let config = AppConfig {
        server: cairn::config::ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
        },
        database: cairn::config::DatabaseConfig {
            url: db_url.clone(),
        },
        redis: cairn::config::RedisConfig {
            url: "redis://localhost:6379".to_string(),
        },
        jwt: cairn::config::JwtConfig {
            secret: "test-secret-key-for-jwt-tokens-123456789012345678901234567890".to_string(),
            expiry_minutes: 15,
            refresh_secret: "test-refresh-secret-key-for-refresh-tokens-1234567890123456789".to_string(),
            refresh_expiry_days: 7,
        },
        oauth: cairn::config::OAuthConfig {
            google_client_id: None,
            google_client_secret: None,
            github_client_id: None,
            github_client_secret: None,
            redirect_base: "http://localhost:8080".to_string(),
        },
        invite: cairn::config::InviteConfig {
            salt: "test-invite-salt".to_string(),
            require_invite: true,
        },
        smtp: cairn::config::SmtpConfig {
            host: "smtp.example.com".to_string(),
            port: 587,
            user: "".to_string(),
            password: "".to_string(),
            from_email: "test@cairn.local".to_string(),
        },
        rate_limit: cairn::config::RateLimitConfig {
            login_per_minute: 100,
            register_per_hour: 100,
            forgot_password_per_hour: 100,
            invite_validate_per_minute: 100,
            authenticated_per_minute: 1000,
        },
    };

    let redis_client = redis::Client::open(config.redis.url.clone()).unwrap();
    let redis_conn = redis::aio::ConnectionManager::new(redis_client).await.unwrap();

    let jwt = JwtManager::new(&config.jwt).unwrap();

    let state = Arc::new(AppState {
        db: db_pool.clone(),
        redis: redis_conn,
        jwt: Arc::new(jwt),
        config: config.clone(),
    });

    let router = create_router(state);

    TestApp { router, db_pool, db_url }
}

pub async fn create_test_user(app: &TestApp, email: &str, username: &str, password: &str, invite_code: Option<&str>) -> (serde_json::Value, String) {
    let body = serde_json::json!({
        "email": email,
        "username": username,
        "password": password,
        "invite_code": invite_code,
        "client_id": Uuid::new_v4()
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
        .into_parts();

    let mut request = Request::from_parts(request.0, request.1);
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 12345))));

    let response = app
        .router
        .clone()
        .oneshot(request)
        .await
        .unwrap();

    assert_eq!(response.status(), axum::http::StatusCode::CREATED);

    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    let access_token = body_json["access_token"].as_str().unwrap().to_string();

    (body_json, access_token)
}

pub async fn login_test_user(app: &TestApp, email: &str, password: &str) -> (serde_json::Value, String) {
    let body = serde_json::json!({
        "email": email,
        "password": password,
        "client_id": Uuid::new_v4()
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
        .into_parts();

    let mut request = Request::from_parts(request.0, request.1);
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 12345))));

    let response = app
        .router
        .clone()
        .oneshot(request)
        .await
        .unwrap();

    let status = response.status();
    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();

    if status != axum::http::StatusCode::OK {
        let body_text = String::from_utf8_lossy(&body_bytes);
        eprintln!("Login failed with status {:?}: {}", status, body_text);
    }

    assert_eq!(status, axum::http::StatusCode::OK);

    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    let access_token = body_json["access_token"].as_str().unwrap().to_string();

    (body_json, access_token)
}

pub async fn create_admin_user(app: &TestApp) -> String {
    let email = "admin@test.com";
    let username = "admin";
    let password = "AdminPass123!";

    sqlx::query(
        r#"
        INSERT INTO users (id, email, email_verified, username, hashed_password, role, settings)
        VALUES ($1, $2, true, $3, $4, 'admin', '{}')
        "#
    )
    .bind(Uuid::new_v4())
    .bind(email)
    .bind(username)
    .bind(bcrypt::hash(password, bcrypt::DEFAULT_COST).unwrap())
    .execute(&app.db_pool)
    .await
    .unwrap();

    let (_, token) = login_test_user(app, email, password).await;
    token
}

pub async fn create_invite_code(app: &TestApp, admin_token: &str) -> String {
    let request = Request::builder()
        .method("POST")
        .uri("/api/admin/invites")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", admin_token))
        .body(Body::from(r#"{"count":1}"#))
        .unwrap()
        .into_parts();

    let mut request = Request::from_parts(request.0, request.1);
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 12345))));

    let response = app
        .router
        .clone()
        .oneshot(request)
        .await
        .unwrap();

    let status = response.status();
    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();

    if status != axum::http::StatusCode::OK {
        let body_text = String::from_utf8_lossy(&body_bytes);
        eprintln!("Create invite code failed with status {:?}: {}", status, body_text);
    }

    assert_eq!(status, axum::http::StatusCode::OK);

    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    body_json["codes"][0]["code"].as_str().unwrap().to_string()
}

pub async fn create_test_sketch(app: &TestApp, token: &str, name: &str) -> Uuid {
    let body = serde_json::json!({
        "name": name,
        "description": "Test sketch description"
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/sketches")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::from(body.to_string()))
        .unwrap()
        .into_parts();

    let mut request = Request::from_parts(request.0, request.1);
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 12345))));

    let response = app
        .router
        .clone()
        .oneshot(request)
        .await
        .unwrap();

    assert_eq!(response.status(), axum::http::StatusCode::CREATED);

    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    Uuid::parse_str(body_json["id"].as_str().unwrap()).unwrap()
}

pub async fn create_test_route(app: &TestApp, token: &str, sketch_id: Uuid, name: &str) -> Uuid {
    let body = serde_json::json!({
        "name": name,
        "description": "Test route description",
        "geojson": {
            "type": "FeatureCollection",
            "features": [
                {
                    "type": "Feature",
                    "geometry": {
                        "type": "LineString",
                        "coordinates": [[0.0, 0.0], [1.0, 1.0]]
                    },
                    "properties": {}
                }
            ]
        },
        "metadata": {
            "distance": 1000.0,
            "activity": "hiking"
        },
        "notes": "Test notes"
    });

    let request = Request::builder()
        .method("POST")
        .uri(format!("/api/sketches/{}/routes", sketch_id))
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token))
        .body(Body::from(body.to_string()))
        .unwrap()
        .into_parts();

    let mut request = Request::from_parts(request.0, request.1);
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 12345))));

    let response = app
        .router
        .clone()
        .oneshot(request)
        .await
        .unwrap();

    assert_eq!(response.status(), axum::http::StatusCode::CREATED);

    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    Uuid::parse_str(body_json["id"].as_str().unwrap()).unwrap()
}

pub async fn send_request<T: Serialize>(
    app: &TestApp,
    method: &str,
    uri: &str,
    token: Option<&str>,
    body: Option<T>,
) -> (axum::http::StatusCode, serde_json::Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");

    if let Some(t) = token {
        builder = builder.header("authorization", format!("Bearer {}", t));
    }

    let request_body = match body {
        Some(b) => Body::from(serde_json::to_string(&b).unwrap()),
        None => Body::empty(),
    };

    let request = builder.body(request_body).unwrap().into_parts();
    let mut request = Request::from_parts(request.0, request.1);
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 12345))));

    let response = app
        .router
        .clone()
        .oneshot(request)
        .await
        .unwrap();

    let status = response.status();
    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();

    let body_json = if body_bytes.is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_slice(&body_bytes).unwrap_or_else(|_| {
            serde_json::json!({ "raw": String::from_utf8_lossy(&body_bytes).to_string() })
        })
    };

    (status, body_json)
}
