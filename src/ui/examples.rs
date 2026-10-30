use crate::models::*;

/// Returns an independent request snapshot when Try is selected.
pub fn render_examples(
    ui: &mut egui::Ui,
    request: &mut ApiRequest,
    response: Option<&ApiResponse>,
) -> Option<RequestExample> {
    let mut tried = None;
    egui::CollapsingHeader::new(format!("Saved examples ({})", request.examples.len())).id_salt("saved_examples").show(ui, |ui| {
        if ui.add_enabled(response.is_some() && request.examples.len() < 100, egui::Button::new("Save response as example")).clicked() {
            if let Some(response) = response {
                match RequestExample::capture(format!("{} {}", request.name, response.status), request, response) {
                    Ok(example) => {
                        let mut redacted = request.clone(); redacted.examples = vec![example];
                        request.examples.push(rustapi::secrets::redact_request(&redacted).examples.remove(0));
                    }
                    Err(error) => { ui.data_mut(|data| data.insert_temp(ui.id().with("example_error"), error.to_string())); }
                }
            }
        }
        if let Some(error) = ui.data(|data| data.get_temp::<String>(ui.id().with("example_error"))) { ui.colored_label(egui::Color32::LIGHT_RED, error); }
        let mut removed = None; let mut copy = None;
        for (index, example) in request.examples.iter_mut().enumerate() {
            egui::CollapsingHeader::new(&example.name).id_salt(&example.id).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Example name"); ui.text_edit_singleline(&mut example.name);
                    if ui.button("Try").clicked() { tried = Some(example.clone()); }
                    if ui.button("Duplicate example").clicked() { let mut value = example.clone(); value.id=uuid::Uuid::new_v4().to_string(); value.name.push_str(" (copy)"); copy=Some(value); }
                    if ui.button("Delete example").clicked() { removed=Some(index); }
                });
                ui.label("Saved request URL"); ui.text_edit_singleline(&mut example.request.url);
                ui.label("Saved request headers"); super::render_key_value_editor(ui, &mut example.request.headers);
                ui.label("Saved request body"); ui.add(egui::TextEdit::multiline(&mut example.request.body).code_editor());
                ui.horizontal(|ui| { ui.label("Status"); ui.add(egui::DragValue::new(&mut example.response.status).range(100..=599)); ui.text_edit_singleline(&mut example.response.status_text); });
                ui.label("Response headers");
                let mut remove_header=None;
                for (i,(key,value)) in example.response.headers.iter_mut().enumerate() { ui.push_id(i, |ui| { ui.horizontal(|ui| { ui.text_edit_singleline(key); ui.text_edit_singleline(value); if ui.button("Remove header").clicked(){remove_header=Some(i);} }); }); }
                if let Some(i)=remove_header { example.response.headers.remove(i); }
                if ui.button("Add response header").clicked() {example.response.headers.push(Default::default());}
                ui.label("Response body (editing replaces saved binary bytes with UTF-8)");
                let mut text=example.response.body.clone();
                if ui.add(egui::TextEdit::multiline(&mut text).code_editor().desired_rows(5)).changed() && text.len()<=1024*1024 { example.set_text(text); }
            });
        }
        if let Some(index)=removed { request.examples.remove(index); }
        if let Some(value)=copy { if request.examples.len()<100 {request.examples.push(value);} }
        ui.small("Save the parent request to persist edits. Try opens a separate tab. Captured examples redact recognized sensitive fields.");
    });
    tried
}
