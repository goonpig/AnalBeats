use std::{
    collections::HashMap,
    fs,
    net::SocketAddr,
    path::Path,
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result};
use axum::{
    extract::{Query, State},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tokio_tungstenite::connect_async;
use tower_http::services::ServeDir;

use buttplug_client::{
    connector::ButtplugRemoteClientConnector,
    device::{ButtplugClientDevice, ClientDeviceOutputCommand},
    serializer::ButtplugClientJSONSerializer,
    ButtplugClient,
};
use buttplug_transport_websocket_tungstenite::ButtplugWebsocketClientTransport;

const CONFIG_PATH: &str = "config.json";
const DATAPULLER_DEFAULT_URL: &str = "ws://127.0.0.1:2946/BSDataPuller/LiveData";
const INTIFACE_URL: &str = "ws://127.0.0.1:12345";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ReactionMode {
    Vibrate,
    Oscillate,
    Rotate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReactionConfig {
    enabled: bool,
    toy_name: Option<String>,
    toy_index: Option<u32>,
    mode: ReactionMode,
    intensity: f64,
    duration_ms: u64,
    cooldown_ms: u64,
}

impl Default for ReactionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            toy_name: None,
            toy_index: None,
            mode: ReactionMode::Vibrate,
            intensity: 0.5,
            duration_ms: 120,
            cooldown_ms: 60,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppConfig {
    datapuller_url: String,
    reactions: HashMap<String, ReactionConfig>,
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut reactions = HashMap::new();

        reactions.insert(
            "hit".to_string(),
            ReactionConfig {
                enabled: true,
                toy_name: None,
                toy_index: None,
                mode: ReactionMode::Vibrate,
                intensity: 0.75,
                duration_ms: 45,
                cooldown_ms: 0,
            },
        );

        reactions.insert(
            "miss".to_string(),
            ReactionConfig {
                enabled: true,
                toy_name: None,
                toy_index: None,
                mode: ReactionMode::Oscillate,
                intensity: 0.35,
                duration_ms: 180,
                cooldown_ms: 80,
            },
        );

        Self {
            datapuller_url: DATAPULLER_DEFAULT_URL.to_string(),
            reactions,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct DeviceInfo {
    name: String,
    index: u32,
}

#[derive(Clone)]
struct AppState {
    config: Arc<RwLock<AppConfig>>,
    bp: Arc<ButtplugClient>,
    cooldowns: Arc<RwLock<HashMap<String, std::time::Instant>>>,
}

#[derive(Debug, Deserialize)]
struct TestQuery {
    reaction: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = load_or_create_config(CONFIG_PATH)?;
    let state = AppState {
        config: Arc::new(RwLock::new(cfg)),
        bp: Arc::new(ButtplugClient::new("Analbeats v0.6")),
        cooldowns: Arc::new(RwLock::new(HashMap::new())),
    };

    let connector = ButtplugRemoteClientConnector::<
        ButtplugWebsocketClientTransport,
        ButtplugClientJSONSerializer,
    >::new(ButtplugWebsocketClientTransport::new_insecure_connector(
        INTIFACE_URL,
    ));

    state
        .bp
        .connect(connector)
        .await
        .with_context(|| format!("Failed to connect Intiface at {INTIFACE_URL}"))?;
    println!("Connected to Intiface at {INTIFACE_URL}");

    tokio::spawn(datapuller_loop(state.clone()));

    let app = Router::new()
        .route("/api/status", get(api_status))
        .route("/api/devices", get(api_devices))
        .route("/api/config", get(api_get_config).post(api_set_config))
        .route("/api/test", post(api_test))
        .nest_service("/", ServeDir::new("frontend"))
        .with_state(state);

    let addr: SocketAddr = "127.0.0.1:3030".parse().unwrap();
    println!("Dashboard: http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn api_status(State(state): State<AppState>) -> impl IntoResponse {
    let cfg = state.config.read().await.clone();
    Json(serde_json::json!({
        "ok": true,
        "intiface_url": INTIFACE_URL,
        "datapuller_url": cfg.datapuller_url,
        "device_count": state.bp.devices().len()
    }))
}

async fn api_devices(State(state): State<AppState>) -> impl IntoResponse {
    let devices = state
        .bp
        .devices()
        .iter()
        .map(|(idx, dev)| DeviceInfo {
            name: dev.name().to_string(),
            index: *idx,
        })
        .collect::<Vec<_>>();
    Json(devices)
}

async fn api_get_config(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.config.read().await.clone())
}

async fn api_set_config(
    State(state): State<AppState>,
    Json(new_cfg): Json<AppConfig>,
) -> impl IntoResponse {
    // strict validation: each enabled reaction must have toy_index selected
    for (k, r) in &new_cfg.reactions {
        if r.enabled && r.toy_index.is_none() {
            return Json(serde_json::json!({
                "ok": false,
                "error": format!("Reaction '{k}' is enabled but no toy is selected")
            }));
        }
    }

    {
        let mut cfg = state.config.write().await;
        *cfg = new_cfg.clone();
    }

    match save_config(CONFIG_PATH, &new_cfg) {
        Ok(_) => Json(serde_json::json!({ "ok": true })),
        Err(e) => Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
    }
}

async fn api_test(State(state): State<AppState>, Query(q): Query<TestQuery>) -> impl IntoResponse {
    let reaction = q.reaction.to_lowercase();
    match trigger_reaction(state, &reaction).await {
        Ok(_) => Json(serde_json::json!({ "ok": true, "reaction": reaction })),
        Err(e) => Json(serde_json::json!({ "ok": false, "reaction": reaction, "error": e.to_string() })),
    }
}

async fn datapuller_loop(state: AppState) {
    let mut prev_misses: i32 = 0;
    let mut prev_event_trigger: i64 = -1;
    let mut prev_score: i64 = 0;
    let mut prev_combo: i64 = 0;
    let mut prev_notes_spawned: i64 = 0;
    let mut last_hit_at = std::time::Instant::now() - Duration::from_secs(10);

    loop {
        let ws_url = {
            let cfg = state.config.read().await;
            cfg.datapuller_url.clone()
        };

        println!("Connecting DataPuller -> {ws_url}");

        match connect_async(&ws_url).await {
            Ok((ws_stream, _)) => {
                println!("DataPuller connected.");
                let (_write, mut read) = ws_stream.split();

                while let Some(msg) = read.next().await {
                    let Ok(msg) = msg else { continue };
                    if !msg.is_text() { continue; }

                    let Ok(text) = msg.to_text() else { continue };
                    println!("DataPuller raw: {}", text);

                    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else { continue };

                    let misses = v.get("Misses").and_then(|x| x.as_i64()).unwrap_or(0) as i32;
                    let health = v.get("PlayerHealth").and_then(|x| x.as_f64()).unwrap_or(100.0);
                    let event_trigger = v.get("EventTrigger").and_then(|x| x.as_i64()).unwrap_or(0);

                    let score = v.get("Score").and_then(|x| x.as_i64()).unwrap_or(0);
                    let combo = v.get("Combo").and_then(|x| x.as_i64()).unwrap_or(0);
                    let notes_spawned = v.get("NotesSpawned").and_then(|x| x.as_i64()).unwrap_or(0);

                    if misses > prev_misses {
                        let _ = trigger_reaction(state.clone(), "miss").await;
                    }

                    let event_changed = event_trigger != prev_event_trigger;
                    let non_zero_event = event_trigger > 0;
                    let forward_progress =
                        score > prev_score || combo > prev_combo || notes_spawned > prev_notes_spawned;

                    let hit_candidate = event_changed && non_zero_event && forward_progress;
                    if hit_candidate && last_hit_at.elapsed() >= Duration::from_millis(22) {
                        let _ = trigger_reaction(state.clone(), "hit").await;
                        last_hit_at = std::time::Instant::now();
                    }

                    if health <= 0.0 {
                        let _ = trigger_reaction(state.clone(), "miss").await;
                    }

                    prev_misses = misses;
                    prev_event_trigger = event_trigger;
                    prev_score = score;
                    prev_combo = combo;
                    prev_notes_spawned = notes_spawned;
                }

                println!("DataPuller disconnected, reconnecting...");
            }
            Err(e) => eprintln!("DataPuller connect failed: {e}"),
        }

        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

async fn trigger_reaction(state: AppState, key: &str) -> Result<()> {
    let cfg = state.config.read().await.clone();
    let rc = cfg
        .reactions
        .get(key)
        .cloned()
        .with_context(|| format!("Reaction not configured: {key}"))?;

    if !rc.enabled {
        return Ok(());
    }

    if !check_cooldown(&state, key, rc.cooldown_ms).await {
        return Ok(());
    }

    println!(
        "Trigger '{}' using toy_index={:?}, toy_name={:?}",
        key, rc.toy_index, rc.toy_name
    );

    let device = find_target_device(&state.bp, rc.toy_index)
        .with_context(|| format!("Selected toy for '{key}' is missing or not connected"))?;

    run_reaction(device, rc).await
}

async fn check_cooldown(state: &AppState, key: &str, cooldown_ms: u64) -> bool {
    if cooldown_ms == 0 {
        return true;
    }

    let now = std::time::Instant::now();
    let mut map = state.cooldowns.write().await;
    let allow = match map.get(key) {
        Some(prev) => now.duration_since(*prev).as_millis() >= cooldown_ms as u128,
        None => true,
    };
    if allow {
        map.insert(key.to_string(), now);
    }
    allow
}

fn find_target_device(
    client: &ButtplugClient,
    toy_index: Option<u32>,
) -> Option<Arc<ButtplugClientDevice>> {
    let idx = toy_index?;
    client.devices().get(&idx).map(|d| d.clone().into())
}

async fn run_reaction(device: Arc<ButtplugClientDevice>, rc: ReactionConfig) -> Result<()> {
    let intensity = rc.intensity.clamp(0.0, 1.0);
    let duration = Duration::from_millis(rc.duration_ms);

    match rc.mode {
        ReactionMode::Vibrate => {
            device
                .run_output(&ClientDeviceOutputCommand::Vibrate(intensity.into()))
                .await
                .with_context(|| format!("vibrate failed on '{}'", device.name()))?;
        }
        ReactionMode::Oscillate => {
            let osc_res = device
                .run_output(&ClientDeviceOutputCommand::Oscillate(intensity.into()))
                .await;

            if osc_res.is_err() {
                return Err(anyhow::anyhow!(
                    "oscillate unsupported on '{}'",
                    device.name()
                ));
            }
        }
        ReactionMode::Rotate => {
            let rot_res = device
                .run_output(&ClientDeviceOutputCommand::Rotate(intensity.into()))
                .await;

            if rot_res.is_err() {
                return Err(anyhow::anyhow!(
                    "rotate unsupported on '{}'",
                    device.name()
                ));
            }
        }
    }

    tokio::time::sleep(duration).await;
    let _ = device.stop().await;
    Ok(())
}

fn load_or_create_config(path: &str) -> Result<AppConfig> {
    if Path::new(path).exists() {
        let txt = fs::read_to_string(path)?;
        let cfg = serde_json::from_str::<AppConfig>(&txt)?;
        Ok(cfg)
    } else {
        let cfg = AppConfig::default();
        save_config(path, &cfg)?;
        Ok(cfg)
    }
}

fn save_config(path: &str, cfg: &AppConfig) -> Result<()> {
    let txt = serde_json::to_string_pretty(cfg)?;
    fs::write(path, txt)?;
    Ok(())
}
