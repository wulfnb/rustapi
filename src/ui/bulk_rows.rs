use crate::{models::KeyValue, storage};
#[derive(Clone, Default)]
struct BulkDraft {
    text: String,
    error: String,
}
pub fn render_bulk_rows(ui: &mut egui::Ui, rows: &mut Vec<KeyValue>) {
    egui::CollapsingHeader::new("Bulk edit JSON").show(ui,|ui| {
        let id=ui.id().with("bulk_rows_draft");
        let mut draft=ui.data(|data|data.get_temp::<BulkDraft>(id)).unwrap_or_else(||BulkDraft{text:serde_json::to_string_pretty(rows).unwrap_or_default(),..Default::default()});
        ui.small("JSON row array preserves order, disabled flags, repeated keys and metadata. Apply replaces this table only after parsing succeeds.");
        ui.add(egui::TextEdit::multiline(&mut draft.text).code_editor().desired_rows(8).desired_width(f32::INFINITY));
        ui.horizontal(|ui| {
            if ui.button("Apply bulk rows").clicked() {match storage::parse_bulk_rows(&draft.text){Ok(value)=>{*rows=value;draft.error.clear();},Err(error)=>draft.error=error.to_string()}}
            if ui.button("Reload rows into bulk editor").clicked() {draft.text=serde_json::to_string_pretty(rows).unwrap_or_default();draft.error.clear();}
        });
        if !draft.error.is_empty(){ui.colored_label(ui.visuals().error_fg_color,&draft.error);}
        ui.data_mut(|data|data.insert_temp(id,draft));
    });
}
#[derive(Clone, Default)]
struct PresetEditor {
    presets: storage::HeaderPresets,
    name: String,
    error: String,
    loaded: bool,
}
pub fn render_header_presets(ui: &mut egui::Ui, rows: &mut Vec<KeyValue>) {
    egui::CollapsingHeader::new("Header presets").show(ui,|ui| {
        let root=storage::get_app_data_dir();let id=egui::Id::new(("workspace_header_presets",&root));
        let mut editor=ui.data(|data|data.get_temp::<PresetEditor>(id)).unwrap_or_default();
        if !editor.loaded {match storage::HeaderPresets::load(&root){Ok(value)=>editor.presets=value,Err(error)=>editor.error=error.to_string()} editor.loaded=true;}
        ui.text_edit_singleline(&mut editor.name).on_hover_text("New preset name");
        if ui.button("Save headers as preset").clicked() {
            let mut next=editor.presets.clone();
            let result=next.add(&editor.name,rows).and_then(|()|next.save(&root));
            match result{Ok(())=>{editor.presets=next;editor.name.clear();editor.error.clear();},Err(error)=>editor.error=error.to_string()}
        }
        let mut remove=None;
        for (index,preset) in editor.presets.presets.iter().enumerate(){ui.push_id(&preset.id,|ui|{ui.horizontal(|ui|{
            ui.label(&preset.name);
            if ui.button("Append preset").clicked(){rows.extend(preset.headers.clone());}
            if ui.button("Replace headers").clicked(){*rows=preset.headers.clone();}
            if ui.button("Delete preset").clicked(){remove=Some(index);}
        });});}
        if let Some(index)=remove {let mut next=editor.presets.clone();next.presets.remove(index);match next.save(&root){Ok(())=>editor.presets=next,Err(error)=>editor.error=error.to_string()}}
        if ui.button("Reload presets").clicked(){editor.loaded=false;}
        if !editor.error.is_empty(){ui.colored_label(ui.visuals().error_fg_color,&editor.error);}
        ui.small("Presets stay in .local/. Credential fields require variable references. Append preserves duplicates; singleton conflicts are rejected when sending.");
        ui.data_mut(|data|data.insert_temp(id,editor));
    });
}
