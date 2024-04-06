mod page_template;

use askama::Template;
use base64::{engine::general_purpose, Engine};
use notify::{Config, RecommendedWatcher, Watcher};
use page_template::PageTemplate;
use pulldown_cmark::{CowStr, Event, LinkType, Tag};
use resolve_path::PathResolveExt;
use std::path::{Path, PathBuf};
use tracing::info;

use axum::{
    extract::{
        ws::{Message, WebSocket},
        Query, WebSocketUpgrade,
    },
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use serde::Deserialize;
use tokio::fs::{read, read_to_string};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = Router::new()
        .route("/", get(get_rendered))
        .route("/watch", get(websocket_notifier));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(Debug, Deserialize)]
struct RenderParams {
    path: PathBuf,
}

async fn get_rendered(Query(RenderParams { path }): Query<RenderParams>) -> Html<String> {
    Html(render_doc(path.resolve()).await.unwrap())
}

/// Gets the file at a specified path, loads it, and converts it to base64
async fn path_to_data_url(path: impl AsRef<Path>) -> anyhow::Result<String> {
    let file = read(&path).await?;
    let encoded = general_purpose::STANDARD.encode(file);

    Ok(format!(
        "data:{};base64,{encoded}",
        mime_guess::from_path(&path)
            .first()
            .map(|m| m.to_string())
            .unwrap_or("text/plain".to_string())
    ))
}

async fn render_doc(path: impl AsRef<Path>) -> anyhow::Result<String> {
    let file = read_to_string(path.as_ref()).await?;

    let options = pulldown_cmark::Options::all();

    let parser = pulldown_cmark::Parser::new_ext(file.as_str(), options);

    let mut events: Vec<_> = parser.collect();

    // Convert image links to base64
    for event in events.iter_mut() {
        if let Event::Start(Tag::Image {
            link_type: LinkType::Inline,
            dest_url,
            ..
        }) = event
        {
            let image_path: PathBuf = dest_url.parse()?;

            *dest_url = CowStr::from(path_to_data_url(image_path.resolve_in(&path)).await?);
        }
    }

    let mut body = String::new();
    pulldown_cmark::html::push_html(&mut body, events.into_iter());

    let template = PageTemplate {
        body,
        title: path.as_ref().as_os_str().to_string_lossy().to_string(),
    };

    Ok(template.render().unwrap())
}

#[derive(Debug, Deserialize)]
struct NotifParams {
    path: PathBuf,
}

/// A WebSocket endpoint that watches files for changes and notifies the client when they occur.
async fn websocket_notifier(
    ws: WebSocketUpgrade,
    Query(params): Query<NotifParams>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws(socket, params))
}

async fn handle_ws(mut socket: WebSocket, NotifParams { path }: NotifParams) {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    let path = path.resolve();

    let mut watcher =
        RecommendedWatcher::new(move |event| tx.send(event).unwrap(), Config::default()).unwrap();

    watcher
        .watch(&path, notify::RecursiveMode::NonRecursive)
        .unwrap();

    while let Some(event) = rx.recv().await {
        info!("Received file change event for {}", path.to_string_lossy());

        socket.send(Message::Text("".to_string())).await.unwrap();
    }
}
