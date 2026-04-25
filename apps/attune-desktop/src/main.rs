#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod embedded_server;
mod tray;

use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("info".parse().unwrap()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // 重复双击：激活已有主窗口（unminimize + show + focus），第二个进程立即退出
            tracing::info!("single-instance: another launch detected, focusing existing window");
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            // 1. spawn 内嵌 axum
            let _server_handle = embedded_server::spawn_server();

            // 2. 异步等服务就绪后开主窗口
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                match embedded_server::wait_for_ready().await {
                    Ok(()) => {
                        let url = embedded_server::server_url();
                        tracing::info!("opening main window pointing to {}", url);
                        if let Err(e) = WebviewWindowBuilder::new(
                            &app_handle,
                            "main",
                            WebviewUrl::External(url.parse().unwrap()),
                        )
                        .title("Attune")
                        .inner_size(1280.0, 800.0)
                        .min_inner_size(800.0, 600.0)
                        .build()
                        {
                            tracing::error!("failed to build main window: {e}");
                        }

                        // 主窗口事件处理：
                        //   1. 关闭按钮 = 隐藏到托盘，不退出进程
                        //   2. OS 级文件拖拽 → emit 'attune-file-drop' 给前端
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let win_clone = window.clone();
                            let app_for_drop = app_handle.clone();
                            window.on_window_event(move |event| match event {
                                tauri::WindowEvent::CloseRequested { api, .. } => {
                                    api.prevent_close();
                                    let _ = win_clone.hide();
                                }
                                tauri::WindowEvent::DragDrop(
                                    tauri::DragDropEvent::Drop { paths, .. },
                                ) => {
                                    let payload: Vec<String> = paths
                                        .iter()
                                        .map(|p| p.to_string_lossy().into_owned())
                                        .collect();
                                    if let Err(e) =
                                        app_for_drop.emit("attune-file-drop", &payload)
                                    {
                                        tracing::warn!(
                                            "failed to emit attune-file-drop: {e}"
                                        );
                                    }
                                }
                                _ => {}
                            });
                        }

                        // 系统托盘
                        if let Err(e) = crate::tray::build(&app_handle) {
                            tracing::error!("failed to build system tray: {e}");
                        }
                    }
                    Err(e) => {
                        tracing::error!("embedded server failed to start: {e}");
                        std::process::exit(1);
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running attune-desktop");
}
