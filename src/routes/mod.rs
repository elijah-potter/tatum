use axum::{routing::get, Router};

mod index;
mod watch;
use index::index;
use watch::watch;

pub fn construct_router() -> Router {
    Router::new()
        .route("/", get(index))
        .route("/watch", get(watch))
}
