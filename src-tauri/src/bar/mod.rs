mod constants;
mod window;

use std::os::raw::c_void;
use std::sync::mpsc::{Sender, channel};

use core_graphics::display::CGDisplayRegisterReconfigurationCallback;
use tauri::{App, Manager};

use crate::utils::thread::spawn_named_thread;

pub fn init(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle = app.app_handle().clone();
    let webview_window = app_handle.get_webview_window("bar").unwrap();

    window::set_window_position(&webview_window)?;
    window::set_window_sticky(&webview_window);
    window::set_window_below_menu(&webview_window);

    init_screen_watcher(&webview_window);

    // Make the app not appear in the dock
    let _ = app_handle.set_activation_policy(tauri::ActivationPolicy::Prohibited);
    let _ = webview_window.show();

    // Open devtools if in dev mode
    #[cfg(debug_assertions)]
    {
        webview_window.open_devtools();
    }

    Ok(())
}

pub fn init_screen_watcher(webview_window: &tauri::WebviewWindow) {
    let window = webview_window.clone();

    spawn_named_thread("screen-watcher", move || {
        let (tx, rx) = channel();

        // Register display reconfiguration callback
        unsafe {
            extern "C" fn display_reconfiguration_callback(
                _display: u32,
                _flags: u32,
                user_info: *const c_void,
            ) {
                if !user_info.is_null() {
                    let tx = unsafe { &*user_info.cast::<Sender<()>>() };
                    let _ = tx.send(());
                }
            }

            let tx_ptr: *const Sender<()> = Box::into_raw(Box::new(tx));

            CGDisplayRegisterReconfigurationCallback(
                display_reconfiguration_callback,
                tx_ptr.cast::<c_void>(),
            );
        }

        while rx.recv().is_ok() {
            if let Err(err) = window::set_window_position(&window) {
                eprintln!("Failed to reposition window after screen change: {err}");
            }
        }
    });
}
