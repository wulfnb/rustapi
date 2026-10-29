//! Main application state and update loop

use crate::http::{execute_request, CodeGenLanguage};
use crate::models::*;
use crate::storage::{self, AppState};
use crate::ui::*;
use eframe::egui;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

/// Keep native dialogs at the UI boundary so commands can be exercised independently.
trait CollectionDialogs {
    fn import(&self) -> Option<std::path::PathBuf>;
    fn export(&self, name: &str) -> Option<std::path::PathBuf>;
}

struct NativeCollectionDialogs;

impl CollectionDialogs for NativeCollectionDialogs {
    fn import(&self) -> Option<std::path::PathBuf> {
        rfd::FileDialog::new()
            .set_title("Import collection")
            .add_filter("JSON", &["json"])
            .pick_file()
    }

    fn export(&self, name: &str) -> Option<std::path::PathBuf> {
        rfd::FileDialog::new()
            .set_title("Export collection")
            .add_filter("JSON", &["json"])
            .set_file_name(format!("{name}.json"))
            .save_file()
    }
}

/// Message for async operations
pub enum AsyncMessage {
    ResponseReceived(ApiResponse),
}

/// Main application state
pub struct RustApiApp {
    dialogs: Box<dyn CollectionDialogs>,
    // Collections and environments
    pub collections: Vec<Collection>,
    pub environments: Vec<Environment>,
    pub active_environment_id: Option<String>,

    // Tab management
    pub tab_manager: TabManager,

    // Response state
    pub is_loading: bool,

    // History
    pub history: RequestHistory,

    // UI state
    pub request_tab: RequestTab,
    pub response_tab: ResponseTab,
    pub show_new_collection_modal: bool,
    pub show_environment_editor: bool,
    pub show_rename_modal: bool,
    pub show_code_gen_modal: bool,
    pub show_history_panel: bool,
    pub rename_target: RenameTarget,
    pub rename_value: String,
    pub new_item_name: String,
    pub sidebar_width: f32,
    pub response_format_json: bool,
    pub search_query: String,
    pub code_gen_language: CodeGenLanguage,

    // Async communication
    pub async_sender: Sender<AsyncMessage>,
    pub async_receiver: Receiver<AsyncMessage>,
}

impl Default for RustApiApp {
    fn default() -> Self {
        let (tx, rx) = channel();

        // Initialize storage and load data
        let _ = storage::init_storage();
        let collections = storage::load_all_collections();
        let environments = storage::load_all_environments();
        let state = storage::load_app_state().unwrap_or_default();
        let history = storage::load_history();

        Self {
            dialogs: Box::new(NativeCollectionDialogs),
            collections: if collections.is_empty() {
                vec![Collection::new("My Collection")]
            } else {
                collections
            },
            environments,
            active_environment_id: state.active_environment_id,
            tab_manager: TabManager::new(),
            is_loading: false,
            history,
            request_tab: RequestTab::default(),
            response_tab: ResponseTab::default(),
            show_new_collection_modal: false,
            show_environment_editor: false,
            show_rename_modal: false,
            show_code_gen_modal: false,
            show_history_panel: false,
            rename_target: RenameTarget::None,
            rename_value: String::new(),
            new_item_name: String::new(),
            sidebar_width: 280.0,
            response_format_json: true,
            search_query: String::new(),
            code_gen_language: CodeGenLanguage::default(),
            async_sender: tx,
            async_receiver: rx,
        }
    }
}

impl RustApiApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
    }

    /// Get the active environment
    fn get_active_environment(&self) -> Option<&Environment> {
        self.active_environment_id
            .as_ref()
            .and_then(|id| self.environments.iter().find(|e| &e.id == id))
    }

    /// Send the current request
    fn send_request(&mut self) {
        let tab = match self.tab_manager.active_tab() {
            Some(t) => t,
            None => return,
        };

        if tab.request.url.is_empty() {
            return;
        }

        self.is_loading = true;

        let request = tab.request.clone();
        let environment = self.get_active_environment().cloned();
        let sender = self.async_sender.clone();

        thread::spawn(move || {
            let response = execute_request(&request, environment.as_ref());
            let _ = sender.send(AsyncMessage::ResponseReceived(response));
        });
    }

    /// Save current request to collection
    fn save_current_request(&mut self) {
        if let Some(tab) = self.tab_manager.active_tab() {
            if let (Some(col_idx), Some(req_idx)) = (tab.collection_idx, tab.request_idx) {
                if let Some(collection) = self.collections.get_mut(col_idx) {
                    if let Some(request) = collection.requests.get_mut(req_idx) {
                        *request = tab.request.clone();
                        request.updated_at = chrono::Utc::now();
                        let _ = storage::save_collection(collection, None);
                        self.tab_manager.mark_saved();
                    }
                }
            }
        }
    }

    /// Add new request to collection
    fn add_request_to_collection(&mut self, col_idx: usize) {
        if let Some(collection) = self.collections.get_mut(col_idx) {
            let request = ApiRequest::new(format!("Request {}", collection.requests.len() + 1));
            collection.requests.push(request.clone());
            let _ = storage::save_collection(collection, None);

            let req_idx = collection.requests.len() - 1;
            self.tab_manager
                .open_tab(request, Some(col_idx), Some(req_idx));
        }
    }

    /// Duplicate a request
    fn duplicate_request(&mut self, col_idx: usize, req_idx: usize) {
        if let Some(collection) = self.collections.get_mut(col_idx) {
            if let Some(request) = collection.requests.get(req_idx) {
                let new_request = request.duplicate();
                collection.requests.push(new_request);
                let _ = storage::save_collection(collection, None);
            }
        }
    }

    /// Delete request from collection
    fn delete_request(&mut self, col_idx: usize, req_idx: usize) {
        if let Some(collection) = self.collections.get_mut(col_idx) {
            if req_idx < collection.requests.len() {
                collection.requests.remove(req_idx);
                let _ = storage::save_collection(collection, None);

                // Close tab if open
                if let Some(tab_idx) = self.tab_manager.tabs.iter().position(|t| {
                    t.collection_idx == Some(col_idx) && t.request_idx == Some(req_idx)
                }) {
                    self.tab_manager.close_tab(tab_idx);
                }
            }
        }
    }

    /// Create new collection
    fn create_collection(&mut self, name: String) {
        let collection = Collection::new(name);
        let _ = storage::save_collection(&collection, None);
        self.collections.push(collection);
    }

    /// Import into the managed profile without rewriting the user's source file.
    fn import_collection(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        let mut collection = storage::import_collection(path)?;
        collection.file_path = None;
        let saved_path = storage::save_collection(&collection, None)?;
        collection.file_path = Some(saved_path.to_string_lossy().into_owned());
        self.collections.push(collection);
        Ok(())
    }

    /// Delete collection
    fn delete_collection(&mut self, col_idx: usize) {
        if col_idx < self.collections.len() {
            let collection = self.collections.remove(col_idx);
            let _ = storage::delete_collection(&collection);

            // Close any tabs from this collection
            let tabs_to_close: Vec<usize> = self
                .tab_manager
                .tabs
                .iter()
                .enumerate()
                .filter(|(_, t)| t.collection_idx == Some(col_idx))
                .map(|(i, _)| i)
                .collect();

            for idx in tabs_to_close.into_iter().rev() {
                self.tab_manager.close_tab(idx);
            }
        }
    }

    fn handle_sidebar_action(&mut self, action: SidebarAction) {
        match action {
            SidebarAction::SelectRequest(col_idx, req_idx) => {
                if let Some(collection) = self.collections.get(col_idx) {
                    if let Some(request) = collection.requests.get(req_idx) {
                        self.tab_manager
                            .open_tab(request.clone(), Some(col_idx), Some(req_idx));
                    }
                }
            }
            SidebarAction::AddRequest(col_idx) => {
                self.add_request_to_collection(col_idx);
            }
            SidebarAction::DeleteRequest(col_idx, req_idx) => {
                self.delete_request(col_idx, req_idx);
            }
            SidebarAction::DuplicateRequest(col_idx, req_idx) => {
                self.duplicate_request(col_idx, req_idx);
            }
            SidebarAction::RenameRequest(col_idx, req_idx) => {
                if let Some(collection) = self.collections.get(col_idx) {
                    if let Some(request) = collection.requests.get(req_idx) {
                        self.rename_value = request.name.clone();
                        self.rename_target = RenameTarget::Request(col_idx, req_idx);
                        self.show_rename_modal = true;
                    }
                }
            }
            SidebarAction::DeleteCollection(col_idx) => {
                self.delete_collection(col_idx);
            }
            SidebarAction::RenameCollection(col_idx) => {
                if let Some(collection) = self.collections.get(col_idx) {
                    self.rename_value = collection.name.clone();
                    self.rename_target = RenameTarget::Collection(col_idx);
                    self.show_rename_modal = true;
                }
            }
            SidebarAction::ExportCollection(col_idx) => {
                if let Some(collection) = self.collections.get(col_idx) {
                    if let Some(path) = self.dialogs.export(&collection.name) {
                        let _ = storage::export_collection(collection, &path);
                    }
                }
            }
            SidebarAction::NewCollection => {
                self.new_item_name.clear();
                self.show_new_collection_modal = true;
            }
        }
    }

    /// Save app state
    fn save_state(&self) {
        let state = AppState {
            active_environment_id: self.active_environment_id.clone(),
            recent_files: vec![],
            window_size: None,
        };
        let _ = storage::save_app_state(&state);
        let _ = storage::save_history(&self.history);
    }
}

impl eframe::App for RustApiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.render(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_state();
    }
}

impl RustApiApp {
    /// Render a frame independently of the native window adapter.
    pub(crate) fn render(&mut self, ctx: &egui::Context) {
        // Process async messages
        while let Ok(msg) = self.async_receiver.try_recv() {
            match msg {
                AsyncMessage::ResponseReceived(response) => {
                    // Add to history
                    if let Some(tab) = self.tab_manager.active_tab() {
                        self.history
                            .add(tab.request.clone(), Some(response.clone()));
                    }

                    // Set response on active tab
                    if let Some(tab) = self.tab_manager.active_tab_mut() {
                        tab.response = Some(response);
                    }
                    self.is_loading = false;
                }
            }
        }

        // Request repaint while loading
        if self.is_loading {
            ctx.request_repaint();
        }

        // Set custom style
        apply_dark_theme(ctx);

        // Top panel with environment selector
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            let mut show_env_editor = false;
            let mut show_import = false;
            let mut show_history = false;

            render_top_bar(
                ui,
                &self.environments,
                &mut self.active_environment_id,
                &mut show_env_editor,
                &mut show_import,
                &mut show_history,
            );

            if show_env_editor {
                self.show_environment_editor = true;
            }
            if show_history {
                self.show_history_panel = true;
            }
            if show_import {
                if let Some(path) = self.dialogs.import() {
                    let _ = self.import_collection(&path);
                }
            }
        });

        // Left panel with collections
        egui::SidePanel::left("sidebar")
            .resizable(true)
            .default_width(self.sidebar_width)
            .width_range(200.0..=400.0)
            .show(ctx, |ui| {
                // Get current selection for highlighting
                let (current_col, current_req) = self
                    .tab_manager
                    .active_tab()
                    .map(|t| (t.collection_idx, t.request_idx))
                    .unwrap_or((None, None));

                if let Some(action) = render_sidebar(
                    ui,
                    &mut self.collections,
                    &mut self.search_query,
                    current_col,
                    current_req,
                ) {
                    self.handle_sidebar_action(action);
                }
            });

        // Main content area
        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_main_content(ui);
        });

        // egui floating windows must follow the top-level panels.
        self.render_modals(ctx);
    }
}

impl RustApiApp {
    /// Render modals
    fn render_modals(&mut self, ctx: &egui::Context) {
        // New collection modal
        if let Some(name) = render_new_collection_modal(
            ctx,
            &mut self.show_new_collection_modal,
            &mut self.new_item_name,
        ) {
            self.create_collection(name);
        }

        // Rename modal
        let current_request_name = self
            .tab_manager
            .active_tab_mut()
            .map(|t| &mut t.request.name);

        render_rename_modal(
            ctx,
            &mut self.show_rename_modal,
            &self.rename_target.clone(),
            &mut self.rename_value,
            &mut self.collections,
            current_request_name,
        );

        // Environment editor
        render_environment_editor(
            ctx,
            &mut self.show_environment_editor,
            &mut self.environments,
            &mut self.active_environment_id,
        );

        // Code generation modal
        if self.show_code_gen_modal {
            if let Some(tab) = self.tab_manager.active_tab() {
                let request = tab.request.clone();
                let env = self.get_active_environment().cloned();
                render_code_gen_modal(
                    ctx,
                    &mut self.show_code_gen_modal,
                    &request,
                    env.as_ref(),
                    &mut self.code_gen_language,
                );
            }
        }

        // History panel
        if let Some(action) = render_history_panel(ctx, &mut self.show_history_panel, &self.history)
        {
            match action {
                HistoryAction::LoadRequest(request) => {
                    self.tab_manager.open_tab(*request, None, None);
                }
                HistoryAction::ClearHistory => {
                    self.history.clear();
                    let _ = storage::save_history(&self.history);
                }
            }
        }
    }

    /// Render main content area
    fn render_main_content(&mut self, ui: &mut egui::Ui) {
        // Tab bar
        if self.tab_manager.has_tabs() {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(16.0);

                let mut tab_to_close: Option<usize> = None;
                let mut tab_to_select: Option<usize> = None;

                for (idx, tab) in self.tab_manager.tabs.iter().enumerate() {
                    let is_active = self.tab_manager.active_tab_idx == Some(idx);

                    ui.push_id(&tab.id, |ui| {
                        let tab_response = ui.selectable_label(is_active, tab.title());

                        if tab_response.clicked() {
                            tab_to_select = Some(idx);
                        }

                        // Close button
                        if ui.small_button("×").clicked() {
                            tab_to_close = Some(idx);
                        }

                        ui.separator();
                    });
                }

                if let Some(idx) = tab_to_select {
                    self.tab_manager.active_tab_idx = Some(idx);
                }
                if let Some(idx) = tab_to_close {
                    self.tab_manager.close_tab(idx);
                }

                // New tab button
                if ui.small_button("➕").on_hover_text("New Request").clicked() {
                    let request = ApiRequest::default();
                    self.tab_manager.open_tab(request, None, None);
                }
            });
            ui.separator();
        }

        // If no tabs, show welcome message
        if !self.tab_manager.has_tabs() {
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(100.0);
                    ui.heading("🦀 RustAPI");
                    ui.add_space(20.0);
                    ui.label("Select a request from the sidebar or create a new one");
                    ui.add_space(20.0);
                    if ui.button("➕ New Request").clicked() {
                        let request = ApiRequest::default();
                        self.tab_manager.open_tab(request, None, None);
                    }
                });
            });
            return;
        }

        // Get active tab info
        let (has_save_target, is_loading, response) = {
            let tab = self.tab_manager.active_tab();
            (
                tab.map(|t| t.collection_idx.is_some()).unwrap_or(false),
                self.is_loading,
                tab.and_then(|t| t.response.clone()),
            )
        };

        ui.add_space(8.0);

        // URL bar
        let mut on_send = false;
        let mut on_generate_code = false;

        if let Some(tab) = self.tab_manager.active_tab_mut() {
            render_url_bar(
                ui,
                &mut tab.request,
                is_loading,
                &mut on_send,
                &mut on_generate_code,
            );
        }

        if on_send {
            self.send_request();
        }
        if on_generate_code {
            self.show_code_gen_modal = true;
        }

        ui.add_space(16.0);

        // Split view: Request editor and Response viewer
        let available_height = ui.available_height();
        let request_height = available_height * 0.45;
        let response_height = available_height * 0.50;

        // Request editor
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(40, 44, 52))
            .rounding(8.0)
            .inner_margin(12.0)
            .outer_margin(egui::Margin::symmetric(16.0, 0.0))
            .show(ui, |ui| {
                ui.set_min_height(request_height);

                let mut on_save = false;
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    render_request_editor(
                        ui,
                        &mut tab.request,
                        &mut self.request_tab,
                        &mut on_save,
                        has_save_target,
                    );
                }

                if on_save {
                    self.save_current_request();
                }
            });

        ui.add_space(16.0);

        // Response viewer
        egui::Frame::none()
            .fill(egui::Color32::from_rgb(40, 44, 52))
            .rounding(8.0)
            .inner_margin(12.0)
            .outer_margin(egui::Margin::symmetric(16.0, 0.0))
            .show(ui, |ui| {
                ui.set_min_height(response_height);

                render_response_viewer(
                    ui,
                    response.as_ref(),
                    is_loading,
                    &mut self.response_tab,
                    &mut self.response_format_json,
                );
            });
    }
}

#[cfg(test)]
#[path = "../tests/unit/app.rs"]
mod tests;
