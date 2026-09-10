mod advisor;
mod catalog;
mod discover;
mod files;
mod inventory;
mod issues;
mod model;
mod remote;
mod store;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use sysinfo::{Pid, ProcessesToUpdate, System};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tower_http::services::ServeDir;

use crate::inventory::Inventory;
use crate::model::{
    Disk, Guest, Health, IssuesReport, NewNote, NewPool, NewShare, Note, Opportunity, Pool, Power,
    RepairRequest, RepairResult, Share, SystemSnapshot,
};
use crate::store::Store;

static STARTED: OnceLock<Instant> = OnceLock::new();
static IDLE: AtomicBool = AtomicBool::new(true);

#[derive(Clone)]
struct AppState {
    inventory: Inventory,
    store: Arc<Mutex<Store>>,
}

#[tokio::main]
async fn main() {
    STARTED.set(Instant::now()).ok();

    let port: u16 = std::env::var("KEEL_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(9478);

    let web = std::env::current_dir()
        .expect("cwd")
        .join("web");

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/system", get(system))
        .route("/api/disks", get(disks))
        .route("/api/pools", get(pools).post(pools_create))
        .route("/api/shares", get(shares).post(shares_create))
        .route("/api/power", get(power))
        .route("/api/power/idle", post(power_idle))
        .route("/api/power/active", post(power_active))
        .route("/api/power/cancel", post(power_active))
        .route("/api/power/opportunities", get(power_opportunities))
        .route("/api/catalog", get(catalog_list))
        .route("/api/issues", get(issues_list))
        .route("/api/repair", post(repair_run))
        .route("/api/notes", get(notes_list).post(notes_create))
        .route("/api/notes/{id}", axum::routing::put(notes_update).delete(notes_delete))
        .route("/api/look", post(look_around))
        .route("/api/look/login", post(look_login))
        .route("/api/look/saved", get(look_saved))
        .route("/api/look/forget", post(look_forget))
        .route("/api/look/session", get(look_session))
        .route("/api/look/files", get(look_files))
        .route("/api/look/browse", post(look_browse))
        .with_state(AppState {
            inventory: Inventory::detect(),
            store: Arc::new(Mutex::new(Store::load())),
        })
        .fallback_service(ServeDir::new(web));

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await.expect("bind");
    eprintln!("keel listening on http://127.0.0.1:{port}");
    axum::serve(listener, app).await.expect("serve");
}

async fn health() -> Json<Health> {
    Json(Health {
        ok: true,
        name: "keel",
        version: env!("CARGO_PKG_VERSION"),
        uptime_ms: STARTED.get().map(|t| t.elapsed().as_millis()).unwrap_or(0),
        inventory: Inventory::detect().label(),
    })
}

async fn system() -> ([(axum::http::header::HeaderName, HeaderValue); 1], Json<SystemSnapshot>) {
    let t0 = Instant::now();
    let snap = tokio::task::spawn_blocking(collect_system)
        .await
        .unwrap_or_else(|_| SystemSnapshot::empty());
    let ms = t0.elapsed();
    (timing_header(ms), Json(snap))
}

async fn disks(State(state): State<AppState>) -> ([(axum::http::header::HeaderName, HeaderValue); 1], Json<Vec<Disk>>) {
    let t0 = Instant::now();
    let list = state.inventory.disks().await;
    (timing_header(t0.elapsed()), Json(list))
}

async fn pools(State(state): State<AppState>) -> Json<Vec<Pool>> {
    Json(state.store.lock().await.pools.clone())
}

async fn pools_create(
    State(state): State<AppState>,
    Json(req): Json<NewPool>,
) -> Result<Json<Pool>, (StatusCode, String)> {
    state
        .store
        .lock()
        .await
        .add_pool(req)
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

async fn shares(State(state): State<AppState>) -> Json<Vec<Share>> {
    Json(state.store.lock().await.shares.clone())
}

async fn shares_create(
    State(state): State<AppState>,
    Json(req): Json<NewShare>,
) -> Result<Json<Share>, (StatusCode, String)> {
    state
        .store
        .lock()
        .await
        .add_share(req)
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

fn power_state() -> Power {
    let idle = IDLE.load(Ordering::Relaxed);
    Power {
        policy: "tailscale-floor",
        mode: if idle { "idle" } else { "active" },
        host_sleep_allowed: false,
        services_stopped: idle,
        tailscale_required: true,
        why: "Idle keeps the host on and Tailscale running so you can open files and the GUI when you are not at home. The rest of the catalog is installed, not running — like an OS with apps closed. keel stays up as the small control plane. Host S3 is off: that would kill Tailscale and the file path.",
    }
}

async fn power() -> Json<Power> {
    Json(power_state())
}

async fn power_idle() -> Json<Power> {
    IDLE.store(true, Ordering::Relaxed);
    Json(power_state())
}

async fn power_active() -> Json<Power> {
    IDLE.store(false, Ordering::Relaxed);
    Json(power_state())
}

async fn catalog_list() -> Json<Vec<Guest>> {
    let idle = IDLE.load(Ordering::Relaxed);
    Json(crate::catalog::guests(idle))
}

async fn issues_list(State(state): State<AppState>) -> Json<IssuesReport> {
    let idle = IDLE.load(Ordering::Relaxed);
    let store = state.store.lock().await;
    Json(crate::issues::report(
        idle,
        state.inventory,
        store.pools.len(),
        store.shares.len(),
    ))
}

async fn repair_run(Json(req): Json<RepairRequest>) -> Json<RepairResult> {
    Json(crate::issues::apply(&req.id))
}

async fn notes_list(State(state): State<AppState>) -> Json<Vec<Note>> {
    Json(state.store.lock().await.notes.clone())
}

async fn notes_create(
    State(state): State<AppState>,
    Json(req): Json<NewNote>,
) -> Result<Json<Note>, (StatusCode, String)> {
    state
        .store
        .lock()
        .await
        .add_note(req)
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

async fn notes_update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<NewNote>,
) -> Result<Json<Note>, (StatusCode, String)> {
    state
        .store
        .lock()
        .await
        .update_note(&id, req)
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

async fn notes_delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .store
        .lock()
        .await
        .delete_note(&id)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

async fn look_around(
    State(state): State<AppState>,
    Json(req): Json<crate::discover::LookRequest>,
) -> Result<Json<crate::discover::LookReport>, (StatusCode, String)> {
    let report = crate::discover::look(&req.host)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    state.store.lock().await.remember_host(&report.host, "", "");
    Ok(Json(report))
}

async fn look_saved(State(state): State<AppState>) -> Json<crate::remote::SavedRemotes> {
    Json(state.store.lock().await.saved_remote.clone())
}

async fn look_forget(
    State(state): State<AppState>,
    Json(req): Json<crate::remote::ForgetHost>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .store
        .lock()
        .await
        .forget_host(&req)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

async fn look_login(
    State(state): State<AppState>,
    Json(req): Json<crate::remote::LoginRequest>,
) -> Result<Json<crate::remote::LoginReport>, (StatusCode, String)> {
    let (report, session) = crate::remote::login(&req)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let mut store = state.store.lock().await;
    store.remember_remote(&req);
    store.live = session;
    Ok(Json(report))
}

async fn look_session(State(state): State<AppState>) -> Json<crate::files::SessionStatus> {
    Json(crate::files::session_status(
        state.store.lock().await.live.as_ref(),
    ))
}

async fn look_files(
    State(state): State<AppState>,
) -> Result<Json<crate::files::FilesReport>, (StatusCode, String)> {
    let session = state
        .store
        .lock()
        .await
        .live
        .clone()
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                "sign in to Proxmox on Pools first".to_string(),
            )
        })?;
    crate::files::inventory(&session)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

async fn look_browse(
    State(state): State<AppState>,
    Json(req): Json<crate::files::BrowseRequest>,
) -> Result<Json<crate::files::BrowseReport>, (StatusCode, String)> {
    let session = state.store.lock().await.live.clone();
    crate::files::browse(session.as_ref(), &req)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

async fn power_opportunities() -> Json<Vec<Opportunity>> {
    let idle = IDLE.load(Ordering::Relaxed);
    let snap = tokio::task::spawn_blocking(collect_system)
        .await
        .unwrap_or_else(|_| SystemSnapshot::empty());
    Json(crate::advisor::opportunities(idle, snap.cpu_pct))
}

fn timing_header(d: Duration) -> [(axum::http::header::HeaderName, HeaderValue); 1] {
    let val = HeaderValue::from_str(&format!("{}ms", d.as_millis().max(1))).unwrap_or(HeaderValue::from_static("1ms"));
    [(axum::http::header::HeaderName::from_static("server-timing"), val)]
}

fn collect_system() -> SystemSnapshot {
    let mut sys = System::new();
    sys.refresh_cpu_all();
    sys.refresh_memory();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let pid = Pid::from_u32(std::process::id());
    let rss = sys.process(pid).map(|p| p.memory()).unwrap_or(0);

    SystemSnapshot {
        hostname: System::host_name().unwrap_or_else(|| "keel".into()),
        os: System::long_os_version().unwrap_or_else(|| std::env::consts::OS.into()),
        cpu_pct: sys.global_cpu_usage(),
        mem_used: sys.used_memory(),
        mem_total: sys.total_memory(),
        daemon_rss: rss,
        cores: sys.cpus().len() as u32,
    }
}
