//! Verificação e instalação automática de updates (GitHub Releases).

use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
/// Dá tempo à janela de carregar antes da primeira verificação.
const STARTUP_DELAY: Duration = Duration::from_secs(5);

static RUNNING: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum Status {
    Checking,
    UpToDate,
    Available { version: String },
    Downloading { downloaded: u64, total: Option<u64> },
    Installing,
    Error { message: String },
}

fn emit(app: &AppHandle, status: Status) {
    let _ = app.emit("update://status", status);
}

/// Raízes TLS embutidas (Mozilla). O Win7 tem store de certificados
/// desatualizado e o verificador da plataforma falharia em hosts modernos.
fn bundled_roots() -> Vec<reqwest::Certificate> {
    webpki_root_certs::TLS_SERVER_ROOT_CERTS
        .iter()
        .filter_map(|der| reqwest::Certificate::from_der(der).ok())
        .collect()
}

pub fn spawn_background_checks(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio_sleep(STARTUP_DELAY).await;
        loop {
            run_once(&app).await;
            tokio_sleep(CHECK_INTERVAL).await;
        }
    });
}

#[tauri::command]
pub async fn check_for_updates(app: AppHandle) {
    run_once(&app).await;
}

async fn run_once(app: &AppHandle) {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Err(e) = check_and_install(app).await {
        log::error!("update falhou: {e}");
        emit(app, Status::Error { message: e.to_string() });
    }
    RUNNING.store(false, Ordering::SeqCst);
}

async fn check_and_install(app: &AppHandle) -> tauri_plugin_updater::Result<()> {
    emit(app, Status::Checking);

    let roots = bundled_roots();
    let updater = app
        .updater_builder()
        .timeout(Duration::from_secs(120))
        .configure_client(move |c| c.tls_certs_only(roots.clone()))
        .build()?;

    let Some(update) = updater.check().await? else {
        emit(app, Status::UpToDate);
        return Ok(());
    };

    emit(app, Status::Available { version: update.version.clone() });

    let mut downloaded = 0u64;
    let progress_app = app.clone();
    let done_app = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                emit(&progress_app, Status::Downloading { downloaded, total });
            },
            move || emit(&done_app, Status::Installing),
        )
        .await?;

    // No Windows o instalador NSIS já encerra o app; em outras plataformas reinicia.
    app.restart();
}

async fn tokio_sleep(d: Duration) {
    // Evita depender de tokio diretamente: dorme numa thread bloqueante do runtime.
    let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(d)).await;
}
