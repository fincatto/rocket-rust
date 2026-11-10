//! `rocket-rust.exe --self-test <arquivo.json>`: abre a janela, espera o
//! frontend provar que executou JS e grava um relatório. Usado no E2E da VM Win7.

use std::{path::PathBuf, time::Duration};

use serde::Serialize;
use tauri::{AppHandle, Manager};

/// Se o frontend não reportar nesse tempo, o teste falha.
const TIMEOUT: Duration = Duration::from_secs(60);

pub struct SelfTest {
    pub output: PathBuf,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    ok: bool,
    app_version: String,
    webview_version: Option<String>,
    os: String,
    user_agent: Option<String>,
    error: Option<String>,
}

pub fn from_args() -> Option<SelfTest> {
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--self-test" {
            let output = args
                .next()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("selftest.json"));
            return Some(SelfTest { output });
        }
    }
    None
}

pub fn start(app: &AppHandle, st: SelfTest) {
    app.manage(st);
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(TIMEOUT);
        finish(&app, None, Some("timeout: o frontend não respondeu".into()));
    });
}

#[tauri::command]
pub fn selftest_report(app: AppHandle, user_agent: String) {
    finish(&app, Some(user_agent), None);
}

fn finish(app: &AppHandle, user_agent: Option<String>, error: Option<String>) {
    let Some(st) = app.try_state::<SelfTest>() else {
        return;
    };
    let report = Report {
        ok: error.is_none(),
        app_version: app.package_info().version.to_string(),
        webview_version: tauri::webview_version().ok(),
        os: crate::osinfo::describe(),
        user_agent,
        error,
    };
    let code = if report.ok { 0 } else { 1 };
    let json = serde_json::to_vec_pretty(&report).expect("relatório serializável");
    if let Err(e) = std::fs::write(&st.output, json) {
        eprintln!("não foi possível gravar {}: {e}", st.output.display());
        app.exit(2);
        return;
    }
    app.exit(code);
}
