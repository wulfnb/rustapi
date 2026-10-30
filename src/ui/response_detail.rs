use crate::models::ApiResponse;
use anyhow::{ensure, Result};
use sha2::{Digest, Sha256};

pub fn decode_response_image(bytes: &[u8]) -> Result<egui::ColorImage> {
    ensure!(
        bytes.len() <= 8 * 1024 * 1024,
        "Image preview input exceeds 8 MiB"
    );
    let reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
    ensure!(
        matches!(
            reader.format(),
            Some(image::ImageFormat::Png | image::ImageFormat::Jpeg)
        ),
        "Preview supports PNG and JPEG images"
    );
    let (width, height) = reader.into_dimensions()?;
    ensure!(
        width <= 4096 && height <= 4096 && (width as u64) * (height as u64) <= 4 * 1024 * 1024,
        "Image dimensions exceed preview limits"
    );
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(32 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode()?.into_rgba8();
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [width as usize, height as usize],
        image.as_raw(),
    ))
}
pub fn response_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take(4096)
        .collect::<Vec<_>>()
        .chunks(16)
        .enumerate()
        .map(|(index, chunk)| {
            format!(
                "{:08x}  {}",
                index * 16,
                chunk
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn tree(ui: &mut egui::Ui, key: &str, value: &serde_json::Value, depth: usize, budget: &mut usize) {
    if *budget == 0 || depth > 32 {
        return;
    }
    *budget -= 1;
    match value {
        serde_json::Value::Object(map) => {
            egui::CollapsingHeader::new(format!("{key} {{ {} fields }}", map.len()))
                .id_salt(key)
                .show(ui, |ui| {
                    for (key, value) in map {
                        tree(ui, key, value, depth + 1, budget);
                        if *budget == 0 {
                            break;
                        }
                    }
                });
        }
        serde_json::Value::Array(values) => {
            egui::CollapsingHeader::new(format!("{key} [ {} items ]", values.len()))
                .id_salt(key)
                .show(ui, |ui| {
                    for (index, value) in values.iter().enumerate() {
                        tree(ui, &index.to_string(), value, depth + 1, budget);
                        if *budget == 0 {
                            break;
                        }
                    }
                });
        }
        _ => {
            let text = value.to_string();
            ui.monospace(format!(
                "{key}: {}",
                rustapi::utils::truncate_str(&text, 512)
            ));
        }
    }
}
pub fn render_response_tree(ui: &mut egui::Ui, response: &ApiResponse) {
    let id = ui.id().with("json_pointer");
    let mut pointer = ui
        .data(|data| data.get_temp::<String>(id))
        .unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label("JSON Pointer filter");
        ui.text_edit_singleline(&mut pointer);
    });
    match serde_json::from_str::<serde_json::Value>(&response.body) {
        Ok(value) => {
            if let Some(selected) = value.pointer(&pointer) {
                let mut budget = 1000;
                tree(ui, "$", selected, 0, &mut budget);
                if budget == 0 {
                    ui.label("Tree limited to 1000 visible nodes. Narrow the JSON Pointer filter.");
                }
            } else {
                ui.label("No value at this JSON Pointer.");
            }
        }
        Err(error) => {
            ui.label(format!("Preview is not complete valid JSON: {error}"));
        }
    }
    ui.data_mut(|data| data.insert_temp(id, pointer));
}
#[derive(Clone)]
struct ImagePreview {
    digest: Vec<u8>,
    result: Result<egui::TextureHandle, String>,
}
pub fn render_response_preview(ui: &mut egui::Ui, response: &ApiResponse) {
    let mime = response
        .headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("content-type"))
        .map(|(_, value)| value.as_str())
        .unwrap_or("");
    if mime.starts_with("image/") {
        let bytes = if response.raw_body.is_empty() {
            response.body.as_bytes()
        } else {
            &response.raw_body
        };
        if bytes.len() > 8 * 1024 * 1024 {
            ui.label("Image preview input exceeds 8 MiB. Download the full response.");
            return;
        }
        let id = ui.id().with("image_preview");
        let digest = Sha256::digest(bytes).to_vec();
        let mut cached = ui.data(|data| data.get_temp::<ImagePreview>(id));
        if cached.as_ref().is_none_or(|cache| cache.digest != digest) {
            cached = Some(ImagePreview {
                digest,
                result: decode_response_image(bytes)
                    .map(|image| {
                        ui.ctx().load_texture(
                            "response-preview",
                            image,
                            egui::TextureOptions::LINEAR,
                        )
                    })
                    .map_err(|error| error.to_string()),
            });
        }
        if let Some(cache) = &cached {
            match &cache.result {
                Ok(texture) => {
                    ui.add(
                        egui::Image::new((texture.id(), texture.size_vec2()))
                            .max_width(ui.available_width())
                            .max_height(500.),
                    );
                }
                Err(error) => {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
            }
        }
        if let Some(cache) = cached {
            ui.data_mut(|data| data.insert_temp(id, cache));
        }
    } else if mime.contains("json") || response.body.trim_start().starts_with(['{', '[']) {
        render_response_tree(ui, response);
    } else {
        ui.small("Inert source preview. HTML/XML/scripts are displayed as text; no remote resources execute or load.");
        render_response_syntax(
            ui,
            &response.body,
            if mime.contains("html") {
                "html"
            } else if mime.contains("xml") {
                "xml"
            } else {
                "txt"
            },
        );
    }
}
pub fn render_response_syntax(ui: &mut egui::Ui, body: &str, language: &str) {
    let text = rustapi::utils::truncate_str(body, 64 * 1024);
    if text.len() < body.len() {
        ui.small("Syntax view limited to 64 KiB; use Raw or Download for more data.");
    }
    let theme = egui_extras::syntax_highlighting::CodeTheme::from_style(ui.style());
    egui_extras::syntax_highlighting::code_view_ui(ui, &theme, &text, language);
}
