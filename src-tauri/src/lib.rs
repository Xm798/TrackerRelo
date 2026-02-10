mod commands;
mod config;
mod downloader;

use tauri::{LogicalUnit, Manager, PixelUnit, WindowSizeConstraints};
use tauri_plugin_log::{Target, TargetKind, RotationStrategy, TimezoneStrategy};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir { file_name: None }),
                    Target::new(TargetKind::Webview),
                ])
                .rotation_strategy(RotationStrategy::KeepSome(3))
                .max_file_size(5_000_000)
                .timezone_strategy(TimezoneStrategy::UseLocal)
                .level(log::LevelFilter::Debug)
                .level_for("reqwest", log::LevelFilter::Warn)
                .level_for("tao", log::LevelFilter::Warn)
                .level_for("wry", log::LevelFilter::Warn)
                .build(),
        )
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            log::info!("TrackerRelo starting");

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
