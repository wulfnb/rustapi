use super::tool_dialogs as dialogs;
use crate::{models::Collection, storage};
use rustapi::{import::ImportResult, openapi::Specification};
#[cfg(test)]
#[path = "../../tests/unit/api_tools_ui.rs"]
mod tests;
#[derive(Default)]
pub struct ApiTools {
    pub open: bool,
    collection: String,
    spec: String,
    saved: String,
    source: Option<std::path::PathBuf>,
    result: String,
    mock_config: String,
    mock: Option<rustapi::mocks::MockServer>,
    checkpoint: String,
    automation: super::automation_tools::AutomationTools,
}
impl ApiTools {
    pub fn recover() -> Self {
        let root = storage::get_app_data_dir();
        let loaded = (|| -> anyhow::Result<Self> {
            let path = storage::owned_path(&root, &[".local", "specification-draft.json"])?;
            if !path.exists() {
                return Ok(Self::default());
            }
            let text = rustapi::tools_cli::read(&path, 24 * 1024 * 1024)?;
            let draft: serde_json::Value = serde_json::from_str(&text)?;
            Ok(Self {
                spec: draft["text"].as_str().unwrap_or_default().into(),
                saved: draft["saved"].as_str().unwrap_or_default().into(),
                source: draft["source"].as_str().map(std::path::PathBuf::from),
                checkpoint: text,
                ..Default::default()
            })
        })();
        loaded.unwrap_or_else(|error| Self {
            result: format!("Specification draft recovery failed: {error}"),
            ..Default::default()
        })
    }
    pub fn window(
        &mut self,
        ctx: &egui::Context,
        collections: &[Collection],
        service: std::sync::Arc<rustapi::http::HttpService>,
        secrets: &std::collections::BTreeMap<String, String>,
    ) -> Option<ImportResult> {
        if !self.open {
            return None;
        }
        let mut imported = None;
        egui::Window::new("API tools").default_size([850.,650.]).vscroll(true).show(ctx,|ui| {
            ui.heading("OpenAPI specification");
            let dirty=self.spec!=self.saved;
            ui.horizontal(|ui| {
                if ui.add_enabled(!dirty,egui::Button::new("Open specification")).clicked(){if let Some(path)=dialogs::open(){match rustapi::tools_cli::read(&path,8*1024*1024){Ok(text)=>{self.spec=text.clone();self.saved=text;self.source=Some(path);self.result.clear();},Err(error)=>self.result=error.to_string()}}}
                if ui.button("Lint specification").clicked(){self.result=match Specification::parse(&self.spec){Ok(spec)=>serde_json::to_string_pretty(&spec.lint()).unwrap_or_default(),Err(error)=>error.to_string()};}
                if ui.button("Import operations").clicked(){match Specification::parse(&self.spec).and_then(|spec|rustapi::import::json(&serde_json::to_vec(&spec.value)?)){Ok(value)=>imported=Some(value),Err(error)=>self.result=error.to_string()}}
                if dirty&&ui.button("Discard specification edits").clicked(){self.spec=self.saved.clone();}
            });
            if let Some(path)=&self.source {ui.label(path.display().to_string());}
            ui.add(egui::TextEdit::multiline(&mut self.spec).code_editor().desired_rows(12).desired_width(f32::INFINITY));
            ui.horizontal(|ui| {
                if ui.button("Save specification as").clicked(){if let Some(path)=dialogs::save(){
                    let unchanged=Some(&path)!=self.source.as_ref()||std::fs::read_to_string(&path).ok().as_ref()==Some(&self.saved);
                    if !unchanged {self.result="File changed on disk; choose another path or reload before saving.".into();}
                    else {match rustapi::workspace::atomic_write(&path,self.spec.as_bytes()){Ok(())=>{self.source=Some(path);self.saved=self.spec.clone();self.result="Specification saved".into();},Err(error)=>self.result=error.to_string()}}
                }}
                if ui.button("Compare specification…").clicked(){if let Some(path)=dialogs::open(){self.result=(||->anyhow::Result<String>{let before=Specification::parse(&self.spec)?;let after=Specification::parse(&rustapi::tools_cli::read(&path,8*1024*1024)?)?;Ok(serde_json::to_string_pretty(&before.drift(&after)?)?)})().unwrap_or_else(|error|error.to_string());}}
            });
            ui.separator();ui.heading("Collection tools");
            if self.collection.is_empty(){if let Some(c)=collections.first(){self.collection=c.id.clone();}}
            egui::ComboBox::from_id_salt("tools_collection").selected_text(collections.iter().find(|c|c.id==self.collection).map(|c|c.name.as_str()).unwrap_or("Select collection")).show_ui(ui,|ui|{for c in collections{ui.selectable_value(&mut self.collection,c.id.clone(),&c.name);}});
            if let Some(collection)=collections.iter().find(|c|c.id==self.collection) {
                if ui.button("Generate offline Markdown + HTML documentation").clicked(){if let Some(parent)=dialogs::folder(){let path=parent.join(format!("rustapi-docs-{}",uuid::Uuid::new_v4().simple()));self.result=match rustapi::documentation::write(collection,&path,false){Ok(())=>format!("Documentation written to {}",path.display()),Err(error)=>error.to_string()};}}
                ui.small("Uses saved requests and examples. Credential fields are redacted. Generated files can be hosted as static documentation.");
                if ui.button("Create mock rules from saved examples").clicked(){match rustapi::mocks::MockConfig::from_examples(collection){Ok(config)=>self.mock_config=serde_json::to_string_pretty(&config).unwrap_or_default(),Err(error)=>self.result=error.to_string()}}
            }
            egui::CollapsingHeader::new("Mock matching rules, response delays and faults").show(ui,|ui| {
                ui.small("Rules are evaluated in order. Set path/query/headers/body_contains/json matchers, delay_ms, status or disconnect. X-RustAPI-Example selects a named example.");
                ui.add(egui::TextEdit::multiline(&mut self.mock_config).code_editor().desired_rows(8).desired_width(f32::INFINITY));
                ui.horizontal(|ui| {
                    if ui.button("Load mock profile").clicked(){if let Some(path)=dialogs::open(){match rustapi::tools_cli::read(&path,16*1024*1024){Ok(text)=>self.mock_config=text,Err(error)=>self.result=error.to_string()}}}
                    if ui.button("Save mock profile").clicked(){if let Some(path)=dialogs::save(){let result=(||->anyhow::Result<()>{let config:rustapi::mocks::MockConfig=serde_json::from_str(&self.mock_config)?;config.validate()?;rustapi::workspace::atomic_write(&path,&serde_json::to_vec_pretty(&config)?)})();self.result=result.map(|_|"Mock profile saved".into()).unwrap_or_else(|error|error.to_string());}}
                    if ui.add_enabled(self.mock.is_none(),egui::Button::new("Start local mock")).clicked(){match serde_json::from_str(&self.mock_config).map_err(anyhow::Error::from).and_then(rustapi::mocks::MockServer::start){Ok(server)=>self.mock=Some(server),Err(error)=>self.result=error.to_string()}}
                });
            });
            if let Some(mock)=&self.mock {ui.label(format!("Mock: http://{} · {} requests · {} unmatched",mock.server.address,mock.requests.load(std::sync::atomic::Ordering::Relaxed),mock.unmatched.load(std::sync::atomic::Ordering::Relaxed)));if ui.button("Stop local mock").clicked(){self.mock=None;}}
            if !self.result.is_empty(){ui.separator();ui.label(&self.result);}
            self.automation.render(ui,collections,service.clone(),secrets);
            if ui.button("Close API tools").clicked(){self.open=false;}
        });
        let draft = serde_json::json!({"text":self.spec,"saved":self.saved,"source":self.source})
            .to_string();
        if draft != self.checkpoint {
            let result = (|| -> anyhow::Result<()> {
                anyhow::ensure!(
                    draft.len() <= 24 * 1024 * 1024,
                    "Specification draft exceeds 24 MiB"
                );
                let path = storage::owned_path(
                    &storage::get_app_data_dir(),
                    &[".local", "specification-draft.json"],
                )?;
                rustapi::workspace::atomic_write(&path, draft.as_bytes())
            })();
            match result {
                Ok(()) => self.checkpoint = draft,
                Err(error) => self.result = error.to_string(),
            }
        }
        imported
    }
}
