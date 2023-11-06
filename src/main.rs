use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    http::Response,
    response::{Html, IntoResponse},
    routing::get,
    Router, Server,
};
use serde::{Deserialize, Serialize};
use serde_json;
use std::time;
use sysinfo::{CpuExt, System, SystemExt};
use tokio::sync::broadcast;

type Snapshot = Vec<f32>;

#[tokio::main]
async fn main() {
    let (cpus_broadcast, _) = broadcast::channel::<Snapshot>(1);
    let (ram_broadcast, _) = broadcast::channel::<MemState>(1);

    tracing_subscriber::fmt::init();

    let app_state = AppState {
        cpus_broadcast: cpus_broadcast.clone(),
        ram_broadcast: ram_broadcast.clone(),
    };

    let router = Router::new()
        .route("/", get(root_get))
        .route("/index.mjs", get(indexmjs_get))
        .route("/index.css", get(indexcss_get))
        .route("/realtime/cpus", get(realtime_cpus_get))
        .route("/realtime/ram", get(realtime_ram_get))
        .with_state(app_state.clone());
    let mut sys = System::new();

    // Update CPU usage in the background
    tokio::task::spawn_blocking(move || loop {
        sys.refresh_cpu();
        sys.refresh_memory();
        let v: Vec<_> = sys.cpus().iter().map(|cpu| cpu.cpu_usage()).collect();
        let memory: MemState = MemState {
            total: sys.total_memory(),
            used: sys.used_memory(),
        };
        let _ = cpus_broadcast.send(v);
        let _ = ram_broadcast.send(memory);
        std::thread::sleep(System::MINIMUM_CPU_UPDATE_INTERVAL * 3);
    });
    let server = Server::bind(&"0.0.0.0:7032".parse().unwrap()).serve(router.into_make_service());

    let addr = server.local_addr();
    println!("Listening on {addr}");

    server.await.unwrap();
}

#[derive(Clone)]
struct AppState {
    cpus_broadcast: broadcast::Sender<Snapshot>,
    ram_broadcast: broadcast::Sender<MemState>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct MemState {
    total: u64,
    used: u64,
}

#[axum::debug_handler]
async fn root_get() -> impl IntoResponse {
    let markup = tokio::fs::read_to_string("src/index.html").await.unwrap();

    Html(markup)
}

#[axum::debug_handler]
async fn indexmjs_get() -> impl IntoResponse {
    let markup = tokio::fs::read_to_string("src/index.mjs").await.unwrap();

    Response::builder()
        .header("content-type", "application/javascript;charset=utf-8")
        .body(markup)
        .unwrap()
}

#[axum::debug_handler]
async fn indexcss_get() -> impl IntoResponse {
    let markup = tokio::fs::read_to_string("src/index.css").await.unwrap();

    Response::builder()
        .header("content-type", "text/css;charset=utf-8")
        .body(markup)
        .unwrap()
}

#[axum::debug_handler]
async fn realtime_cpus_get(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|ws: WebSocket| async { realtime_cpus_stream(state, ws).await })
}

async fn realtime_cpus_stream(app_state: AppState, mut ws: WebSocket) {
    let mut rx = app_state.cpus_broadcast.subscribe();

    while let Ok(msg) = rx.recv().await {
        ws.send(Message::Text(serde_json::to_string(&msg).unwrap()))
            .await
            .unwrap();
    }
}

#[axum::debug_handler]
async fn realtime_ram_get(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|ws: WebSocket| async { realtime_ram_stream(state, ws).await })
}

async fn realtime_ram_stream(app_state: AppState, mut ws: WebSocket) {
    let mut rx = app_state.ram_broadcast.subscribe();

    while let Ok(msg) = rx.recv().await {
        ws.send(Message::Text(serde_json::to_string(&msg).unwrap()))
            .await
            .unwrap();
    }
}
