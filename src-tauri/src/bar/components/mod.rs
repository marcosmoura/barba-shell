use tauri::WebviewWindow;

pub mod apps;
pub mod battery;
pub mod cpu;
pub mod hyprspace;
pub mod media;

pub fn init(window: &WebviewWindow) { media::init(window); }
