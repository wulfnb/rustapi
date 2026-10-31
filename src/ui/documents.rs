use crate::storage;
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct DocumentEditor {
    pub open: bool,
    pub name: String,
    pub text: String,
    saved: String,
    loaded_name: String,
    #[serde(skip)]
    pub error: String,
    #[serde(skip)]
    fingerprint: u64,
}
impl DocumentEditor {
    pub fn recover() -> Self {
        let root = storage::get_app_data_dir();
        let result = (|| -> anyhow::Result<Self> {
            let path = storage::owned_path(&root, &[".local", "document-draft.json"])?;
            if !path.exists() {
                return Ok(Self::default());
            }
            anyhow::ensure!(
                std::fs::metadata(&path)?.len() <= 4 * 1024 * 1024,
                "Document draft exceeds 4 MiB"
            );
            Ok(serde_json::from_slice(&std::fs::read(path)?)?)
        })();
        match result {
            Ok(mut draft) => {
                draft.fingerprint = draft.hash();
                draft
            }
            Err(error) => Self {
                error: format!("Document draft recovery failed: {error}"),
                ..Default::default()
            },
        }
    }
    fn hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (&self.name, &self.text, &self.saved, &self.loaded_name).hash(&mut hasher);
        hasher.finish()
    }
    fn checkpoint(&mut self) {
        let hash = self.hash();
        if hash == self.fingerprint {
            return;
        }
        let result = (|| -> anyhow::Result<()> {
            let bytes = serde_json::to_vec(self)?;
            anyhow::ensure!(
                bytes.len() <= 4 * 1024 * 1024,
                "Document draft exceeds 4 MiB"
            );
            let path = storage::owned_path(
                &storage::get_app_data_dir(),
                &[".local", "document-draft.json"],
            )?;
            rustapi::workspace::atomic_write(&path, &bytes)
        })();
        match result {
            Ok(()) => self.fingerprint = hash,
            Err(error) => self.error = format!("Document draft save failed: {error}"),
        }
    }

    pub fn window(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }
        let root = storage::get_app_data_dir();
        egui::Window::new("Workspace documents").default_size([760.,550.]).vscroll(true).show(ctx,|ui| {
            let dirty=self.text!=self.saved || self.name!=self.loaded_name;
            match storage::list_documents(&root) {
                Ok(names)=>{ for name in names { if ui.add_enabled(!dirty,egui::Button::new(&name)).clicked() {
                    match storage::read_document(&root,&name) { Ok(text)=>{ self.text=text.clone(); self.saved=text; self.name=name.clone(); self.loaded_name=name;self.error.clear(); }, Err(error)=>self.error=error.to_string() }
                } } }, Err(error)=>{self.error=error.to_string();}
            }
            if ui.add_enabled(!dirty,egui::Button::new("New document")).clicked() {self.name="WORKSPACE-README.md".into();self.loaded_name.clear();self.text.clear();self.saved.clear();}
            if dirty { ui.label("Save or discard changes before opening another document."); }
            ui.label("Markdown filename");ui.text_edit_singleline(&mut self.name);
            ui.add(egui::TextEdit::multiline(&mut self.text).code_editor().desired_width(f32::INFINITY).desired_rows(18));
            ui.horizontal(|ui| {
                if ui.button("Save document").clicked() {
                    // Renaming creates a new document; explicitly prevent overwriting another one.
                    let exists=storage::list_documents(&root).map(|names| names.contains(&self.name)).unwrap_or(true);
                    if self.name!=self.loaded_name && exists {self.error="A document with this name already exists. Open it before editing.".into();}
                    else { match storage::write_document(&root,&self.name,&self.text) { Ok(())=>{self.saved=self.text.clone();self.loaded_name=self.name.clone();self.error.clear();}, Err(error)=>self.error=error.to_string() } }
                }
                if ui.button("Discard document changes").clicked() {self.text=self.saved.clone();self.name=self.loaded_name.clone();self.error.clear();}
                if ui.button("Close documents").clicked() {self.open=false;}
            });
            if !self.error.is_empty() {ui.colored_label(ui.visuals().error_fg_color,&self.error);}
            ui.small("Documents are UTF-8 Markdown files in documents/. Edits stay in memory until Save; drafts recover after restart. A new filename saves a copy.");
        });
        self.checkpoint();
    }
}
