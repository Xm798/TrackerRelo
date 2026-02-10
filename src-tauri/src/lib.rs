mod commands;
mod config;
mod downloader;

use tauri::{LogicalUnit, Manager, PixelUnit, WindowSizeConstraints};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            for (_, window) in app.webview_windows() {
                let inner_size = window.inner_size()?;
                let scale_factor = window.scale_factor()?;
                let logical_size = inner_size.to_logical::<f64>(scale_factor);
                let locked_width = logical_size.width;

                let constraints = WindowSizeConstraints {
                    min_width: Some(PixelUnit::from(LogicalUnit::new(locked_width))),
                    max_width: Some(PixelUnit::from(LogicalUnit::new(locked_width))),
                    ..Default::default()
                };

                window.set_size_constraints(constraints)?;
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::test_connection,
            commands::scan_torrents,
            commands::execute_replace,
            commands::list_trackers,
            commands::load_config,
            commands::save_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
