use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
    routing::get,
    Router, Server,
};
use serde::{Deserialize, Serialize};
use serde_json;
use sysinfo::{ComponentExt, CpuExt, ProcessExt, System, SystemExt};
use tokio::sync::broadcast;

#[derive(Clone)]
struct AppState {
    cpus_broadcast: broadcast::Sender<CpuState>,
    ram_broadcast: broadcast::Sender<MemState>,
    process_broadcast: broadcast::Sender<Vec<ProcessInfo>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ProcessInfo {
    name: String,
    cpu_usage: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct CpuState {
    cores: Vec<CpuCore>,
    temp: f32,
    core_temp: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct CpuCore {
    usage: f32,
    temp: Option<f32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct MemState {
    total: u64,
    used: u64,
}

#[tokio::main]
async fn main() {
    let (cpus_broadcast, _) = broadcast::channel::<CpuState>(1);
    let (ram_broadcast, _) = broadcast::channel::<MemState>(1);
    let (process_broadcast, _) = broadcast::channel::<Vec<ProcessInfo>>(1);

    tracing_subscriber::fmt::init();

    let app_state = AppState {
        cpus_broadcast: cpus_broadcast.clone(),
        ram_broadcast: ram_broadcast.clone(),
        process_broadcast: process_broadcast.clone(),
    };

    let router = Router::new()
        .route("/realtime/cpus", get(realtime_cpus_get))
        .route("/realtime/ram", get(realtime_ram_get))
        .route("/realtime/processes", get(realtime_process_get))
        .with_state(app_state.clone());

    let mut sys = System::new_all();
    let mut send_less_freq = 0;

    tokio::task::spawn_blocking(move || loop {
        sys.refresh_cpu();
        if send_less_freq == 0 {
            sys.refresh_memory();
            sys.refresh_processes();

            let memory_state: MemState = MemState {
                total: sys.total_memory(),
                used: sys.used_memory(),
            };

            let mut processes: Vec<ProcessInfo> = sys
                .processes()
                .values()
                .map(|proc| ProcessInfo {
                    name: proc.name().to_string(),
                    cpu_usage: proc.cpu_usage() as i32,
                })
                .collect();
            processes.sort_by_key(|proc_info| proc_info.cpu_usage);
            processes.reverse();
            processes.truncate(4);

            dbg!(&memory_state);
            dbg!(&processes);

            let _ = ram_broadcast.send(memory_state);
            let _ = process_broadcast.send(processes);
        }
        send_less_freq += 1;
        if send_less_freq == 5 {
            send_less_freq = 0;
        }
        sys.refresh_components();

        let mut cpu_state = CpuState {
            cores: vec![],
            temp: 0.,
            core_temp: false,
        };
        let cpu_usages: Vec<f32> = sys.cpus().iter().map(|cpu| cpu.cpu_usage()).collect();

        #[cfg(not(feature = "core_temp"))]
        {
            cpu_state.cores = cpu_usages
                .into_iter()
                .map(|core_us| CpuCore {
                    usage: core_us,
                    temp: None,
                })
                .collect();
        }

        #[cfg(feature = "core_temp")]
        {
            cpu_state.core_temp = true;
            let components = sys.components();
            for (i, core) in cpu_usages.into_iter().enumerate() {
                for component in components {
                    if component
                        .label()
                        .to_owned()
                        .contains(format!("coretemp Core {}", i).as_str())
                    {
                        cpu_state.cores.push(CpuCore {
                            usage: core,
                            temp: Some(component.temperature()),
                        });
                    }
                }
            }
        }

        for component in sys.components() {
            if component.label().contains("coretemp Package")
                || component.label().contains("cpu_thermal")
            {
                cpu_state.temp = component.temperature();
            }
        }

        dbg!(&cpu_state);

        let _ = cpus_broadcast.send(cpu_state);
        std::thread::sleep(System::MINIMUM_CPU_UPDATE_INTERVAL * 3);
    });
    let server = Server::bind(&"0.0.0.0:7032".parse().unwrap()).serve(router.into_make_service());

    let addr = server.local_addr();
    println!("Listening on {addr}");

    server.await.unwrap();
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

#[axum::debug_handler]
async fn realtime_process_get(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|ws: WebSocket| async { realtime_process_stream(state, ws).await })
}

async fn realtime_process_stream(app_state: AppState, mut ws: WebSocket) {
    let mut rx = app_state.process_broadcast.subscribe();

    while let Ok(msg) = rx.recv().await {
        ws.send(Message::Text(serde_json::to_string(&msg).unwrap()))
            .await
            .unwrap();
    }
}
