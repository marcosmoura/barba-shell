use tauri::Wry;
use tauri::plugin::{Builder, PluginApi};

mod bar;
mod cli;
mod config;
mod constants;
mod hotkey;
mod launch;
mod utils;
mod wallpaper;

/// Runs the Tauri application.
///
/// # Panics
pub fn run() {
    // Initialize the configuration system early
    config::init();

    // Initialize wallpaper manager early so CLI commands can use it
    wallpaper::init();

    let (is_cli_mode, cli_exit_code) = launch::get_launch_mode();

    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .manage(bar::components::keepawake::KeepAwakeController::default())
        .plugin(tauri_plugin_cli::init())
        .plugin({
            Builder::new("helper")
                .setup(|app, _api: PluginApi<Wry, ()>| {
                    cli::handle_cli_invocation(app, &std::env::args().collect::<Vec<String>>());

                    Ok(())
                })
                .build()
        })
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            cli::handle_cli_invocation(app, &args);
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(hotkey::create_hotkey_plugin())
        .invoke_handler(tauri::generate_handler![
            bar::components::apps::open_app,
            bar::components::battery::get_battery_info,
            bar::components::cpu::get_cpu_info,
            bar::components::hyprspace::get_hyprspace_current_workspace_windows,
            bar::components::hyprspace::get_hyprspace_focused_window,
            bar::components::hyprspace::get_hyprspace_focused_workspace,
            bar::components::hyprspace::get_hyprspace_workspaces,
            bar::components::hyprspace::go_to_hyprspace_workspace,
            bar::components::keepawake::is_system_awake,
            bar::components::keepawake::toggle_system_awake,
            bar::components::media::get_current_media_info,
        ])
        .setup(move |app| {
            if is_cli_mode {
                launch::quit_app_with_code(app.handle(), cli_exit_code);

                return Ok(());
            }

            // Start watching the config file for changes
            config::watch_config_file(app.handle().clone());

            // Initialize Bar components
            bar::init(app);

            // Start wallpaper manager (set initial wallpaper and start timer)
            wallpaper::start();

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
