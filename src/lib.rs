pub mod assets;
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
pub mod admin;

pub use auth::jwt::JwtManager;
pub use config::AppConfig;
pub use db::{create_database_if_not_exists, create_pool, run_migrations};

use axum::{
    routing::{get, post, put},
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
        .route("/logout", get(auth::handlers::logout).post(auth::handlers::logout))
        .route("/refresh", post(auth::handlers::refresh))
        .layer(axum::middleware::from_fn_with_state(state.clone(), rate_limit_middleware));

    let protected_auth_routes = Router::new()
        .route("/me", get(auth::handlers::me).put(auth::handlers::update_profile))
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth_middleware))
        .layer(axum::middleware::from_fn_with_state(state.clone(), rate_limit_middleware));

    let protected = Router::new()
        .route("/api/sketches", get(sketches::handlers::list_sketches).post(sketches::handlers::create_sketch))
        .route("/api/sketches/shared", get(sketches::handlers::list_shared_sketches))
        .route("/api/sketches/{id}", get(sketches::handlers::get_sketch).put(sketches::handlers::update_sketch).delete(sketches::handlers::delete_sketch))
        .route("/api/sketches/{id}/shares", get(sharing::handlers::list_shares).post(sharing::handlers::create_share))
        .route("/api/sketches/{id}/shares/{user_id}", put(sharing::handlers::update_share).delete(sharing::handlers::delete_share))
        .route("/api/sketches/{id}/public-link", post(sharing::handlers::create_public_link).delete(sharing::handlers::delete_public_link))
        .route("/api/sketches/{id}/routes", get(routes::handlers::list_routes).post(routes::handlers::create_route))
        .route("/api/routes/{id}", get(routes::handlers::get_route).put(routes::handlers::update_route).delete(routes::handlers::delete_route))
        .route("/api/sync/push", post(sync::handlers::push))
        .route("/api/sync/pull", post(sync::handlers::pull))
        .route("/api/sync/resolve/{route_id}", post(sync::handlers::resolve))
        .route("/api/assets", post(assets::handlers::upload))
        .route("/assets/{filename}", get(assets::handlers::serve))
        .route("/api/export", post(export::handlers::create_export))
        .route("/api/export/{job_id}", get(export::handlers::get_export_status))
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth_middleware))
        .layer(axum::middleware::from_fn_with_state(state.clone(), rate_limit_middleware));

    let public_routes = Router::new()
        .route("/api/public/{token}", get(sharing::handlers::get_public_sketch))
        .route("/api/trailblazers", get(invite::handlers::list_trailblazers));

    let invite_routes = Router::new()
        .route("/{code}/validate", get(invite::handlers::validate_invite))
        .layer(axum::middleware::from_fn_with_state(state.clone(), rate_limit_middleware));

    let admin_routes = Router::new()
        .route("/invites", get(invite::handlers::list_invites).post(invite::handlers::create_invites))
        .route("/invites/{id}", get(invite::handlers::get_invite).put(invite::handlers::update_invite).delete(invite::handlers::revoke_invite))
        .route("/users", get(admin::handlers::list_users).post(admin::handlers::create_user))
        .route("/users/{id}", get(admin::handlers::get_user).put(admin::handlers::update_user).delete(admin::handlers::delete_user))
        .layer(axum::middleware::from_fn_with_state(state.clone(), admin_middleware))
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth_middleware))
        .layer(axum::middleware::from_fn_with_state(state.clone(), rate_limit_middleware));

    let html_admin_routes = admin::create_admin_router(state.clone());

    Router::new()
        .merge(public_routes)
        .nest("/api/auth", auth_routes)
        .nest("/api/auth", protected_auth_routes)
        .merge(protected)
        .nest("/api/invite", invite_routes)
        .nest("/api/admin", admin_routes)
        .nest("/admin", html_admin_routes)
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .layer(RequestBodyLimitLayer::new(10 * 1024 * 1024)) // 10MB limit
        .layer(cors)
        .with_state(state)
}
