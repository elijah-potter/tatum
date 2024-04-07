use std::path::PathBuf;

use axum::{
    extract::{
        ws::{Message, WebSocket},
        Query, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use notify::{Config, RecommendedWatcher, Watcher};
use resolve_path::PathResolveExt;
use serde::Deserialize;
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct WatchParams {
    /// The path to watch for changes.
    path: PathBuf,
}

/// A WebSocket endpoint that watches files for changes and notifies the client when they occur.
pub async fn watch(ws: WebSocketUpgrade, Query(params): Query<WatchParams>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws(socket, params))
}

async fn handle_ws(mut socket: WebSocket, WatchParams { path }: WatchParams) {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    let path = path.resolve();

    let mut watcher =
        RecommendedWatcher::new(move |event| tx.send(event).unwrap(), Config::default()).unwrap();

    watcher
        .watch(&path, notify::RecursiveMode::NonRecursive)
        .unwrap();

    while let Some(_event) = rx.recv().await {
        info!("Received file change event for {}", path.to_string_lossy());

        socket.send(Message::Text("".to_string())).await.unwrap();
    }
}
