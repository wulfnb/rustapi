//! RustAPI - A fast, file-based API testing client inspired by Bruno
//!
//! Features:
//! - Collections stored as JSON files (Git-friendly)
//! - Environment variables with {{variable}} syntax
//! - Support for GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS
//! - Headers, query params, body (JSON, text, form data, URL encoded)
//! - Authentication (Bearer Token, Basic Auth, API Key)
//! - Code generation (cURL, Python, JavaScript, Rust)
//! - Request history
//! - Multiple tabs support
//! - Dark theme with modern UI

mod app;
mod http;
mod models;
mod storage;
mod ui;
mod utils;

use app::RustApiApp;

fn main() -> eframe::Result<()> {
    // Configure native options
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("RustAPI - API Testing Client"),
        ..Default::default()
    };

    // Run the application
    eframe::run_native(
        "RustAPI",
        native_options,
        Box::new(|cc| Ok(Box::new(RustApiApp::new(cc)))),
    )
}
