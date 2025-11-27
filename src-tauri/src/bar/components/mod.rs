use tauri::WebviewWindow;

pub mod hyprspace;
pub mod media;

pub fn init(window: &WebviewWindow) { media::init(window); }
