pub mod handlers;
pub mod templates;

use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

use crate::middleware::{
    admin::admin_middleware,
    auth::{auth_middleware_with_cookie_redirect, optional_auth_middleware_with_cookie},
};
use crate::AppState;

pub fn create_admin_router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    let public_routes = Router::new()
        .route("/login", get(handlers::login_page).post(handlers::login))
        .route("/logout", post(handlers::logout))
        .route("/", get(handlers::admin_index))
        .layer(axum::middleware::from_fn_with_state(state.clone(), optional_auth_middleware_with_cookie));

    let protected_routes = Router::new()
        .route("/dashboard", get(handlers::admin_page))
        .route("/invites", get(handlers::invites))
        .route("/invites/create", post(handlers::create_invites))
        .route("/invites/revoke", post(handlers::revoke_invite))
        .route("/users", get(handlers::users_list))
        .route("/stats", get(handlers::stats))
        .layer(axum::middleware::from_fn_with_state(state.clone(), admin_middleware))
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth_middleware_with_cookie_redirect));

    public_routes
        .merge(protected_routes)
        .with_state(state)
}
