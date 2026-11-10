mod osinfo;
mod selftest;
mod updater;
#[cfg(windows)]
mod win7_shims;

use serde::Serialize;
use tauri::Manager;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    app_version: String,
    webview_version: String,
    os: String,
    self_test: bool,
}

#[tauri::command]
fn app_info(app: tauri::AppHandle) -> AppInfo {
    AppInfo {
        app_version: app.package_info().version.to_string(),
        webview_version: tauri::webview_version().unwrap_or_else(|e| format!("erro: {e}")),
        os: osinfo::describe(),
        self_test: app.try_state::<selftest::SelfTest>().is_some(),
    }
}

pub fn run() {
    let self_test = selftest::from_args();

    let mut builder = tauri::Builder::default();
    if self_test.is_none() {
        // Garante uma só instância, senão o instalador do update encontra o exe em uso.
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }));
    }

    builder
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            app_info,
            updater::check_for_updates,
            selftest::selftest_report
        ])
        .setup(move |app| {
            match self_test {
                Some(st) => selftest::start(app.handle(), st),
                None => updater::spawn_background_checks(app.handle().clone()),
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o aplicativo");
}
