mod page_template;
mod render;
mod routes;
mod svg_template;

use clap::{command, Parser};
use routes::construct_router;
use tracing::info;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Whether to print logs.
    /// If true, Tatum will exclusively print out the `address:port` of the listening server once it starts.
    #[arg(short, long, default_value_t = false)]
    quiet: bool,

    #[arg(short, long, default_value_t = 0)]
    port: u16,

    #[arg(short, long, default_value_t = ("127.0.0.1").to_string())]
    address: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if !args.quiet {
        tracing_subscriber::fmt::init();
    }

    let app = construct_router();

    let listener = tokio::net::TcpListener::bind((args.address, args.port))
        .await
        .unwrap();

    if args.quiet {
        println!("{}", listener.local_addr().unwrap());
    } else {
        info!("Listening on {}", listener.local_addr().unwrap());
    }

    axum::serve(listener, app).await.unwrap();
}
