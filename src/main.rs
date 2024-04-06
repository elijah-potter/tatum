mod page_template;
mod svg_template;

use askama::Template;
use base64::{engine::general_purpose, Engine};
use notify::{Config, RecommendedWatcher, Watcher};
use page_template::PageTemplate;
use pulldown_cmark::{CowStr, Event, LinkType, Tag};
use resolve_path::PathResolveExt;
use std::{
    env::args,
    path::{Path, PathBuf},
};
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

use crate::svg_template::SvgTemplate;

#[tokio::main]
async fn main() {
    let silent = args().into_iter().any(|v| v == "-q");

    if !silent {
        tracing_subscriber::fmt::init();
    }

    let app = Router::new()
        .route("/", get(get_rendered))
        .route("/watch", get(websocket_notifier));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();

    if silent {
        println!("{}", listener.local_addr().unwrap());
    } else {
        info!("Listening on {}", listener.local_addr().unwrap());
    }

    axum::serve(listener, app).await.unwrap();
}

#[derive(Debug, Deserialize)]
struct RenderParams {
    path: PathBuf,
}

async fn get_rendered(Query(RenderParams { path }): Query<RenderParams>) -> Html<String> {
    info!("Rendering document {}", path.to_string_lossy());

    Html(render_doc(path.resolve()).await.unwrap())
}

fn data_url(data: &[u8], mime_type: &str) -> String {
    let encoded = general_purpose::STANDARD.encode(data);

    format!("data:{};base64,{encoded}", mime_type)
}

/// Gets the file at a specified path, loads it, and converts it to a base64-encoded data URL
async fn path_to_data_url(path: impl AsRef<Path>) -> anyhow::Result<String> {
    let file = read(&path).await?;

    Ok(data_url(
        &file,
        mime_guess::from_path(&path)
            .first_raw()
            .unwrap_or("text/plain"),
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

            let resolved = image_path.resolve_in(&path);

            info!("Loading image {}", resolved.to_string_lossy());

            if let Ok(data_url) = path_to_data_url(resolved).await {
                *dest_url = CowStr::from(data_url);
            } else {
                *dest_url = CowStr::from(data_url(
                    SvgTemplate {
                        fill: "red".to_string(),
                        text: "Unable to embed image.".to_string(),
                    }
                    .to_string()
                    .as_bytes(),
                    "image/svg+xml",
                ))
            }
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
