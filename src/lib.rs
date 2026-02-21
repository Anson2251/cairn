pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod export;
pub mod invite;
pub mod middleware;
pub mod routes;
pub mod sharing;
pub mod sketches;
pub mod sync;

pub use auth::jwt::JwtManager;
pub use config::AppConfig;
pub use db::{create_database_if_not_exists, create_pool, run_migrations};

use axum::{
    routing::{delete, get, post},
    Router,
};
use redis::aio::ConnectionManager;
use sqlx::PgPool;
use std::sync::Arc;
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    limit::RequestBodyLimitLayer,
    trace::TraceLayer,
};

use middleware::{
    admin::admin_middleware,
    auth::auth_middleware,
    rate_limit::rate_limit_middleware,
};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub redis: ConnectionManager,
    pub jwt: Arc<JwtManager>,
    pub config: AppConfig,
}

pub fn create_router(state: Arc<AppState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let auth_routes = Router::new()
        .route("/register", post(auth::handlers::register))
        .route("/login", post(auth::handlers::login))
        .route("/logout", post(auth::handlers::logout))
        .route("/refresh", post(auth::handlers::refresh))
        .layer(axum::middleware::from_fn_with_state(state.clone(), rate_limit_middleware));

    let protected_auth_routes = Router::new()
        .route("/me", get(auth::handlers::me))
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth_middleware))
        .layer(axum::middleware::from_fn_with_state(state.clone(), rate_limit_middleware));

    let protected = Router::new()
        .route("/api/sketches", get(sketches::handlers::list_sketches).post(sketches::handlers::create_sketch))
        .route("/api/sketches/{id}", get(sketches::handlers::get_sketch).put(sketches::handlers::update_sketch).delete(sketches::handlers::delete_sketch))
        .route("/api/sketches/{id}/routes", get(routes::handlers::list_routes).post(routes::handlers::create_route))
        .route("/api/routes/{id}", get(routes::handlers::get_route).put(routes::handlers::update_route).delete(routes::handlers::delete_route))
        .route("/api/sync/push", post(sync::handlers::push))
        .route("/api/sync/pull", post(sync::handlers::pull))
        .route("/api/sync/resolve/{route_id}", post(sync::handlers::resolve))
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth_middleware))
        .layer(axum::middleware::from_fn_with_state(state.clone(), rate_limit_middleware));

    let invite_routes = Router::new()
        .route("/{code}/validate", get(invite::handlers::validate_invite))
        .layer(axum::middleware::from_fn_with_state(state.clone(), rate_limit_middleware));

    let admin_routes = Router::new()
        .route("/invites", get(invite::handlers::list_invites).post(invite::handlers::create_invites))
        .route("/invites/{id}", delete(invite::handlers::revoke_invite))
        .layer(axum::middleware::from_fn_with_state(state.clone(), admin_middleware))
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth_middleware))
        .layer(axum::middleware::from_fn_with_state(state.clone(), rate_limit_middleware));

    Router::new()
        .nest("/api/auth", auth_routes)
        .nest("/api/auth", protected_auth_routes)
        .merge(protected)
        .nest("/api/invite", invite_routes)
        .nest("/api/admin", admin_routes)
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .layer(RequestBodyLimitLayer::new(10 * 1024 * 1024)) // 10MB limit
        .layer(cors)
        .with_state(state)
}
