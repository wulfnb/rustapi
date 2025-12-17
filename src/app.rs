//! Main application state and UI

use crate::http_client::{execute_request, format_json};
use crate::models::*;
use crate::storage;
use eframe::egui;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

/// Tabs for the request editor
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RequestTab {
    #[default]
    Params,
    Headers,
    Body,
}

/// Tabs for the response viewer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResponseTab {
    #[default]
    Body,
    Headers,
}

/// Message for async operations
pub enum AsyncMessage {
    ResponseReceived(ApiResponse),
}

/// Main application state
pub struct RustApiApp {
    // Collections and environments
    pub collections: Vec<Collection>,
    pub environments: Vec<Environment>,
    pub active_environment_id: Option<String>,

    // Current request being edited
    pub current_request: ApiRequest,
    pub current_collection_idx: Option<usize>,
    pub current_request_idx: Option<usize>,

    // Response state
    pub response: Option<ApiResponse>,
    pub is_loading: bool,

    // UI state
    pub request_tab: RequestTab,
    pub response_tab: ResponseTab,
    pub show_new_collection_modal: bool,
    pub show_new_environment_modal: bool,
    pub show_environment_editor: bool,
    pub new_item_name: String,
    pub sidebar_width: f32,
    pub response_format_json: bool,

    // Async communication
    pub async_sender: Sender<AsyncMessage>,
    pub async_receiver: Receiver<AsyncMessage>,

    // Search
    pub search_query: String,
}

impl Default for RustApiApp {
    fn default() -> Self {
        let (tx, rx) = channel();

        // Initialize storage and load data
        let _ = storage::init_storage();
        let collections = storage::load_all_collections();
        let environments = storage::load_all_environments();
        let state = storage::load_app_state().unwrap_or_default();

        Self {
            collections: if collections.is_empty() {
                vec![Collection::new("My Collection")]
            } else {
                collections
            },
            environments,
            active_environment_id: state.active_environment_id,
            current_request: ApiRequest::default(),
            current_collection_idx: None,
            current_request_idx: None,
            response: None,
            is_loading: false,
            request_tab: RequestTab::default(),
            response_tab: ResponseTab::default(),
            show_new_collection_modal: false,
            show_new_environment_modal: false,
            show_environment_editor: false,
            new_item_name: String::new(),
            sidebar_width: 280.0,
            response_format_json: true,
            async_sender: tx,
            async_receiver: rx,
            search_query: String::new(),
        }
    }
}

impl RustApiApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
    }

    /// Get the active environment
    fn get_active_environment(&self) -> Option<&Environment> {
        self.active_environment_id.as_ref().and_then(|id| {
            self.environments.iter().find(|e| &e.id == id)
        })
    }

    /// Send the current request
    fn send_request(&mut self) {
        if self.current_request.url.is_empty() {
            return;
        }

        self.is_loading = true;
        self.response = None;

        let request = self.current_request.clone();
        let environment = self.get_active_environment().cloned();
        let sender = self.async_sender.clone();

        thread::spawn(move || {
            let response = execute_request(&request, environment.as_ref());
            let _ = sender.send(AsyncMessage::ResponseReceived(response));
        });
    }

    /// Save current request to collection
    fn save_current_request(&mut self) {
        if let Some(col_idx) = self.current_collection_idx {
            if let Some(req_idx) = self.current_request_idx {
                if let Some(collection) = self.collections.get_mut(col_idx) {
                    if let Some(request) = collection.requests.get_mut(req_idx) {
                        *request = self.current_request.clone();
                        request.updated_at = chrono::Utc::now();
                        let _ = storage::save_collection(collection, None);
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
            
            self.current_collection_idx = Some(col_idx);
            self.current_request_idx = Some(collection.requests.len() - 1);
            self.current_request = request;
        }
    }

    /// Delete request from collection
    fn delete_request(&mut self, col_idx: usize, req_idx: usize) {
        if let Some(collection) = self.collections.get_mut(col_idx) {
            if req_idx < collection.requests.len() {
                collection.requests.remove(req_idx);
                let _ = storage::save_collection(collection, None);
                
                // Clear current request if it was deleted
                if self.current_collection_idx == Some(col_idx) 
                    && self.current_request_idx == Some(req_idx) 
                {
                    self.current_request = ApiRequest::default();
                    self.current_request_idx = None;
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

    /// Delete collection
    fn delete_collection(&mut self, col_idx: usize) {
        if col_idx < self.collections.len() {
            let collection = self.collections.remove(col_idx);
            let _ = storage::delete_collection(&collection);
            
            if self.current_collection_idx == Some(col_idx) {
                self.current_collection_idx = None;
                self.current_request_idx = None;
                self.current_request = ApiRequest::default();
            }
        }
    }

    /// Create new environment
    fn create_environment(&mut self, name: String) {
        let env = Environment::new(name);
        let _ = storage::save_environment(&env);
        self.environments.push(env);
    }

    /// Save app state
    fn save_state(&self) {
        let state = AppState {
            collections: self.collections.clone(),
            environments: self.environments.clone(),
            active_environment_id: self.active_environment_id.clone(),
            recent_files: vec![],
            window_size: None,
        };
        let _ = storage::save_app_state(&state);
    }
}

impl eframe::App for RustApiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process async messages
        while let Ok(msg) = self.async_receiver.try_recv() {
            match msg {
                AsyncMessage::ResponseReceived(response) => {
                    self.response = Some(response);
                    self.is_loading = false;
                }
            }
        }

        // Request repaint while loading
        if self.is_loading {
            ctx.request_repaint();
        }

        // Set custom style
        self.setup_style(ctx);

        // Render modals
        self.render_modals(ctx);

        // Top panel with environment selector
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            self.render_top_bar(ui);
        });

        // Left panel with collections
        egui::SidePanel::left("sidebar")
            .resizable(true)
            .default_width(self.sidebar_width)
            .width_range(200.0..=400.0)
            .show(ctx, |ui| {
                self.render_sidebar(ui);
            });

        // Main content area
        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_main_content(ui);
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_state();
    }
}

impl RustApiApp {
    /// Setup custom style
    fn setup_style(&self, ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        
        // Dark theme colors - inspired by One Dark
        let bg_dark = egui::Color32::from_rgb(30, 33, 39);
        let bg_medium = egui::Color32::from_rgb(40, 44, 52);
        let bg_light = egui::Color32::from_rgb(50, 55, 65);
        let accent = egui::Color32::from_rgb(97, 175, 239);
        let text_primary = egui::Color32::from_rgb(220, 223, 228);
        let text_secondary = egui::Color32::from_rgb(150, 155, 165);

        style.visuals.dark_mode = true;
        style.visuals.panel_fill = bg_dark;
        style.visuals.window_fill = bg_medium;
        style.visuals.widgets.noninteractive.bg_fill = bg_medium;
        style.visuals.widgets.inactive.bg_fill = bg_light;
        style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(60, 65, 75);
        style.visuals.widgets.active.bg_fill = accent;
        style.visuals.selection.bg_fill = accent.linear_multiply(0.3);
        style.visuals.override_text_color = Some(text_primary);
        style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, text_secondary);
        
        // Rounded corners
        style.visuals.window_rounding = egui::Rounding::same(8.0);
        style.visuals.widgets.noninteractive.rounding = egui::Rounding::same(6.0);
        style.visuals.widgets.inactive.rounding = egui::Rounding::same(6.0);
        style.visuals.widgets.hovered.rounding = egui::Rounding::same(6.0);
        style.visuals.widgets.active.rounding = egui::Rounding::same(6.0);

        ctx.set_style(style);
    }

    /// Render top bar
    fn render_top_bar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            
            // Logo/Title
            ui.heading(egui::RichText::new("🦀 RustAPI").strong().size(20.0));
            
            ui.add_space(20.0);
            ui.separator();
            ui.add_space(20.0);

            // Environment selector
            ui.label("Environment:");
            egui::ComboBox::from_id_salt("env_selector")
                .selected_text(
                    self.active_environment_id
                        .as_ref()
                        .and_then(|id| self.environments.iter().find(|e| &e.id == id))
                        .map(|e| e.name.as_str())
                        .unwrap_or("No Environment")
                )
                .show_ui(ui, |ui| {
                    if ui.selectable_label(self.active_environment_id.is_none(), "No Environment").clicked() {
                        self.active_environment_id = None;
                    }
                    for env in &self.environments {
                        if ui.selectable_label(
                            self.active_environment_id.as_ref() == Some(&env.id),
                            &env.name
                        ).clicked() {
                            self.active_environment_id = Some(env.id.clone());
                        }
                    }
                });

            if ui.button("⚙").on_hover_text("Manage Environments").clicked() {
                self.show_environment_editor = true;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("📁 Import").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("JSON", &["json"])
                        .pick_file()
                    {
                        if let Ok(collection) = storage::import_collection(&path) {
                            let _ = storage::save_collection(&collection, None);
                            self.collections.push(collection);
                        }
                    }
                }
            });
        });
        ui.add_space(8.0);
    }

    /// Render sidebar
    fn render_sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        
        // Search box
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            let search_edit = egui::TextEdit::singleline(&mut self.search_query)
                .hint_text("🔍 Search requests...")
                .desired_width(ui.available_width() - 16.0);
            ui.add(search_edit);
        });
        
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        // Collections header
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.strong("Collections");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                if ui.small_button("➕").on_hover_text("New Collection").clicked() {
                    self.new_item_name = String::new();
                    self.show_new_collection_modal = true;
                }
            });
        });

        ui.add_space(8.0);

        // Collections list
        egui::ScrollArea::vertical().show(ui, |ui| {
            let mut action: Option<SidebarAction> = None;

            for (col_idx, collection) in self.collections.iter_mut().enumerate() {
                ui.add_space(4.0);
                
                // Collection header
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    let icon = if collection.expanded { "▼" } else { "▶" };
                    if ui.small_button(icon).clicked() {
                        collection.expanded = !collection.expanded;
                    }
                    
                    let col_label = ui.selectable_label(false, &collection.name);
                    if col_label.clicked() {
                        collection.expanded = !collection.expanded;
                    }

                    col_label.context_menu(|ui| {
                        if ui.button("➕ Add Request").clicked() {
                            action = Some(SidebarAction::AddRequest(col_idx));
                            ui.close_menu();
                        }
                        if ui.button("📤 Export").clicked() {
                            action = Some(SidebarAction::ExportCollection(col_idx));
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("🗑 Delete").clicked() {
                            action = Some(SidebarAction::DeleteCollection(col_idx));
                            ui.close_menu();
                        }
                    });
                });

                // Requests in collection
                if collection.expanded {
                    for (req_idx, request) in collection.requests.iter().enumerate() {
                        // Filter by search query
                        if !self.search_query.is_empty() 
                            && !request.name.to_lowercase().contains(&self.search_query.to_lowercase())
                            && !request.url.to_lowercase().contains(&self.search_query.to_lowercase())
                        {
                            continue;
                        }

                        let is_selected = self.current_collection_idx == Some(col_idx) 
                            && self.current_request_idx == Some(req_idx);

                        ui.horizontal(|ui| {
                            ui.add_space(32.0);
                            
                            // Method badge
                            let method_color = request.method.color();
                            ui.label(egui::RichText::new(request.method.as_str())
                                .color(method_color)
                                .size(10.0)
                                .strong());

                            let req_label = ui.selectable_label(is_selected, &request.name);
                            if req_label.clicked() {
                                self.current_collection_idx = Some(col_idx);
                                self.current_request_idx = Some(req_idx);
                                self.current_request = request.clone();
                                self.response = None;
                            }

                            req_label.context_menu(|ui| {
                                if ui.button("🗑 Delete").clicked() {
                                    action = Some(SidebarAction::DeleteRequest(col_idx, req_idx));
                                    ui.close_menu();
                                }
                            });
                        });
                    }
                }
            }

            // Handle actions
            if let Some(action) = action {
                match action {
                    SidebarAction::AddRequest(col_idx) => {
                        self.add_request_to_collection(col_idx);
                    }
                    SidebarAction::DeleteRequest(col_idx, req_idx) => {
                        self.delete_request(col_idx, req_idx);
                    }
                    SidebarAction::DeleteCollection(col_idx) => {
                        self.delete_collection(col_idx);
                    }
                    SidebarAction::ExportCollection(col_idx) => {
                        if let Some(collection) = self.collections.get(col_idx) {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("JSON", &["json"])
                                .set_file_name(&format!("{}.json", collection.name))
                                .save_file()
                            {
                                let _ = storage::export_collection(collection, &path);
                            }
                        }
                    }
                }
            }
        });
    }

    /// Render main content
    fn render_main_content(&mut self, ui: &mut egui::Ui) {
        ui.add_space(16.0);

        // URL bar
        ui.horizontal(|ui| {
            ui.add_space(16.0);

            // Method selector
            egui::ComboBox::from_id_salt("method_selector")
                .width(100.0)
                .selected_text(egui::RichText::new(self.current_request.method.as_str())
                    .color(self.current_request.method.color())
                    .strong())
                .show_ui(ui, |ui| {
                    for method in HttpMethod::all() {
                        ui.selectable_value(
                            &mut self.current_request.method,
                            *method,
                            egui::RichText::new(method.as_str()).color(method.color()),
                        );
                    }
                });

            // URL input
            let url_edit = egui::TextEdit::singleline(&mut self.current_request.url)
                .hint_text("Enter URL...")
                .font(egui::TextStyle::Monospace)
                .desired_width(ui.available_width() - 150.0);
            
            let url_response = ui.add(url_edit);
            
            // Send on Enter
            if url_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.send_request();
            }

            // Send button
            let send_btn = egui::Button::new(
                if self.is_loading {
                    egui::RichText::new("⏳ Sending...")
                } else {
                    egui::RichText::new("▶ Send").strong()
                }
            );
            
            if ui.add_enabled(!self.is_loading, send_btn).clicked() {
                self.send_request();
            }

            ui.add_space(16.0);
        });

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
                self.render_request_editor(ui);
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
                self.render_response_viewer(ui);
            });
    }

    /// Render request editor tabs
    fn render_request_editor(&mut self, ui: &mut egui::Ui) {
        // Tabs
        ui.horizontal(|ui| {
            if ui.selectable_label(self.request_tab == RequestTab::Params, "Params")
                .clicked() 
            {
                self.request_tab = RequestTab::Params;
            }
            if ui.selectable_label(self.request_tab == RequestTab::Headers, "Headers")
                .clicked() 
            {
                self.request_tab = RequestTab::Headers;
            }
            if ui.selectable_label(self.request_tab == RequestTab::Body, "Body")
                .clicked() 
            {
                self.request_tab = RequestTab::Body;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.current_collection_idx.is_some() && self.current_request_idx.is_some() {
                    if ui.button("💾 Save").clicked() {
                        self.save_current_request();
                    }
                }
            });
        });

        ui.separator();
        ui.add_space(8.0);

        // Tab content
        egui::ScrollArea::vertical().show(ui, |ui| {
            match self.request_tab {
                RequestTab::Params => self.render_params_tab(ui),
                RequestTab::Headers => self.render_headers_tab(ui),
                RequestTab::Body => self.render_body_tab(ui),
            }
        });
    }

    /// Render query parameters tab
    fn render_params_tab(&mut self, ui: &mut egui::Ui) {
        Self::render_key_value_editor_static(ui, &mut self.current_request.query_params);
    }

    /// Render headers tab  
    fn render_headers_tab(&mut self, ui: &mut egui::Ui) {
        Self::render_key_value_editor_static(ui, &mut self.current_request.headers);
    }

    /// Generic key-value editor (static method to avoid borrow issues)
    fn render_key_value_editor_static(ui: &mut egui::Ui, items: &mut Vec<KeyValue>) {
        let mut to_delete: Option<usize> = None;

        // Header row
        ui.horizontal(|ui| {
            ui.add_space(24.0);
            ui.label(egui::RichText::new("Key").strong());
            ui.add_space(150.0);
            ui.label(egui::RichText::new("Value").strong());
        });

        ui.add_space(4.0);

        for (idx, item) in items.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.checkbox(&mut item.enabled, "");

                let key_edit = egui::TextEdit::singleline(&mut item.key)
                    .hint_text("Key")
                    .desired_width(180.0);
                ui.add(key_edit);

                let value_edit = egui::TextEdit::singleline(&mut item.value)
                    .hint_text("Value")
                    .desired_width(ui.available_width() - 40.0);
                ui.add(value_edit);

                if ui.small_button("🗑").clicked() {
                    to_delete = Some(idx);
                }
            });
        }

        // Delete if needed
        if let Some(idx) = to_delete {
            items.remove(idx);
        }

        ui.add_space(8.0);

        if ui.button("➕ Add").clicked() {
            items.push(KeyValue::default());
        }
    }

    /// Render body tab
    fn render_body_tab(&mut self, ui: &mut egui::Ui) {
        // Body type selector
        ui.horizontal(|ui| {
            for body_type in BodyType::all() {
                if ui.selectable_label(
                    self.current_request.body_type == *body_type,
                    body_type.as_str()
                ).clicked() {
                    self.current_request.body_type = body_type.clone();
                }
            }
        });

        ui.add_space(8.0);

        match self.current_request.body_type {
            BodyType::None => {
                ui.label("This request does not have a body.");
            }
            BodyType::Json | BodyType::Text => {
                let text_edit = egui::TextEdit::multiline(&mut self.current_request.body)
                    .code_editor()
                    .desired_width(f32::INFINITY)
                    .desired_rows(10);
                ui.add(text_edit);
            }
            BodyType::FormData | BodyType::UrlEncoded => {
                Self::render_key_value_editor_static(ui, &mut self.current_request.form_data);
            }
            BodyType::Binary => {
                ui.label("Binary body not fully implemented yet.");
            }
        }
    }

    /// Render response viewer
    fn render_response_viewer(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.strong("Response");
            
            if let Some(ref response) = self.response {
                ui.separator();
                
                // Status badge
                let status_text = format!("{} {}", response.status, response.status_text);
                ui.label(egui::RichText::new(status_text)
                    .color(response.status_color())
                    .strong());

                ui.separator();
                ui.label(format!("{}ms", response.time_ms));
                
                ui.separator();
                ui.label(format_size(response.size));
            }
        });

        ui.separator();

        if let Some(ref response) = self.response {
            if let Some(ref error) = response.error {
                ui.colored_label(egui::Color32::RED, error);
            } else {
                // Response tabs
                ui.horizontal(|ui| {
                    if ui.selectable_label(self.response_tab == ResponseTab::Body, "Body")
                        .clicked() 
                    {
                        self.response_tab = ResponseTab::Body;
                    }
                    if ui.selectable_label(self.response_tab == ResponseTab::Headers, "Headers")
                        .clicked() 
                    {
                        self.response_tab = ResponseTab::Headers;
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.checkbox(&mut self.response_format_json, "Format JSON").changed() {
                            // Trigger re-render
                        }
                        
                        if ui.button("📋 Copy").clicked() {
                            ui.output_mut(|o| o.copied_text = response.body.clone());
                        }
                    });
                });

                ui.add_space(4.0);

                match self.response_tab {
                    ResponseTab::Body => {
                        egui::ScrollArea::both().show(ui, |ui| {
                            let body = if self.response_format_json {
                                format_json(&response.body)
                            } else {
                                response.body.clone()
                            };
                            
                            ui.add(
                                egui::TextEdit::multiline(&mut body.as_str())
                                    .code_editor()
                                    .desired_width(f32::INFINITY)
                            );
                        });
                    }
                    ResponseTab::Headers => {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            egui::Grid::new("response_headers_grid")
                                .striped(true)
                                .show(ui, |ui| {
                                    for (key, value) in &response.headers {
                                        ui.label(egui::RichText::new(key).strong());
                                        ui.label(value);
                                        ui.end_row();
                                    }
                                });
                        });
                    }
                }
            }
        } else if self.is_loading {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
        } else {
            ui.centered_and_justified(|ui| {
                ui.label("Send a request to see the response");
            });
        }
    }

    /// Render modals
    fn render_modals(&mut self, ctx: &egui::Context) {
        // New collection modal
        if self.show_new_collection_modal {
            egui::Window::new("New Collection")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Name:");
                        ui.text_edit_singleline(&mut self.new_item_name);
                    });
                    
                    ui.add_space(8.0);
                    
                    ui.horizontal(|ui| {
                        if ui.button("Create").clicked() && !self.new_item_name.is_empty() {
                            self.create_collection(self.new_item_name.clone());
                            self.show_new_collection_modal = false;
                            self.new_item_name.clear();
                        }
                        if ui.button("Cancel").clicked() {
                            self.show_new_collection_modal = false;
                            self.new_item_name.clear();
                        }
                    });
                });
        }

        // Environment editor modal
        if self.show_environment_editor {
            egui::Window::new("Manage Environments")
                .collapsible(false)
                .resizable(true)
                .default_size([500.0, 400.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("➕ New Environment").clicked() {
                            self.create_environment(format!("Environment {}", self.environments.len() + 1));
                        }
                    });

                    ui.separator();

                    let mut env_to_delete: Option<usize> = None;
                    
                    for (env_idx, env) in self.environments.iter_mut().enumerate() {
                        egui::CollapsingHeader::new(&env.name)
                            .default_open(false)
                            .show(ui, |ui| {
                                // Name edit
                                ui.horizontal(|ui| {
                                    ui.label("Name:");
                                    if ui.text_edit_singleline(&mut env.name).changed() {
                                        let _ = storage::save_environment(env);
                                    }
                                });

                                ui.add_space(8.0);
                                ui.label("Variables:");

                                // Variables editor
                                let mut var_to_delete: Option<usize> = None;
                                for (var_idx, var) in env.variables.iter_mut().enumerate() {
                                    ui.horizontal(|ui| {
                                        ui.checkbox(&mut var.enabled, "");
                                        ui.text_edit_singleline(&mut var.key);
                                        ui.text_edit_singleline(&mut var.value);
                                        if ui.small_button("🗑").clicked() {
                                            var_to_delete = Some(var_idx);
                                        }
                                    });
                                }

                                if let Some(idx) = var_to_delete {
                                    env.variables.remove(idx);
                                    let _ = storage::save_environment(env);
                                }

                                if ui.button("➕ Add Variable").clicked() {
                                    env.variables.push(KeyValue::default());
                                    let _ = storage::save_environment(env);
                                }

                                ui.add_space(8.0);

                                if ui.button("🗑 Delete Environment").clicked() {
                                    env_to_delete = Some(env_idx);
                                }
                            });
                    }

                    if let Some(idx) = env_to_delete {
                        let env = self.environments.remove(idx);
                        let _ = storage::delete_environment(&env);
                        if self.active_environment_id.as_ref() == Some(&env.id) {
                            self.active_environment_id = None;
                        }
                    }

                    ui.add_space(8.0);
                    
                    if ui.button("Close").clicked() {
                        self.show_environment_editor = false;
                    }
                });
        }
    }
}

/// Sidebar actions
enum SidebarAction {
    AddRequest(usize),
    DeleteRequest(usize, usize),
    DeleteCollection(usize),
    ExportCollection(usize),
}

/// Format file size
fn format_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

