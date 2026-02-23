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
        .route("/invites/new", get(handlers::new_invite_form))
        .route("/invites/create", post(handlers::create_invites))
        .route("/invites/{id}", get(handlers::edit_invite_form))
        .route("/invites/{id}/edit", post(handlers::update_invite_form))
        .route("/invites/{id}/delete", post(handlers::delete_invite_form))
        .route("/users", get(handlers::users_list))
        .route("/users/new", get(handlers::new_user_form).post(handlers::create_user_form))
        .route("/users/{id}", get(handlers::edit_user_form))
        .route("/users/{id}/edit", post(handlers::update_user_form))
        .route("/users/{id}/delete", post(handlers::delete_user_form))
        .route("/stats", get(handlers::stats))
        .layer(axum::middleware::from_fn_with_state(state.clone(), admin_middleware))
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth_middleware_with_cookie_redirect));

    public_routes
        .merge(protected_routes)
        .with_state(state)
}
