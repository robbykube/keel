mod advisor;
mod apps;
mod auth;
mod catalog;
mod config;
mod discover;
mod files;
mod folders;
mod github_connect;
mod installer;
mod inventory;
mod issues;
mod model;
mod overlay;
mod remote;
mod services;
mod copies;
mod smb;
mod storage;
mod store;
mod updates;

use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use axum::extract::{Path, Query, State};
use axum::response::Redirect;
use axum::handler::Handler;
use axum::http::{HeaderValue, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use sysinfo::{Networks, Pid, ProcessesToUpdate, System};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tower_http::services::ServeDir;

use crate::inventory::Inventory;
use crate::model::{
    Disk, Guest, Health, InstallerPlan, IssuesReport, NewNote, NewPool, NewShare, Note,
    Opportunity, Pool, Power, RepairRequest, RepairResult, ShareView, SystemSnapshot,
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
    if let Some(command) = std::env::args().nth(1).as_deref() {
        match command {
            "update-apply" => {
                if let Err(error) = updates::apply_from_request().await {
                    eprintln!("keel: {error}");
                    std::process::exit(1);
                }
                return;
            }
            "sign-file" => {
                let Some(path) = std::env::args().nth(2) else {
                    eprintln!("keel: sign-file needs a file");
                    std::process::exit(2);
                };
                if let Err(error) = updates::sign_file(std::path::Path::new(&path)) {
                    eprintln!("keel: {error}");
                    std::process::exit(1);
                }
                return;
            }
            "keygen" => {
                let Some(path) = std::env::args().nth(2) else {
                    eprintln!("keel: keygen needs a path outside the repository");
                    std::process::exit(2);
                };
                if let Err(error) = updates::generate_release_key(std::path::Path::new(&path)) {
                    eprintln!("keel: {error}");
                    std::process::exit(1);
                }
                return;
            }
            _ => {}
        }
    }
    if let Err(error) = crate::config::ensure_supported() {
        eprintln!("keel: {error}");
        std::process::exit(1);
    }

    STARTED.set(Instant::now()).ok();

    let port: u16 = std::env::var("KEEL_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(9478);

    let web = std::env::current_dir().expect("cwd").join("web");

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/updates", get(updates_status))
        .route("/api/updates/progress", get(updates_progress))
        .route(
            "/api/updates/install",
            post(updates_install).layer(axum::middleware::from_fn(auth::require_admin)),
        )
        .route(
            "/api/updates/restore",
            post(updates_restore).layer(axum::middleware::from_fn(auth::require_admin)),
        )
        .route(
            "/api/settings/github-token",
            post(github_token_save).layer(axum::middleware::from_fn(auth::require_admin)),
        )
        .route(
            "/api/settings/github-client-id",
            post(github_client_id_save).layer(axum::middleware::from_fn(auth::require_admin)),
        )
        .route(
            "/api/settings/github-connect",
            get(github_connect_poll)
                .post(github_connect_start)
                .layer(axum::middleware::from_fn(auth::require_admin)),
        )
        .route(
            "/api/settings/github-connect/cancel",
            post(github_connect_cancel).layer(axum::middleware::from_fn(auth::require_admin)),
        )
        .route("/api/settings/github-app", get(github_app_setup))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/session", get(auth::session_state))
        .route("/api/auth/logout", post(auth::sign_out))
        .route(
            "/api/restart",
            post(restart).layer(axum::middleware::from_fn(auth::require_admin)),
        )
        .route(
            "/api/auth/requests",
            get(auth::sign_in_requests).post(auth::create_sign_in),
        )
        .route(
            "/api/auth/requests/{id}",
            get(auth::sign_in_status).post(auth::answer_sign_in),
        )
        .route("/api/system", get(system))
        .route("/api/disks", get(disks))
        .route("/api/storage", get(storage_view))
        .route(
            "/api/storage/prepare",
            post(storage_prepare).layer(axum::middleware::from_fn(auth::require_admin)),
        )
        .route(
            "/api/storage/owner",
            post(storage_owner).layer(axum::middleware::from_fn(auth::require_admin)),
        )
        .route(
            "/api/storage/copy-now",
            post(storage_copy_now).layer(axum::middleware::from_fn(auth::require_admin)),
        )
        .route("/api/installer", get(installer_status))
        .route(
            "/api/installer/progress",
            get(installer_progress).layer(axum::middleware::from_fn(auth::require_admin)),
        )
        .route(
            "/api/installer/plan",
            post(installer_plan).layer(axum::middleware::from_fn(auth::require_admin)),
        )
        .route(
            "/api/pools",
            get(pools).post(pools_create.layer(axum::middleware::from_fn(auth::require_admin))),
        )
        .route(
            "/api/shares",
            get(shares).post(shares_create.layer(axum::middleware::from_fn(auth::require_admin))),
        )
        .route(
            "/api/folders",
            get(folders_home).layer(axum::middleware::from_fn(auth::require_admin)),
        )
        .route(
            "/api/folders/list",
            get(folders_list).layer(axum::middleware::from_fn(auth::require_admin)),
        )
        .route(
            "/api/folders/mkdir",
            post(folders_mkdir).layer(axum::middleware::from_fn(auth::require_admin)),
        )
        .route(
            "/api/folders/copy",
            post(folders_copy).layer(axum::middleware::from_fn(auth::require_admin)),
        )
        .route(
            "/api/folders/move",
            post(folders_move).layer(axum::middleware::from_fn(auth::require_admin)),
        )
        .route("/api/power", get(power))
        .route(
            "/api/power/idle",
            post(power_idle).layer(axum::middleware::from_fn(auth::require_admin)),
        )
        .route(
            "/api/power/active",
            post(power_active).layer(axum::middleware::from_fn(auth::require_admin)),
        )
        .route(
            "/api/power/cancel",
            post(power_active).layer(axum::middleware::from_fn(auth::require_admin)),
        )
        .route("/api/power/opportunities", get(power_opportunities))
        .route(
            "/api/shutdown",
            post(shutdown).layer(axum::middleware::from_fn(auth::require_admin)),
        )
        .route("/api/overlay", get(overlay_status))
        .route("/api/services", get(services_status))
        .route("/api/network/ports", get(network_ports))
        .route(
            "/api/overlay/enroll",
            post(overlay_enroll).layer(axum::middleware::from_fn(auth::require_admin)),
        )
        .route("/api/catalog", get(catalog_list))
        .route("/api/apps", get(apps_list))
        .route(
            "/api/apps/{id}/action",
            post(app_action).layer(axum::middleware::from_fn(auth::require_admin)),
        )
        .route("/api/issues", get(issues_list))
        .route(
            "/api/repair",
            post(repair_run).layer(axum::middleware::from_fn(auth::require_admin)),
        )
        .route(
            "/api/notes",
            get(notes_list)
                .post(notes_create.layer(axum::middleware::from_fn(auth::require_admin))),
        )
        .route(
            "/api/notes/{id}",
            axum::routing::put(notes_update)
                .delete(notes_delete)
                .layer(axum::middleware::from_fn(auth::require_admin)),
        )
        .route(
            "/api/look",
            post(look_around).layer(axum::middleware::from_fn(auth::require_admin)),
        )
        .route(
            "/api/look/login",
            post(look_login).layer(axum::middleware::from_fn(auth::require_admin)),
        )
        .route("/api/look/saved", get(look_saved))
        .route(
            "/api/look/forget",
            post(look_forget).layer(axum::middleware::from_fn(auth::require_admin)),
        )
        .route("/api/look/session", get(look_session))
        .route("/api/look/files", get(look_files))
        .route(
            "/api/look/browse",
            post(look_browse).layer(axum::middleware::from_fn(auth::require_admin)),
        )
        .with_state({
            let mut store = Store::load();
            crate::storage::ensure_shares(&mut store);
            crate::smb::bootstrap(&mut store);
            let state = AppState {
                inventory: Inventory::detect(),
                store: Arc::new(Mutex::new(store)),
            };
            tokio::spawn(watch_disks(state.clone()));
            state
        })
        .fallback_service(ServeDir::new(web))
        // Everything keel serves is local and live. A cached GUI is always
        // wrong here: after an update the appliance's own kiosk would keep
        // drawing the previous version's pages from disk cache. "no-cache"
        // still allows revalidation, so unchanged files come back as 304.
        .layer(axum::middleware::from_fn(no_cache));

    let bind = std::env::var("KEEL_BIND").unwrap_or_else(|_| "127.0.0.1".into());
    let addr: SocketAddr = format!("{bind}:{port}")
        .parse()
        .unwrap_or_else(|_| panic!("invalid KEEL_BIND address: {bind}"));
    if !addr.ip().is_loopback() && !crate::auth::token_configured() {
        panic!("KEEL_ADMIN_TOKEN is required when KEEL_BIND is not loopback");
    }
    let listener = TcpListener::bind(addr).await.expect("bind");
    eprintln!("keel listening on http://{addr}");
    if crate::auth::loopback_trusted() {
        eprintln!("keel trusts loopback requests without a token (local console)");
    }
    // Connect info is what lets `auth::require_admin` tell a local kiosk request
    // from a LAN one. Without it every request looks remote and the console
    // kiosk is locked out of its own appliance.
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("serve");
}

async fn no_cache(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache"),
    );
    response
}

async fn health() -> Json<Health> {
    Json(Health {
        ok: true,
        name: "keel",
        version: env!("CARGO_PKG_VERSION"),
        uptime_ms: STARTED.get().map(|t| t.elapsed().as_millis()).unwrap_or(0),
        inventory: Inventory::detect().label(),
        can_install: crate::installer::available(),
    })
}

async fn system() -> (
    [(axum::http::header::HeaderName, HeaderValue); 1],
    Json<SystemSnapshot>,
) {
    let t0 = Instant::now();
    let snap = tokio::task::spawn_blocking(collect_system)
        .await
        .unwrap_or_else(|_| SystemSnapshot::empty());
    let ms = t0.elapsed();
    (timing_header(ms), Json(snap))
}

async fn disks(
    State(state): State<AppState>,
) -> (
    [(axum::http::header::HeaderName, HeaderValue); 1],
    Json<Vec<Disk>>,
) {
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

async fn shares(State(state): State<AppState>) -> Json<Vec<ShareView>> {
    let shares = state.store.lock().await.shares.clone();
    Json(
        tokio::task::spawn_blocking({
            let shares = shares.clone();
            move || crate::smb::decorate(&shares)
        })
        .await
        .unwrap_or_else(|_| crate::smb::decorate(&shares)),
    )
}

async fn shares_create(
    State(state): State<AppState>,
    Json(req): Json<NewShare>,
) -> Result<Json<ShareView>, (StatusCode, String)> {
    let mut store = state.store.lock().await;
    let share = store
        .add_share(req)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let shares = store.shares.clone();
    drop(store);
    let views = tokio::task::spawn_blocking({
        let shares = shares.clone();
        move || {
            crate::smb::apply(&shares);
            crate::smb::decorate(&shares)
        }
    })
    .await
    .unwrap_or_else(|_| crate::smb::decorate(&shares));
    let view = views
        .into_iter()
        .find(|item| item.name == share.name)
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "share was saved but not listed".into(),
            )
        })?;
    Ok(Json(view))
}

async fn folders_home(
    State(state): State<AppState>,
) -> Json<crate::folders::FolderHome> {
    let shares = state.store.lock().await.shares.clone();
    Json(
        tokio::task::spawn_blocking(move || crate::folders::home(&shares))
            .await
            .expect("folder list"),
    )
}

async fn folders_list(
    State(state): State<AppState>,
    Query(query): Query<crate::folders::ListQuery>,
) -> Result<Json<crate::folders::Listing>, (StatusCode, Json<serde_json::Value>)> {
    let shares = state.store.lock().await.shares.clone();
    let result = tokio::task::spawn_blocking(move || {
        crate::folders::list(&shares, &query.share, &query.path)
    })
    .await
    .unwrap_or_else(|_| Err("Keel could not open that folder. Reload the page.".into()));
    match result {
        Ok(listing) => Ok(Json(listing)),
        Err(message) => Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "ok": false, "message": message })),
        )),
    }
}

async fn folders_mkdir(
    State(state): State<AppState>,
    Json(req): Json<crate::folders::MakeRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let shares = state.store.lock().await.shares.clone();
    let name = req.name.clone();
    let result = tokio::task::spawn_blocking(move || {
        let path = crate::folders::make_folder(&shares, &req.share, &req.path, &req.name)?;
        crate::storage::give_to_owner(&path);
        Ok(format!("{name} is ready."))
    })
    .await
    .unwrap_or_else(|_| Err("Keel could not make that folder. Try again.".into()));
    storage_reply(result)
}

async fn folders_copy(
    State(state): State<AppState>,
    Json(req): Json<crate::folders::TransferRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    folder_transfer(state, req, false).await
}

async fn folders_move(
    State(state): State<AppState>,
    Json(req): Json<crate::folders::TransferRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    folder_transfer(state, req, true).await
}

async fn folder_transfer(
    state: AppState,
    req: crate::folders::TransferRequest,
    remove_source: bool,
) -> (StatusCode, Json<serde_json::Value>) {
    let shares = state.store.lock().await.shares.clone();
    let verb = if remove_source { "Moved." } else { "Copied." };
    let result = tokio::task::spawn_blocking(move || {
        let path = if remove_source {
            crate::folders::move_item(
                &shares,
                &req.from_share,
                &req.from_path,
                &req.to_share,
                &req.to_path,
            )
        } else {
            crate::folders::copy_item(
                &shares,
                &req.from_share,
                &req.from_path,
                &req.to_share,
                &req.to_path,
            )
        }?;
        crate::storage::give_to_owner(&path);
        Ok(verb.to_string())
    })
    .await
    .unwrap_or_else(|_| Err("Keel could not finish that. Try again.".into()));
    storage_reply(result)
}

fn power_state() -> Power {
    let idle = IDLE.load(Ordering::Relaxed);
    Power {
        policy: "overlay-floor",
        mode: if idle { "idle" } else { "active" },
        host_sleep_allowed: false,
        services_stopped: idle,
        overlay_required: true,
        why: "Idle keeps the host on and the selected overlay running so you can open files and the GUI when you are not at home. The rest of the catalog is installed, not running — like an OS with apps closed. keel stays up as the small control plane. Host S3 is off: that would kill the overlay and the file path.",
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

fn power_command(verb: &str, done: &str, word: &str) -> (StatusCode, Json<serde_json::Value>) {
    if !cfg!(target_os = "linux") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "ok": false, "message": format!("{word} is available on the Keel Linux appliance, not this development host.") })),
        );
    }
    match std::process::Command::new("systemctl").arg(verb).spawn() {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({ "ok": true, "message": done }))),
        Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "ok": false, "message": format!("Could not start {}: {error}", word.to_lowercase()) }))),
    }
}

async fn shutdown() -> (StatusCode, Json<serde_json::Value>) {
    power_command("poweroff", "Keel is shutting down.", "Shutdown")
}

async fn restart() -> (StatusCode, Json<serde_json::Value>) {
    power_command("reboot", "Keel is restarting. This page comes back in about a minute.", "Restart")
}

/// Once a minute: offer or hide folders when a disk comes or goes, and start
/// the daily copy when it is due.
async fn watch_disks(state: AppState) {
    let mut seen: Vec<bool> = Vec::new();
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
        let now: Vec<bool> = crate::storage::ROLES
            .iter()
            .map(|role| crate::storage::mounted(role.id))
            .collect();
        if now != seen {
            seen = now;
            let shares = state.store.lock().await.shares.clone();
            let _ = tokio::task::spawn_blocking(move || crate::smb::apply(&shares)).await;
        }
        if crate::copies::due() {
            let _ = tokio::task::spawn_blocking(crate::copies::run_now).await;
        }
    }
}

async fn storage_view(State(state): State<AppState>) -> Json<crate::storage::StorageView> {
    let disks = state.inventory.disks().await;
    Json(
        tokio::task::spawn_blocking(move || crate::storage::view(disks))
            .await
            .expect("storage view"),
    )
}

fn storage_reply(result: Result<String, String>) -> (StatusCode, Json<serde_json::Value>) {
    match result {
        Ok(message) => (StatusCode::OK, Json(serde_json::json!({ "ok": true, "message": message }))),
        Err(message) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "ok": false, "message": message })),
        ),
    }
}

async fn storage_prepare(
    State(state): State<AppState>,
    Json(req): Json<crate::storage::PrepareRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let disks = state.inventory.disks().await;
    let result = tokio::task::spawn_blocking(move || crate::storage::prepare(&req, &disks))
        .await
        .unwrap_or_else(|_| Err("The disk job stopped unexpectedly.".into()));
    if result.is_ok() {
        let mut store = state.store.lock().await;
        crate::storage::ensure_shares(&mut store);
        let shares = store.shares.clone();
        drop(store);
        let _ = tokio::task::spawn_blocking(move || crate::smb::apply(&shares)).await;
    }
    storage_reply(result)
}

async fn storage_owner(
    State(state): State<AppState>,
    Json(req): Json<crate::storage::OwnerRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let shares = state.store.lock().await.shares.clone();
    let result = tokio::task::spawn_blocking(move || {
        let dirs: Vec<String> = shares
            .iter()
            .filter(|share| !share.read_only && crate::smb::available(share))
            .map(|share| share.path.clone())
            .filter(|path| path.starts_with('/'))
            .collect();
        let result = crate::storage::set_owner(&req, &dirs);
        if result.is_ok() {
            crate::smb::apply(&shares);
        }
        result
    })
    .await
    .unwrap_or_else(|_| Err("Setting the name stopped unexpectedly.".into()));
    storage_reply(result)
}

async fn storage_copy_now() -> (StatusCode, Json<serde_json::Value>) {
    if !crate::copies::can_run() {
        return storage_reply(Err(
            "Copy needs the Files disk and at least one copy disk connected.".into(),
        ));
    }
    if crate::copies::running() {
        return storage_reply(Err("A copy is already running.".into()));
    }
    tokio::task::spawn_blocking(crate::copies::run_now);
    storage_reply(Ok("Copy started. This page shows when it is done.".into()))
}

async fn catalog_list() -> Json<Vec<Guest>> {
    let idle = IDLE.load(Ordering::Relaxed);
    Json(crate::catalog::guests(idle))
}

async fn issues_list(State(state): State<AppState>) -> Json<IssuesReport> {
    let idle = IDLE.load(Ordering::Relaxed);
    let store = state.store.lock().await;
    let mut report = crate::issues::report(
        idle,
        state.inventory,
        store.pools.len() + crate::storage::prepared_roles().len(),
        store.shares.len(),
    );
    let extra = crate::storage::issues();
    report.red |= extra.iter().any(|issue| issue.severity == "red");
    report.items.splice(0..0, extra);
    Json(report)
}

async fn repair_run(Json(req): Json<RepairRequest>) -> Json<RepairResult> {
    if req.id.starts_with("restart-") {
        return Json(
            tokio::task::spawn_blocking(move || crate::services::restart(&req.id))
                .await
                .unwrap_or_else(|_| RepairResult {
                    ok: false,
                    id: "restart".into(),
                    message: "Could not start the service restart.".into(),
                }),
        );
    }
    Json(crate::issues::apply(&req.id))
}

async fn installer_status(State(state): State<AppState>) -> Json<crate::model::InstallerStatus> {
    Json(crate::installer::status(state.inventory.disks().await))
}

async fn updates_status() -> Json<crate::updates::UpdateStatus> {
    Json(crate::updates::check().await)
}

async fn updates_progress() -> Json<crate::updates::UpdateProgress> {
    Json(crate::updates::progress())
}

async fn updates_install(
    Json(request): Json<crate::updates::UpdateRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    update_reply(crate::updates::queue_apply(&request.confirmation).await)
}

async fn updates_restore(
    Json(request): Json<crate::updates::UpdateRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    update_reply(crate::updates::queue_restore(&request.confirmation).await)
}

async fn github_token_save(
    Json(request): Json<crate::updates::GithubTokenRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    update_reply(crate::updates::save_github_token(&request.token))
}

async fn github_client_id_save(
    Json(request): Json<crate::updates::GithubClientIdRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    update_reply(crate::updates::save_github_client_id(&request.client_id))
}

async fn github_connect_start() -> Result<Json<crate::github_connect::GithubConnectStatus>, (StatusCode, Json<serde_json::Value>)>
{
    crate::github_connect::start()
        .await
        .map(Json)
        .map_err(|message| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "ok": false, "message": message })),
            )
        })
}

async fn github_connect_poll() -> Result<Json<crate::github_connect::GithubConnectStatus>, (StatusCode, Json<serde_json::Value>)>
{
    crate::github_connect::poll()
        .await
        .map(Json)
        .map_err(|message| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "ok": false, "message": message })),
            )
        })
}

async fn github_connect_cancel() -> Json<crate::github_connect::GithubConnectStatus> {
    Json(crate::github_connect::cancel())
}

#[derive(Deserialize)]
struct GithubAppQuery {
    code: Option<String>,
}

async fn github_app_setup(Query(query): Query<GithubAppQuery>) -> Redirect {
    let Some(code) = query.code.filter(|value| !value.trim().is_empty()) else {
        return Redirect::to("/settings.html?github=missing");
    };
    match crate::github_connect::finish_app(&code).await {
        Ok(_) => Redirect::to("/settings.html?github=ready"),
        Err(_) => Redirect::to("/settings.html?github=failed"),
    }
}

fn update_reply(result: Result<String, String>) -> (StatusCode, Json<serde_json::Value>) {
    match result {
        Ok(message) => (
            StatusCode::OK,
            Json(serde_json::json!({ "ok": true, "message": message })),
        ),
        Err(message) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "ok": false, "message": message })),
        ),
    }
}

async fn installer_progress() -> Json<crate::installer::InstallProgress> {
    Json(crate::installer::progress())
}

async fn installer_plan(
    State(state): State<AppState>,
    Json(plan): Json<InstallerPlan>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let disks = state.inventory.disks().await;
    crate::installer::start(&plan, &disks)
        .map(|message| {
            (
                StatusCode::ACCEPTED,
                Json(serde_json::json!({ "ok": true, "message": message })),
            )
        })
        .map_err(|message| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "ok": false, "message": message })),
            )
        })
}

async fn apps_list() -> Json<Vec<crate::apps::AppManifest>> {
    let idle = IDLE.load(Ordering::Relaxed);
    Json(crate::apps::manifests(idle))
}

async fn app_action(
    Path(id): Path<String>,
    Json(req): Json<crate::apps::ActionRequest>,
) -> Json<RepairResult> {
    let fallback_id = id.clone();
    Json(
        tokio::task::spawn_blocking(move || crate::apps::action(&id, &req.action))
            .await
            .unwrap_or_else(|_| RepairResult {
                ok: false,
                id: fallback_id,
                message: "Could not start the app action.".into(),
            }),
    )
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
    let session = state.store.lock().await.live.clone().ok_or_else(|| {
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

async fn overlay_status() -> Json<crate::overlay::Status> {
    Json(
        tokio::task::spawn_blocking(crate::overlay::status)
            .await
            .unwrap_or_else(|_| crate::overlay::Status {
                provider: crate::overlay::Provider::Unknown,
                state: crate::overlay::State::Unavailable,
                control_url: None,
                overlay_ip: None,
                dns_name: None,
                why: "Could not collect overlay status.".into(),
            }),
    )
}

async fn services_status() -> Json<Vec<crate::services::ServiceStatus>> {
    Json(
        tokio::task::spawn_blocking(crate::services::core_status)
            .await
            .unwrap_or_default(),
    )
}

async fn network_ports() -> Json<Vec<crate::services::PortStatus>> {
    Json(
        tokio::task::spawn_blocking(crate::services::port_status)
            .await
            .unwrap_or_default(),
    )
}

async fn overlay_enroll(
    Json(req): Json<crate::overlay::EnrollRequest>,
) -> Json<crate::overlay::EnrollResult> {
    Json(
        tokio::task::spawn_blocking(|| crate::overlay::enroll(req))
            .await
            .unwrap_or_else(|_| crate::overlay::EnrollResult {
                ok: false,
                provider: crate::overlay::Provider::Unknown,
                message: "Could not start overlay enrollment.".into(),
            }),
    )
}

fn timing_header(d: Duration) -> [(axum::http::header::HeaderName, HeaderValue); 1] {
    let val = HeaderValue::from_str(&format!("{}ms", d.as_millis().max(1)))
        .unwrap_or(HeaderValue::from_static("1ms"));
    [(
        axum::http::header::HeaderName::from_static("server-timing"),
        val,
    )]
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
        platform: std::env::consts::OS,
        addresses: local_addresses(),
        cpu_pct: sys.global_cpu_usage(),
        mem_used: sys.used_memory(),
        mem_total: sys.total_memory(),
        daemon_rss: rss,
        cores: sys.cpus().len() as u32,
    }
}

/// The addresses another computer on the house network can use to reach this
/// machine. Link-local and loopback are dropped: they are noise to the owner,
/// who is being told "type this into your browser".
fn local_addresses() -> Vec<String> {
    let networks = Networks::new_with_refreshed_list();
    let mut found: Vec<String> = networks
        .iter()
        .flat_map(|(_, data)| data.ip_networks())
        .filter_map(|net| match net.addr {
            IpAddr::V4(v4) if !v4.is_loopback() && !v4.is_link_local() && !v4.is_unspecified() => {
                Some(v4.to_string())
            }
            _ => None,
        })
        .collect();
    found.sort();
    found.dedup();
    found
}
