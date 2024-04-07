mod page_template;
mod render;
mod routes;
mod svg_template;

use routes::construct_router;
use std::env::args;
use tracing::info;

#[tokio::main]
async fn main() {
    let silent = args().into_iter().any(|v| v == "-q");

    if !silent {
        tracing_subscriber::fmt::init();
    }

    let app = construct_router();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();

    if silent {
        println!("{}", listener.local_addr().unwrap());
    } else {
        info!("Listening on {}", listener.local_addr().unwrap());
    }

    axum::serve(listener, app).await.unwrap();
}
