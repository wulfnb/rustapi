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
mod ui;

#[cfg(test)]
#[path = "../tests/support/egui.rs"]
mod test_support;

use rustapi::{http, models, storage};

use app::RustApiApp;

fn main() -> eframe::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match rustapi::cli::dispatch(&args) {
        Ok(Some(code)) => std::process::exit(code),
        Ok(None) => {}
        Err(e) => {
            eprintln!("{e:#}");
            std::process::exit(2);
        }
    }
    let smoke_test = args.first().is_some_and(|arg| arg == "--smoke-test");
    let icon =
        eframe::icon_data::from_png_bytes(include_bytes!("../assets/icons/rustapi-icon-256.png"))
            .ok();
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1400.0, 900.0])
        .with_min_inner_size([800.0, 600.0])
        .with_title("RustAPI - API Testing Client");
    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }
    // Configure native options
    let native_options = eframe::NativeOptions {
        #[cfg(target_os = "windows")]
        renderer: eframe::Renderer::Wgpu,
        viewport,
        ..Default::default()
    };

    // Run the application
    eframe::run_native(
        "RustAPI",
        native_options,
        Box::new(move |cc| {
            let mut inner = RustApiApp::new(cc);
            if smoke_test {
                inner
                    .tab_manager
                    .open_tab(models::ApiRequest::new("Startup smoke test"), None);
            }
            Ok(Box::new(DesktopApp {
                inner,
                smoke_frames: smoke_test.then_some(0),
            }))
        }),
    )?;
    if smoke_test {
        println!("RustAPI desktop smoke test passed");
    }
    Ok(())
}

struct DesktopApp {
    inner: RustApiApp,
    smoke_frames: Option<u8>,
}

impl eframe::App for DesktopApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.inner.update(ctx, frame);
        if let Some(frames) = &mut self.smoke_frames {
            *frames += 1;
            if *frames >= 3 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ctx.request_repaint();
        }
    }

    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        self.inner.on_exit(gl);
    }
}
