//! Desktop adapter over the shared workspace and execution services.
use crate::{
    http::CodeGenLanguage,
    models::*,
    storage::{self, AppState},
    ui::*,
};
use eframe::egui;
use rustapi::{
    http::{Cancellation, HttpService},
    runner::{RunOptions, RunReport},
    variables::Variables,
};
use std::{
    collections::BTreeMap,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
    time::{Duration, Instant},
};

// Native dialogs remain at the desktop boundary so command behavior is testable.
trait CollectionDialogs {
    fn import(&self) -> Option<std::path::PathBuf>;
    fn export(&self, name: &str) -> Option<std::path::PathBuf>;
}
struct NativeCollectionDialogs;
impl CollectionDialogs for NativeCollectionDialogs {
    fn import(&self) -> Option<std::path::PathBuf> {
        rfd::FileDialog::new()
            .set_title("Import collection")
            .add_filter("Collections", &["json", "yaml", "yml"])
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

pub enum AsyncMessage {
    Response {
        epoch: u64,
        tab_id: String,
        execution: String,
        request: Box<ApiRequest>,
        response: Box<ApiResponse>,
        variables: Variables,
        before: Box<Variables>,
        environment_id: Option<String>,
        collection_id: Option<String>,
    },
    RunProgress {
        epoch: u64,
        entry: rustapi::runner::RunEntry,
    },
    Run {
        epoch: u64,
        result: Result<RunReport, String>,
    },
    Authorized {
        epoch: u64,
        tab_id: String,
        binding: String,
        result: Result<String, String>,
    },
}
#[derive(Clone)]
enum Action {
    Open(String, Box<ApiRequest>),
    TryExample(Box<RequestExample>, Option<(String, String)>),
    Add(String, Option<String>),
    Delete(String, String),
    Duplicate(String, String),
    NewFolder(String, Option<String>),
    Edit(String, Option<String>),
    Rename(String, String, bool),
    DeleteFolder(String, String),
    DeleteCollection(String),
    Export(String),
    ExportCredentials(String),
    ExportPortable(String),
    ExportV3(String),
    Move(String, String),
    MoveFolder(String, String),
    Reorder(String, Option<String>, String, bool),
}
pub struct RustApiApp {
    dialogs: Box<dyn CollectionDialogs>,
    pub collections: Vec<Collection>,
    pub environments: Vec<Environment>,
    pub active_environment_id: Option<String>,
    pub tab_manager: TabManager,
    service: Arc<HttpService>,
    sender: Sender<AsyncMessage>,
    receiver: Receiver<AsyncMessage>,
    epoch: u64,
    variables: BTreeMap<String, String>,
    error: Option<String>,
    notice: String,
    request_tab: RequestTab,
    response_tab: ResponseTab,
    format_json: bool,
    search: String,
    response_search: String,
    documents: DocumentEditor,
    api_tools: ApiTools,
    protocol_console: ProtocolConsole,
    flow_editor: FlowEditor,
    platform_tools: PlatformTools,
    show_history: bool,
    show_environments: bool,
    show_import: bool,
    import_text: String,
    pending_import: Option<rustapi::import::ImportResult>,
    show_new_collection: bool,
    new_name: String,
    save_as: Option<String>,
    save_collection: Option<String>,
    save_folder: Option<String>,
    close_pending: Option<String>,
    exit_pending: bool,
    allow_exit: bool,
    edit: Option<(String, Option<String>)>,
    edit_draft: Option<Collection>,
    rename: Option<(String, String, bool, String)>,
    move_request: Option<(String, String)>,
    moving_folder: bool,
    move_collection: Option<String>,
    move_folder: Option<String>,
    show_runner: bool,
    run_collection: Option<String>,
    run_folder: Option<String>,
    run_options: RunOptions,
    data_path: String,
    run_cancel: Option<Cancellation>,
    report: Option<RunReport>,
    show_vault: bool,
    vault_password: String,
    secrets: BTreeMap<String, String>,
    session_secrets: BTreeMap<String, String>,
    vault_unlocked: bool,
    secret_name: String,
    secret_value: String,
    show_debug: bool,
    debug_filter: String,
    show_inspector: bool,
    reveal_inspector: bool,
    show_cookies: bool,
    cookie_bridge: Option<rustapi::http::CookieBridge>,
    cookie_url: String,
    cookie_value: String,
    show_code: bool,
    code_language: CodeGenLanguage,
    include_secrets: bool,
    last_draft: Instant,
    pub global_variables: storage::GlobalVariables,
    pub environment_overrides: storage::LocalEnvironmentOverrides,
    show_globals: bool,
    global_key: String,
    global_value: String,
    show_overrides: bool,
    override_key: String,
    override_value: String,
}
impl Default for RustApiApp {
    fn default() -> Self {
        let (sender, receiver) = channel();
        let mut errors = vec![];
        if let Err(e) = storage::init_storage() {
            errors.push(e.to_string());
        }
        let collections = storage::load_all_collections();
        let environments = storage::load_all_environments();
        for directory in [
            storage::get_collections_dir(),
            storage::get_environments_dir(),
        ] {
            if let Ok(entries) = std::fs::read_dir(&directory) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.extension().is_some_and(|e| e == "json") {
                        let result = if directory == storage::get_collections_dir() {
                            storage::load_collection(&p).map(|_| ())
                        } else {
                            storage::load_environment(&p).map(|_| ())
                        };
                        if let Err(e) = result {
                            errors.push(e.to_string());
                        }
                    }
                }
            }
        }
        let state = storage::load_app_state().unwrap_or_else(|e| {
            errors.push(e.to_string());
            AppState::default()
        });
        let tab_manager = storage::load_drafts::<TabManager>().unwrap_or_else(|e| {
            errors.push(format!("Draft recovery failed: {e}"));
            TabManager::new()
        });
        Self {
            dialogs: Box::new(NativeCollectionDialogs),
            collections,
            environments,
            active_environment_id: state.active_environment_id,
            tab_manager,
            service: Arc::new(HttpService::default()),
            sender,
            receiver,
            epoch: 0,
            variables: BTreeMap::new(),
            error: (!errors.is_empty()).then(|| errors.join("\n")),
            notice: String::new(),
            request_tab: Default::default(),
            response_tab: Default::default(),
            format_json: true,
            search: String::new(),
            response_search: String::new(),
            documents: DocumentEditor::recover(),
            api_tools: ApiTools::recover(),
            protocol_console: ProtocolConsole::default(),
            flow_editor: FlowEditor::default(),
            platform_tools: PlatformTools::default(),
            show_history: false,
            show_environments: false,
            show_import: false,
            import_text: String::new(),
            pending_import: None,
            show_new_collection: false,
            new_name: String::new(),
            save_as: None,
            save_collection: None,
            save_folder: None,
            close_pending: None,
            exit_pending: false,
            allow_exit: false,
            edit: None,
            edit_draft: None,
            rename: None,
            move_request: None,
            moving_folder: false,
            move_collection: None,
            move_folder: None,
            show_runner: false,
            run_collection: None,
            run_folder: None,
            run_options: Default::default(),
            data_path: String::new(),
            run_cancel: None,
            report: None,
            show_vault: false,
            vault_password: String::new(),
            secrets: BTreeMap::new(),
            session_secrets: BTreeMap::new(),
            vault_unlocked: false,
            secret_name: String::new(),
            secret_value: String::new(),
            show_debug: false,
            debug_filter: String::new(),
            show_inspector: false,
            reveal_inspector: false,
            show_cookies: false,
            cookie_bridge: None,
            cookie_url: String::new(),
            cookie_value: String::new(),
            show_code: false,
            code_language: Default::default(),
            include_secrets: false,
            last_draft: Instant::now(),
            global_variables: storage::GlobalVariables::load(&storage::get_app_data_dir())
                .unwrap_or_default(),
            environment_overrides: storage::LocalEnvironmentOverrides::load(
                &storage::get_app_data_dir(),
            )
            .unwrap_or_default(),
            show_globals: false,
            global_key: String::new(),
            global_value: String::new(),
            show_overrides: false,
            override_key: String::new(),
            override_value: String::new(),
        }
    }
}
impl RustApiApp {
    fn execution_secrets(&self) -> BTreeMap<String, String> {
        let mut values = self.secrets.clone();
        values.extend(self.session_secrets.clone());
        values
    }
    pub fn new(_: &eframe::CreationContext<'_>) -> Self {
        Self::default()
    }
    pub fn variables_for_request(&self, request: &ApiRequest) -> Variables {
        let mut vars = Variables::for_request(request, self.environment());
        vars.global = self.global_variables.variables.clone();
        if let Some(env) = self.environment() {
            vars.local_overrides = self.environment_overrides.get_overrides(&env.id, &env.name);
        }
        vars.local = self.variables.clone();
        vars.secrets = self.execution_secrets();
        vars
    }
    fn result<T>(&mut self, result: anyhow::Result<T>) -> Option<T> {
        match result {
            Ok(v) => Some(v),
            Err(e) => {
                self.error = Some(format!("{e:#}"));
                None
            }
        }
    }
    fn persist_collection(c: &mut Collection) -> anyhow::Result<()> {
        c.updated_at = chrono::Utc::now();
        let path = storage::save_collection(c, None)?;
        c.file_path = Some(path.to_string_lossy().into());
        Ok(())
    }
    fn commit_collection(&mut self, mut c: Collection) -> bool {
        if self.result(Self::persist_collection(&mut c)).is_some() {
            if let Some(current) = self.collections.iter_mut().find(|v| v.id == c.id) {
                *current = c;
            } else {
                self.collections.push(c);
            }
            true
        } else {
            false
        }
    }
    fn environment(&self) -> Option<&Environment> {
        self.active_environment_id
            .as_ref()
            .and_then(|id| self.environments.iter().find(|e| &e.id == id))
    }
    fn contextual_request(&self, tab: &OpenRequestTab) -> ApiRequest {
        if let Some(mut c) = tab
            .collection_id
            .as_ref()
            .and_then(|id| self.collections.iter().find(|c| &c.id == id))
            .cloned()
        {
            if let Some(r) = c.request_mut(&tab.request.id) {
                *r = tab.request.clone();
                if let Some(r) = c
                    .ordered_requests(None)
                    .into_iter()
                    .find(|r| r.id == tab.request.id)
                {
                    return r;
                }
            }
        }
        tab.request.clone()
    }
    fn send_request(&mut self) {
        let Some(tab) = self.tab_manager.active_tab() else {
            return;
        };
        if tab.pending.is_some() || tab.request.url.trim().is_empty() {
            return;
        }
        let tab_id = tab.id.clone();
        let collection_id = tab.collection_id.clone();
        let environment_id = self.active_environment_id.clone();
        let request = self.contextual_request(tab);
        let mut variables = self.variables_for_request(&request);
        let before = Box::new(Variables {
            global: variables.global.clone(),
            collection: variables.collection.clone(),
            environment: variables.environment.clone(),
            local_overrides: variables.local_overrides.clone(),
            local: variables.local.clone(),
            ..Default::default()
        });
        let execution = uuid::Uuid::new_v4().to_string();
        let cancel = Cancellation::default();
        self.tab_manager.active_tab_mut().unwrap().pending =
            Some((execution.clone(), cancel.clone()));
        let service = self.service.clone();
        let sender = self.sender.clone();
        let epoch = self.epoch;
        std::thread::spawn(move || {
            let response = rustapi::runner::execute(&service, &request, &mut variables, &cancel);
            let _ = sender.send(AsyncMessage::Response {
                epoch,
                tab_id,
                execution,
                request: Box::new(request),
                response: Box::new(response),
                variables,
                before,
                environment_id,
                collection_id,
            });
        });
    }
    fn process_messages(&mut self) {
        while let Ok(message) = self.receiver.try_recv() {
            match message {
                AsyncMessage::Response {
                    epoch,
                    tab_id,
                    execution,
                    request,
                    response,
                    variables,
                    before,
                    environment_id,
                    collection_id,
                } if epoch == self.epoch => {
                    // The immutable sent snapshot is recorded even if its originating tab was closed.
                    let mut history = storage::load_history();
                    history.add(*request, Some((*response).clone()));
                    let result = storage::save_history(&history);
                    self.result(result);
                    if let Some(tab) = self.tab_manager.tabs.iter_mut().find(|t| {
                        t.id == tab_id && t.pending.as_ref().is_some_and(|(id, _)| id == &execution)
                    }) {
                        tab.response = Some(*response);
                        tab.pending = None;
                        merge_changes(&before.local, &variables.local, &mut self.variables);
                        if let Some(env) = environment_id
                            .as_ref()
                            .and_then(|id| self.environments.iter_mut().find(|e| &e.id == id))
                        {
                            env.variables.retain(|v| {
                                !before.environment.contains_key(&v.key)
                                    || variables.environment.contains_key(&v.key)
                            });
                            for (k, v) in &variables.environment {
                                if before.environment.get(k) != Some(v) {
                                    if let Some(field) =
                                        env.variables.iter_mut().find(|f| &f.key == k)
                                    {
                                        field.value = v.clone();
                                        field.enabled = true;
                                    } else {
                                        env.variables.push(KeyValue::new(k, v));
                                    }
                                }
                            }
                        }
                        if let Some(c) = collection_id
                            .as_ref()
                            .and_then(|id| self.collections.iter_mut().find(|c| &c.id == id))
                        {
                            let mut current = c.variables.clone().into_iter().collect();
                            merge_changes(&before.collection, &variables.collection, &mut current);
                            c.variables = current.into_iter().collect();
                        }
                        merge_changes(
                            &before.global,
                            &variables.global,
                            &mut self.global_variables.variables,
                        );
                        let _ = self.global_variables.save(&storage::get_app_data_dir());
                    }
                }
                AsyncMessage::RunProgress { epoch, entry } if epoch == self.epoch => {
                    self.report
                        .get_or_insert_with(Default::default)
                        .entries
                        .push(entry);
                }
                AsyncMessage::Run { epoch, result } if epoch == self.epoch => {
                    self.run_cancel = None;
                    match result {
                        Ok(report) => self.report = Some(report),
                        Err(e) => self.error = Some(e),
                    }
                }
                AsyncMessage::Authorized {
                    epoch,
                    tab_id,
                    binding,
                    result,
                } if epoch == self.epoch => match result {
                    Ok(code) => {
                        if let Some(index) = self
                            .tab_manager
                            .tabs
                            .iter()
                            .position(|tab| tab.id == tab_id)
                        {
                            let request = self.contextual_request(&self.tab_manager.tabs[index]);
                            let vars = self.variables_for_request(&request);
                            let current =
                                vars.resolve_oauth(&request.auth.oauth).and_then(|config| {
                                    rustapi::http::oauth::authorization_binding(&config)
                                });
                            if current.ok().as_deref() != Some(&binding) {
                                self.error =
                                    Some("OAuth configuration changed; authorize again".into());
                                continue;
                            }
                            let tab = &mut self.tab_manager.tabs[index];
                            tab.request.auth.oauth.code = code;
                            tab.is_modified = true;
                            self.notice =
                                "OAuth authorized. Send the request to exchange the code.".into();
                        }
                    }
                    Err(e) => self.error = Some(e),
                },
                _ => {}
            }
        }
    }
    fn save_tab(&mut self, id: &str, target: Option<(String, Option<String>)>) -> bool {
        let Some(tab) = self.tab_manager.tabs.iter().find(|t| t.id == id).cloned() else {
            return false;
        };
        let Some((collection_id, folder)) =
            target.or_else(|| tab.collection_id.clone().map(|c| (c, None)))
        else {
            self.save_as = Some(id.into());
            self.save_collection = self.collections.first().map(|c| c.id.clone());
            self.save_folder = None;
            return false;
        };
        let Some(mut c) = self
            .collections
            .iter()
            .find(|c| c.id == collection_id)
            .cloned()
        else {
            self.error = Some("Collection no longer exists; use Save As".into());
            return false;
        };
        let mut r = tab.request.clone();
        if tab.collection_id.as_ref() != Some(&collection_id) && c.request(&r.id).is_some() {
            r.id = uuid::Uuid::new_v4().to_string();
        }
        let saved_request_id = r.id.clone();
        r.updated_at = chrono::Utc::now();
        if let Some(existing) = c.request_mut(&r.id) {
            *existing = r;
        } else if let Some(folder) = folder {
            let Some(f) = c.folder_mut(&folder) else {
                self.error = Some("Folder no longer exists".into());
                return false;
            };
            f.requests.push(r);
        } else {
            c.requests.push(r);
        }
        if !self.commit_collection(c) {
            return false;
        }
        if let Some(t) = self.tab_manager.tabs.iter_mut().find(|t| t.id == id) {
            t.collection_id = Some(collection_id);
            t.request.id = saved_request_id;
            t.is_modified = false;
        }
        self.persist_drafts();
        true
    }
    fn export_collection(&mut self, id: &str, include_secrets: bool) {
        if let Some(c) = self.collections.iter().find(|c| c.id == id).cloned() {
            if let Some(path) = self.dialogs.export(&c.name) {
                let c = if include_secrets {
                    c
                } else {
                    rustapi::secrets::redact_collection(&c)
                };
                let result = storage::export_collection(&c, &path);
                self.result(result);
            }
        }
    }
    fn persist_drafts(&mut self) {
        let result = storage::save_drafts(&self.tab_manager);
        self.result(result);
        self.last_draft = Instant::now();
    }
    fn request_close(&mut self, id: String) {
        if self
            .tab_manager
            .tabs
            .iter()
            .any(|t| t.id == id && t.is_modified)
        {
            self.close_pending = Some(id);
        } else {
            self.close_tab(&id);
        }
    }
    fn close_tab(&mut self, id: &str) {
        if let Some(i) = self.tab_manager.tabs.iter().position(|t| t.id == id) {
            self.tab_manager.close_tab(i);
            self.persist_drafts();
        }
    }
    fn action(&mut self, action: Action) {
        match action {
            Action::Open(c, r) => self.tab_manager.open_tab(*r, Some(c)),
            Action::TryExample(example, source) => {
                let response = example.response();
                let mut request = *example.request;
                if let Some((cid, rid)) = source {
                    if let Some(mut collection) =
                        self.collections.iter().find(|c| c.id == cid).cloned()
                    {
                        request.id = rid.clone();
                        if let Some(parent) = collection.request_mut(&rid) {
                            *parent = request.clone();
                        }
                        if let Some(context) = collection
                            .ordered_requests(None)
                            .into_iter()
                            .find(|r| r.id == rid)
                        {
                            request = context;
                        }
                    }
                }
                request.id = uuid::Uuid::new_v4().to_string();
                request.name = example.name;
                self.tab_manager.open_tab(request, None);
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    tab.is_modified = true;
                    tab.response = response.ok();
                }
            }
            Action::Add(id, folder) => {
                if let Some(mut c) = self.collections.iter().find(|c| c.id == id).cloned() {
                    let mut r = ApiRequest::new("New Request");
                    r.auth.auth_type = AuthType::Inherit;
                    if let Some(f) = folder {
                        if let Some(f) = c.folder_mut(&f) {
                            f.requests.push(r.clone());
                        }
                    } else {
                        c.requests.push(r.clone());
                    }
                    if self.commit_collection(c) {
                        self.tab_manager.open_tab(r, Some(id));
                    }
                }
            }
            Action::Duplicate(id, rid) => {
                if let Some(mut c) = self.collections.iter().find(|c| c.id == id).cloned() {
                    if c.duplicate_request(&rid).is_some() {
                        self.commit_collection(c);
                    }
                }
            }
            Action::Delete(id, rid) => {
                if let Some(mut c) = self.collections.iter().find(|c| c.id == id).cloned() {
                    c.remove_request(&rid);
                    if self.commit_collection(c) {
                        for t in &mut self.tab_manager.tabs {
                            if t.collection_id.as_ref() == Some(&id) && t.request.id == rid {
                                t.collection_id = None;
                                t.is_modified = true;
                            }
                        }
                        self.persist_drafts();
                    }
                }
            }
            Action::NewFolder(id, parent) => {
                if let Some(mut c) = self.collections.iter().find(|c| c.id == id).cloned() {
                    let f = Folder::new("New Folder");
                    if let Some(parent) = parent {
                        if let Some(parent) = c.folder_mut(&parent) {
                            parent.folders.push(f);
                        }
                    } else {
                        c.folders.push(f);
                    }
                    self.commit_collection(c);
                }
            }
            Action::Edit(id, folder) => {
                self.edit_draft = self.collections.iter().find(|c| c.id == id).cloned();
                self.edit = Some((id, folder));
            }
            Action::Rename(id, target, folder) => {
                if let Some(c) = self.collections.iter().find(|c| c.id == id) {
                    let name = if folder {
                        let mut c = c.clone();
                        c.folder_mut(&target).map(|f| f.name.clone())
                    } else {
                        c.request(&target).map(|r| r.name.clone())
                    };
                    if let Some(name) = name {
                        self.rename = Some((id, target, folder, name));
                    }
                }
            }
            Action::DeleteFolder(id, fid) => {
                if let Some(mut c) = self.collections.iter().find(|c| c.id == id).cloned() {
                    c.remove_folder(&fid);
                    if self.commit_collection(c) {
                        self.detach_missing_tabs();
                    }
                }
            }
            Action::DeleteCollection(id) => {
                if let Some(i) = self.collections.iter().position(|c| c.id == id) {
                    let result = storage::delete_collection(&self.collections[i]);
                    if self.result(result).is_some() {
                        self.collections.remove(i);
                        self.detach_missing_tabs();
                    }
                }
            }
            Action::Export(id) => self.export_collection(&id, false),
            Action::ExportCredentials(id) => self.export_collection(&id, true),
            Action::ExportV3(id) => {
                if let Some(c) = self.collections.iter().find(|c| c.id == id).cloned() {
                    if let Some(parent) = rfd::FileDialog::new()
                        .set_title("Choose parent folder for new v3 export")
                        .pick_folder()
                    {
                        let target = parent.join(format!("collection-{}", uuid::Uuid::new_v4()));
                        let result =
                            rustapi::v3::export_directory(&c, &target, false).map(|notes| {
                                format!("Exported {}\n{}", target.display(), notes.join("\n"))
                            });
                        if let Some(notice) = self.result(result) {
                            self.notice = notice;
                        }
                    }
                }
            }
            Action::ExportPortable(id) => {
                if let Some(c) = self.collections.iter().find(|c| c.id == id).cloned() {
                    if let Some(path) = self.dialogs.export(&c.name) {
                        let result = rustapi::export::collection(&c, false).and_then(|exported| {
                            rustapi::workspace::atomic_write(
                                &path,
                                &serde_json::to_vec_pretty(&exported.document)?,
                            )?;
                            Ok(exported.warnings.join("\n"))
                        });
                        if let Some(note) = self.result(result) {
                            self.notice = note;
                        }
                    }
                }
            }
            Action::MoveFolder(c, r) => {
                self.moving_folder = true;
                self.move_request = Some((c, r));
                self.move_collection = self.collections.first().map(|c| c.id.clone());
                self.move_folder = None;
            }
            Action::Move(c, r) => {
                self.moving_folder = false;
                self.move_request = Some((c, r));
                self.move_collection = self.collections.first().map(|c| c.id.clone());
                self.move_folder = None;
            }
            Action::Reorder(id, parent, target, up) => {
                if let Some(mut c) = self.collections.iter().find(|c| c.id == id).cloned() {
                    let (requests, folders, order) = if let Some(parent) = parent {
                        let Some(f) = c.folder_mut(&parent) else {
                            return;
                        };
                        (&f.requests, &f.folders, &mut f.order)
                    } else {
                        (&c.requests, &c.folders, &mut c.order)
                    };
                    *order = child_order(requests, folders, order);
                    if let Some(i) = order.iter().position(|id| id == &target) {
                        let j = if up {
                            i.saturating_sub(1)
                        } else {
                            (i + 1).min(order.len() - 1)
                        };
                        order.swap(i, j);
                    }
                    self.commit_collection(c);
                }
            }
        }
    }
    fn detach_missing_tabs(&mut self) {
        for t in &mut self.tab_manager.tabs {
            if t.collection_id.as_ref().is_some_and(|id| {
                !self
                    .collections
                    .iter()
                    .any(|c| &c.id == id && c.request(&t.request.id).is_some())
            }) {
                t.collection_id = None;
                t.is_modified = true;
            }
        }
        self.persist_drafts();
    }
    fn import(&mut self, result: anyhow::Result<rustapi::import::ImportResult>) {
        self.pending_import = None;
        if let Some(import) = self.result(result) {
            if import.requires_confirmation() {
                self.pending_import = Some(import);
            } else {
                self.finish_import(import);
            }
        }
    }
    fn finish_import(&mut self, import: rustapi::import::ImportResult) {
        if let Some(c) = import.collection {
            self.commit_collection(c);
        }
        if let Some(e) = import.environment {
            if self.result(storage::save_environment(&e)).is_some() {
                self.environments.push(e);
            }
        }
        self.notice = import.warnings.join("\n");
    }
    fn import_review_window(&mut self, ctx: &egui::Context) {
        let Some(import) = &self.pending_import else {
            return;
        };
        let mut open = true;
        let mut accept = false;
        let mut cancel = false;
        egui::Window::new("Review import omissions")
            .open(&mut open)
            .collapsible(false)
            .default_width(640.)
            .show(ctx, |ui| {
                ui.label("Some source data cannot be retained. Nothing has been imported yet.");
                egui::ScrollArea::vertical()
                    .max_height(320.)
                    .show(ui, |ui| {
                        for diagnostic in &import.diagnostics {
                            ui.monospace(&diagnostic.path);
                            ui.label(&diagnostic.message);
                            ui.separator();
                        }
                    });
                accept = ui.button("Import with omissions").clicked();
                cancel = ui.button("Cancel import").clicked();
            });
        if !open || cancel {
            self.pending_import = None;
        } else if accept {
            if let Some(import) = self.pending_import.take() {
                self.finish_import(import);
            }
        }
    }
    fn open_workspace(&mut self, path: &std::path::Path) {
        if self.tab_manager.tabs.iter().any(|t| t.is_modified) {
            self.persist_drafts();
        }
        if let Some(path) = self.result(rustapi::workspace::open_workspace(path)) {
            for t in &self.tab_manager.tabs {
                if let Some((_, c)) = &t.pending {
                    c.cancel();
                }
            }
            if let Some(c) = &self.run_cancel {
                c.cancel();
            }
            let epoch = self.epoch + 1;
            std::env::set_var("RUSTAPI_DATA_DIR", path);
            let next = Self {
                epoch,
                ..Default::default()
            };
            *self = next;
        }
    }
    fn start_run(&mut self) {
        if self.run_cancel.is_some() {
            return;
        }
        let Some(c) = self
            .run_collection
            .as_ref()
            .and_then(|id| self.collections.iter().find(|c| &c.id == id))
            .cloned()
        else {
            self.error = Some("Select a collection".into());
            return;
        };
        let mut options = self.run_options.clone();
        options.folder = self.run_folder.clone();
        if !self.data_path.is_empty() {
            let result = rustapi::runner::load_data(std::path::Path::new(&self.data_path));
            let Some(data) = self.result(result) else {
                return;
            };
            options.data = data;
        }
        self.report = Some(RunReport::default());
        let env = self.environment().cloned();
        let service = self.service.clone();
        let secrets = self.execution_secrets();
        let cancel = Cancellation::default();
        self.run_cancel = Some(cancel.clone());
        let sender = self.sender.clone();
        let epoch = self.epoch;
        std::thread::spawn(move || {
            let result = rustapi::runner::run_with_progress(
                &service,
                &c,
                env.as_ref(),
                &options,
                secrets,
                &cancel,
                |entry| {
                    let _ = sender.send(AsyncMessage::RunProgress {
                        epoch,
                        entry: entry.clone(),
                    });
                },
            )
            .map_err(|e| e.to_string());
            let _ = sender.send(AsyncMessage::Run { epoch, result });
        });
    }
    fn authorize(&mut self, ctx: &egui::Context) {
        let Some(tab) = self.tab_manager.active_tab() else {
            return;
        };
        if !tab.request.auth.oauth.authorization_requested {
            return;
        }
        let id = tab.id.clone();
        let request = self.contextual_request(tab);
        let mut vars = Variables::for_request(&request, self.environment());
        vars.local = self.variables.clone();
        vars.secrets = self.execution_secrets();
        self.tab_manager
            .active_tab_mut()
            .unwrap()
            .request
            .auth
            .oauth
            .authorization_requested = false;
        let result = (|| -> anyhow::Result<_> {
            let mut config = vars.resolve_oauth(&request.auth.oauth)?;
            let had_redirect = !config.redirect_uri.is_empty();
            let authorization = rustapi::http::oauth::begin_authorization(&mut config)?;
            let binding = rustapi::http::oauth::authorization_binding(&config)?;
            let tab = self.tab_manager.active_tab_mut().unwrap();
            tab.request.auth.oauth.verifier = config.verifier;
            tab.request.auth.oauth.code.clear();
            if !had_redirect {
                tab.request.auth.oauth.redirect_uri = config.redirect_uri;
            }
            tab.is_modified = true;
            Ok((authorization, binding))
        })();
        match result {
            Ok((authorization, binding)) => {
                ctx.open_url(egui::OpenUrl::new_tab(authorization.url.clone()));
                let sender = self.sender.clone();
                let epoch = self.epoch;
                std::thread::spawn(move || {
                    let result = authorization
                        .wait(&Cancellation::default())
                        .map_err(|error| error.to_string());
                    let _ = sender.send(AsyncMessage::Authorized {
                        epoch,
                        tab_id: id,
                        binding,
                        result,
                    });
                });
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }
}
impl eframe::App for RustApiApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.render(ctx);
    }
    fn on_exit(&mut self, _: Option<&eframe::glow::Context>) {
        self.persist_drafts();
        let state = AppState {
            active_environment_id: self.active_environment_id.clone(),
            recent_files: vec![],
            window_size: None,
        };
        let result = storage::save_app_state(&state);
        if let Err(e) = result {
            eprintln!("Could not save app state: {e}");
        }
        let result = storage::save_history(&storage::load_history());
        if let Err(e) = result {
            eprintln!("Could not save history: {e}");
        }
    }
}
impl RustApiApp {
    fn choose_import(&mut self) {
        if let Some(path) = self.dialogs.import() {
            let result = rustapi::import::path(&path);
            self.import(result);
        }
    }
    pub(crate) fn render(&mut self, ctx: &egui::Context) {
        let open = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::O);
        let export = egui::KeyboardShortcut::new(
            egui::Modifiers {
                command: true,
                shift: true,
                ..Default::default()
            },
            egui::Key::E,
        );
        if ctx.input_mut(|i| i.consume_shortcut(&open)) {
            self.choose_import();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&export)) {
            if let Some(id) = self
                .tab_manager
                .active_tab()
                .and_then(|t| t.collection_id.clone())
                .or_else(|| self.collections.first().map(|c| c.id.clone()))
            {
                self.export_collection(&id, false);
            }
        }

        self.process_messages();
        apply_dark_theme(ctx);
        if ctx.input(|i| i.viewport().close_requested())
            && !self.allow_exit
            && self.tab_manager.tabs.iter().any(|t| t.is_modified)
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.exit_pending = true;
        }
        if self.tab_manager.tabs.iter().any(|t| t.pending.is_some()) || self.run_cancel.is_some() {
            ctx.request_repaint_after(Duration::from_millis(20));
        }
        ctx.request_repaint_after(Duration::from_secs(1));
        if self.last_draft.elapsed() > Duration::from_secs(2) {
            self.persist_drafts();
        }
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("RustAPI");
                if ui.button("Open / create workspace").clicked() {
                    if let Some(p) = rfd::FileDialog::new().pick_folder() {
                        self.open_workspace(&p);
                    }
                }
                egui::ComboBox::from_id_salt("env")
                    .selected_text(
                        self.environment()
                            .map(|e| e.name.as_str())
                            .unwrap_or("No environment"),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.active_environment_id,
                            None,
                            "No environment",
                        );
                        for e in &self.environments {
                            ui.selectable_value(
                                &mut self.active_environment_id,
                                Some(e.id.clone()),
                                &e.name,
                            );
                        }
                    });
                if ui.button("Environments").clicked() {
                    self.show_environments = true;
                }
                if ui.button("Import").clicked() {
                    self.show_import = true;
                }
                if ui.button("Runner").clicked() {
                    self.show_runner = true;
                    if self.run_collection.is_none() {
                        self.run_collection = self.collections.first().map(|c| c.id.clone());
                    }
                }
                if ui.button("Vault").clicked() {
                    self.show_vault = true;
                }
                if ui.button("Debug console").clicked() {
                    self.show_debug = true;
                }
                if ui.button("Inspect variables").clicked() {
                    self.show_inspector = true;
                    self.reveal_inspector = false;
                }
                if ui.button("Globals").clicked() {
                    self.show_globals = true;
                }
                if ui.button("Local overrides").clicked() {
                    self.show_overrides = true;
                }
                if ui.button("Cookies").clicked() {
                    self.show_cookies = true;
                }
                if ui.button("Documents").clicked() {
                    self.documents.open = true;
                }
                if ui.button("API tools").clicked() {
                    self.api_tools.open = true;
                }
                if ui.button("Protocols").clicked() {
                    self.protocol_console.open = true;
                }
                if ui.button("Visual flows").clicked() {
                    self.flow_editor.open = true;
                }
                if ui.button("Self-hosted tools").clicked() {
                    self.platform_tools.open = true;
                }
                if ui.button("History").clicked() {
                    self.show_history = true;
                }
            });
            ui.small(format!(
                "Local workspace: {}",
                storage::get_app_data_dir().display()
            ));
            if let Some(error) = self.error.clone() {
                ui.horizontal_wrapped(|ui| {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                    if ui.button("Dismiss").clicked() {
                        self.error = None;
                    }
                });
            }
            if !self.notice.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(&self.notice);
                    if ui.button("Dismiss notice").clicked() {
                        self.notice.clear();
                    }
                });
            }
        });
        let mut action = None;
        egui::SidePanel::left("collections")
            .default_width(260.)
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Collections");
                    if ui.button("+").clicked() {
                        self.show_new_collection = true;
                    }
                });
                ui.text_edit_singleline(&mut self.search);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for c in &self.collections {
                        let response = egui::CollapsingHeader::new(&c.name)
                            .id_salt(&c.id)
                            .default_open(true)
                            .show(ui, |ui| {
                                render_tree(
                                    ui,
                                    &c.id,
                                    None,
                                    &c.requests,
                                    &c.folders,
                                    &c.order,
                                    &self.search,
                                    &mut action,
                                );
                            });
                        response.header_response.context_menu(|ui| {
                            for (label, a) in [
                                ("Add request", Action::Add(c.id.clone(), None)),
                                ("Add folder", Action::NewFolder(c.id.clone(), None)),
                                ("Collection settings", Action::Edit(c.id.clone(), None)),
                                ("Export (redacted)", Action::Export(c.id.clone())),
                                (
                                    "Export v2.1 (redacted)",
                                    Action::ExportPortable(c.id.clone()),
                                ),
                                (
                                    "Export v3 directory (redacted)",
                                    Action::ExportV3(c.id.clone()),
                                ),
                                (
                                    "Export including credentials",
                                    Action::ExportCredentials(c.id.clone()),
                                ),
                                (
                                    "Delete collection (keeps open drafts)",
                                    Action::DeleteCollection(c.id.clone()),
                                ),
                            ] {
                                if ui.button(label).clicked() {
                                    action = Some(a);
                                    ui.close_menu();
                                }
                            }
                        });
                    }
                });
            });
        if let Some(a) = action {
            self.action(a);
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut close = None;
            egui::ScrollArea::horizontal()
                .id_salt("tab_scroll")
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (i, t) in self.tab_manager.tabs.iter().enumerate() {
                            ui.push_id(&t.id, |ui| {
                                if ui
                                    .selectable_label(
                                        self.tab_manager.active_tab_idx == Some(i),
                                        t.title(),
                                    )
                                    .clicked()
                                {
                                    self.tab_manager.active_tab_idx = Some(i);
                                }
                                if ui.small_button("×").clicked() {
                                    close = Some(t.id.clone());
                                }
                            });
                        }
                        if ui.button("+ New Request").clicked() {
                            let mut r = ApiRequest::default();
                            r.auth.auth_type = AuthType::None;
                            self.tab_manager.open_tab(r, None);
                        }
                    });
                });
            if let Some(id) = close {
                self.request_close(id);
            }
            ui.separator();
            let mut send = false;
            let mut code = false;
            let mut save = None;
            let mut tried = None;
            let mut example_source = None;
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                let before = serde_json::to_vec(&tab.request).unwrap_or_default();
                ui.horizontal(|ui| {
                    ui.label("Name");
                    ui.text_edit_singleline(&mut tab.request.name);
                    if tab.pending.is_some() && ui.button("Cancel request").clicked() {
                        if let Some((_, cancel)) = &tab.pending {
                            cancel.cancel();
                        }
                    }
                });
                if !tab.request.settings.verify_tls {
                    ui.colored_label(
                        egui::Color32::YELLOW,
                        "TLS certificate verification is disabled for this request",
                    );
                }
                render_url_bar(
                    ui,
                    &mut tab.request,
                    tab.pending.is_some(),
                    &mut send,
                    &mut code,
                );
                egui::ScrollArea::vertical()
                    .id_salt("request_editor_scroll")
                    .max_height((ui.available_height() * 0.52).max(150.))
                    .show(ui, |ui| {
                        let mut clicked = false;
                        render_request_editor(
                            ui,
                            &mut tab.request,
                            &mut self.request_tab,
                            &mut clicked,
                            tab.collection_id.is_some(),
                        );
                        if clicked {
                            save = Some(tab.id.clone());
                        }
                    });
                example_source = tab
                    .collection_id
                    .as_ref()
                    .map(|cid| (cid.clone(), tab.request.id.clone()));
                tried = render_examples(ui, &mut tab.request, tab.response.as_ref());
                if serde_json::to_vec(&tab.request).unwrap_or_default() != before {
                    tab.is_modified = true;
                }
                ui.separator();
                ui.push_id(&tab.id, |ui| {
                    render_response_viewer(
                        ui,
                        tab.response.as_ref(),
                        tab.pending.is_some(),
                        &mut self.response_tab,
                        &mut self.format_json,
                        &mut self.response_search,
                    );
                });
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label("Create a request or import a collection to get started.");
                });
            }
            if let Some(example) = tried {
                self.action(Action::TryExample(Box::new(example), example_source));
            }
            if send {
                self.send_request();
            }
            if let Some(id) = save {
                self.save_tab(&id, None);
            }
            if code {
                self.show_code = true;
            }
        });
        self.authorize(ctx);
        self.windows(ctx);
    }
}
#[allow(clippy::too_many_arguments)] // Recursive tree view carries the parent identity and ordering.
fn render_tree(
    ui: &mut egui::Ui,
    cid: &str,
    parent: Option<&str>,
    requests: &[ApiRequest],
    folders: &[Folder],
    order: &[String],
    search: &str,
    action: &mut Option<Action>,
) {
    for id in child_order(requests, folders, order) {
        if let Some(r) = requests.iter().find(|r| r.id == id) {
            if !search.is_empty()
                && !format!("{} {}", r.name, r.url)
                    .to_lowercase()
                    .contains(&search.to_lowercase())
            {
                continue;
            }
            let response = ui.selectable_label(false, format!("{}  {}", r.method.as_str(), r.name));
            if response.clicked() {
                *action = Some(Action::Open(cid.into(), Box::new(r.clone())));
            }
            for example in &r.examples {
                ui.indent(&example.id, |ui| {
                    if ui
                        .selectable_label(false, format!("Example: {}", example.name))
                        .clicked()
                    {
                        *action = Some(Action::TryExample(
                            Box::new(example.clone()),
                            Some((cid.into(), r.id.clone())),
                        ));
                    }
                });
            }
            response.context_menu(|ui| {
                for (label, a) in [
                    ("Rename", Action::Rename(cid.into(), r.id.clone(), false)),
                    ("Duplicate", Action::Duplicate(cid.into(), r.id.clone())),
                    (
                        "Move to folder / collection",
                        Action::Move(cid.into(), r.id.clone()),
                    ),
                    (
                        "Move up",
                        Action::Reorder(cid.into(), parent.map(String::from), r.id.clone(), true),
                    ),
                    (
                        "Move down",
                        Action::Reorder(cid.into(), parent.map(String::from), r.id.clone(), false),
                    ),
                    (
                        "Delete (keeps open draft)",
                        Action::Delete(cid.into(), r.id.clone()),
                    ),
                ] {
                    if ui.button(label).clicked() {
                        *action = Some(a);
                        ui.close_menu();
                    }
                }
            });
        } else if let Some(f) = folders.iter().find(|f| f.id == id) {
            let response = egui::CollapsingHeader::new(&f.name)
                .id_salt(&f.id)
                .default_open(true)
                .show(ui, |ui| {
                    render_tree(
                        ui,
                        cid,
                        Some(&f.id),
                        &f.requests,
                        &f.folders,
                        &f.order,
                        search,
                        action,
                    );
                });
            response.header_response.context_menu(|ui| {
                for (label, a) in [
                    ("Add request", Action::Add(cid.into(), Some(f.id.clone()))),
                    (
                        "Add folder",
                        Action::NewFolder(cid.into(), Some(f.id.clone())),
                    ),
                    ("Rename", Action::Rename(cid.into(), f.id.clone(), true)),
                    (
                        "Folder settings",
                        Action::Edit(cid.into(), Some(f.id.clone())),
                    ),
                    ("Move folder", Action::MoveFolder(cid.into(), f.id.clone())),
                    (
                        "Move up",
                        Action::Reorder(cid.into(), parent.map(String::from), f.id.clone(), true),
                    ),
                    (
                        "Move down",
                        Action::Reorder(cid.into(), parent.map(String::from), f.id.clone(), false),
                    ),
                    (
                        "Delete folder (keeps open drafts)",
                        Action::DeleteFolder(cid.into(), f.id.clone()),
                    ),
                ] {
                    if ui.button(label).clicked() {
                        *action = Some(a);
                        ui.close_menu();
                    }
                }
            });
        }
    }
}
fn merge_changes(
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
    target: &mut BTreeMap<String, String>,
) {
    for key in before.keys() {
        if !after.contains_key(key) {
            target.remove(key);
        }
    }
    for (k, v) in after {
        if before.get(k) != Some(v) {
            target.insert(k.clone(), v.clone());
        }
    }
}
fn folder_choices(
    ui: &mut egui::Ui,
    folders: &[Folder],
    selected: &mut Option<String>,
    prefix: &str,
) {
    for f in folders {
        let name = format!("{prefix}{}", f.name);
        ui.selectable_value(selected, Some(f.id.clone()), &name);
        folder_choices(ui, &f.folders, selected, &format!("{name} / "));
    }
}

impl RustApiApp {
    fn windows(&mut self, ctx: &egui::Context) {
        let execution_secrets = self.execution_secrets();
        self.documents.window(ctx);
        self.flow_editor.window(
            ctx,
            &self.collections,
            self.service.clone(),
            &execution_secrets,
        );
        self.protocol_console
            .window(ctx, self.service.clone(), &execution_secrets);
        if let Some(imported) = self.api_tools.window(
            ctx,
            &self.collections,
            self.service.clone(),
            &execution_secrets,
        ) {
            self.import(Ok(imported));
        }
        if let Some(action) = self.platform_tools.window(
            ctx,
            &self.collections,
            self.service.clone(),
            &execution_secrets,
        ) {
            match action {
                PlatformAction::Import(collection) => {
                    self.import(Ok(rustapi::import::ImportResult {
                        collection: Some(*collection),
                        environment: None,
                        warnings: vec![],
                        diagnostics: vec![],
                    }))
                }
                PlatformAction::Draft(request) => self.tab_manager.open_tab(*request, None),
                PlatformAction::Secrets(values) => self.session_secrets.extend(values),
                PlatformAction::ExternalSecrets(values) => {
                    self.session_secrets
                        .retain(|key, _| !key.starts_with("ext/"));
                    self.session_secrets.extend(values);
                }
                PlatformAction::Linked {
                    local_id,
                    remote_id,
                    revision,
                } => {
                    if let Some(mut collection) =
                        self.collections.iter().find(|c| c.id == local_id).cloned()
                    {
                        collection
                            .imported_settings
                            .insert("team.remote_id".into(), remote_id.into());
                        collection
                            .imported_settings
                            .insert("team.revision".into(), revision.into());
                        self.commit_collection(collection);
                    }
                }
            }
        }
        if self.show_new_collection {
            egui::Window::new("New collection").show(ctx, |ui| {
                ui.text_edit_singleline(&mut self.new_name);
                if ui.button("Create").clicked()
                    && !self.new_name.trim().is_empty()
                    && self.commit_collection(Collection::new(self.new_name.trim()))
                {
                    self.new_name.clear();
                    self.show_new_collection = false;
                }
                if ui.button("Cancel").clicked() {
                    self.show_new_collection = false;
                }
            });
        }
        if self.show_import {
            egui::Window::new("Import collections / environments / cURL")
                .default_width(600.)
                .show(ctx, |ui| {
                    ui.label("JSON imports create a managed copy. Source files are unchanged.");
                    if ui.button("Choose collection or environment JSON").clicked() {
                        self.choose_import();
                    }
                    if ui.button("Choose v3 collection directory").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            self.import(rustapi::import::path(&path));
                        }
                    }
                    ui.label("Or paste a cURL command");
                    ui.add(
                        egui::TextEdit::multiline(&mut self.import_text)
                            .code_editor()
                            .desired_rows(5)
                            .desired_width(f32::INFINITY),
                    );
                    if ui.button("Import cURL").clicked() {
                        self.import(rustapi::import::curl(&self.import_text));
                    }
                    if ui.button("Close").clicked() {
                        self.show_import = false;
                    }
                });
        }
        self.import_review_window(ctx);
        if let Some(id) = self.save_as.clone() {
            egui::Window::new("Save request into collection")
                .collapsible(false)
                .show(ctx, |ui| {
                    egui::ComboBox::from_id_salt("save_collection")
                        .selected_text(
                            self.save_collection
                                .as_ref()
                                .and_then(|id| self.collections.iter().find(|c| &c.id == id))
                                .map(|c| c.name.as_str())
                                .unwrap_or("Select collection"),
                        )
                        .show_ui(ui, |ui| {
                            for c in &self.collections {
                                if ui
                                    .selectable_value(
                                        &mut self.save_collection,
                                        Some(c.id.clone()),
                                        &c.name,
                                    )
                                    .changed()
                                {
                                    self.save_folder = None;
                                }
                            }
                        });
                    if let Some(c) = self
                        .save_collection
                        .as_ref()
                        .and_then(|id| self.collections.iter().find(|c| &c.id == id))
                    {
                        egui::ComboBox::from_id_salt("save_folder")
                            .selected_text("Target folder")
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.save_folder, None, "Collection root");
                                folder_choices(ui, &c.folders, &mut self.save_folder, "");
                            });
                    }
                    if self.collections.is_empty() {
                        ui.label("Create a collection first.");
                        if ui.button("New collection").clicked() {
                            self.show_new_collection = true;
                        }
                    }
                    if ui.button("Save").clicked() {
                        if let Some(c) = self.save_collection.clone() {
                            if self.save_tab(&id, Some((c, self.save_folder.clone()))) {
                                self.save_as = None;
                                if self.close_pending.as_ref() == Some(&id) {
                                    self.close_tab(&id);
                                    self.close_pending = None;
                                }
                            }
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        self.save_as = None;
                    }
                });
        }
        if let Some(id) = self.close_pending.clone() {
            if self.save_as.is_none() {
                egui::Window::new("Unsaved request")
                    .collapsible(false)
                    .show(ctx, |ui| {
                        ui.label("Save this request before closing?");
                        if ui.button("Save").clicked() && self.save_tab(&id, None) {
                            self.close_tab(&id);
                            self.close_pending = None;
                        }
                        if ui.button("Discard changes and close").clicked() {
                            self.close_tab(&id);
                            self.close_pending = None;
                        }
                        if ui.button("Cancel").clicked() {
                            self.close_pending = None;
                        }
                    });
            }
        }
        if self.exit_pending {
            egui::Window::new("Unsaved requests on exit").collapsible(false).show(ctx,|ui|{
            ui.label("Save requests to collections, or keep recoverable local drafts for next launch.");
            if ui.button("Save all").clicked(){let ids:Vec<_>=self.tab_manager.tabs.iter().filter(|t|t.is_modified).map(|t|t.id.clone()).collect();let mut saved=true;for id in ids{if !self.save_tab(&id,None){saved=false;break;}}if saved{self.allow_exit=true;ctx.send_viewport_cmd(egui::ViewportCommand::Close);}}
            if ui.button("Exit and keep drafts").clicked(){let result=storage::save_drafts(&self.tab_manager);if self.result(result).is_some(){self.allow_exit=true;ctx.send_viewport_cmd(egui::ViewportCommand::Close);}}
            if ui.button("Discard drafts and exit").clicked(){self.tab_manager=TabManager::new();self.persist_drafts();self.allow_exit=true;ctx.send_viewport_cmd(egui::ViewportCommand::Close);}
            if ui.button("Cancel").clicked(){self.exit_pending=false;}
        });
        }
        if let Some((cid, target, folder, mut name)) = self.rename.clone() {
            egui::Window::new("Rename").show(ctx, |ui| {
                ui.text_edit_singleline(&mut name);
                self.rename = Some((cid.clone(), target.clone(), folder, name.clone()));
                if ui.button("Save").clicked() && !name.trim().is_empty() {
                    if let Some(mut c) = self.collections.iter().find(|c| c.id == cid).cloned() {
                        if folder {
                            if let Some(f) = c.folder_mut(&target) {
                                f.name = name.clone();
                            }
                        } else if let Some(r) = c.request_mut(&target) {
                            r.name = name.clone();
                        }
                        if self.commit_collection(c) {
                            if !folder {
                                for t in &mut self.tab_manager.tabs {
                                    if t.collection_id.as_ref() == Some(&cid)
                                        && t.request.id == target
                                    {
                                        t.request.name = name.clone();
                                    }
                                }
                            }
                            self.rename = None;
                        }
                    }
                }
                if ui.button("Cancel").clicked() {
                    self.rename = None;
                }
            });
        }
        if let Some((_cid, folder)) = self.edit.clone() {
            if let Some(mut c) = self.edit_draft.clone() {
                egui::Window::new("Collection / folder settings")
                    .default_width(600.)
                    .vscroll(true)
                    .show(ctx, |ui| {
                        let (name, description, vars, auth, pre, post) = if let Some(id) = &folder {
                            if let Some(f) = c.folder_mut(id) {
                                (
                                    &mut f.name,
                                    &mut f.description,
                                    &mut f.variables,
                                    &mut f.auth,
                                    &mut f.pre_script,
                                    &mut f.post_script,
                                )
                            } else {
                                return;
                            }
                        } else {
                            (
                                &mut c.name,
                                &mut c.description,
                                &mut c.variables,
                                &mut c.auth,
                                &mut c.pre_script,
                                &mut c.post_script,
                            )
                        };
                        ui.label("Name");
                        ui.text_edit_singleline(name);
                        ui.label("Description");
                        ui.add(
                            egui::TextEdit::multiline(description)
                                .desired_rows(4)
                                .desired_width(f32::INFINITY),
                        );
                        ui.separator();
                        ui.label("Variables");
                        edit_variables(ui, vars);
                        ui.separator();
                        render_auth_editor(ui, auth);
                        ui.label("Pre-request script");
                        ui.add(egui::TextEdit::multiline(pre).code_editor());
                        ui.label("Post-response script");
                        ui.add(egui::TextEdit::multiline(post).code_editor());
                        if ui.button("Save").clicked() && self.commit_collection(c.clone()) {
                            self.edit = None;
                        }
                        if ui.button("Cancel").clicked() {
                            self.edit = None;
                        }
                        // Keep edits staged without writing until Save.
                        if self.edit.is_some() {
                            self.collection_editor_cache(&c);
                        }
                    });
            }
        }
        if let Some((cid, rid)) = self.move_request.clone() {
            egui::Window::new("Move request").show(ctx, |ui| {
                egui::ComboBox::from_id_salt("move_collection")
                    .selected_text(
                        self.move_collection
                            .as_ref()
                            .and_then(|id| self.collections.iter().find(|c| &c.id == id))
                            .map(|c| c.name.as_str())
                            .unwrap_or("Select collection"),
                    )
                    .show_ui(ui, |ui| {
                        for c in &self.collections {
                            if ui
                                .selectable_value(
                                    &mut self.move_collection,
                                    Some(c.id.clone()),
                                    &c.name,
                                )
                                .changed()
                            {
                                self.move_folder = None;
                            }
                        }
                    });
                if let Some(c) = self
                    .move_collection
                    .as_ref()
                    .and_then(|id| self.collections.iter().find(|c| &c.id == id))
                {
                    egui::ComboBox::from_id_salt("move_folder")
                        .selected_text("Target folder")
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.move_folder, None, "Collection root");
                            folder_choices(ui, &c.folders, &mut self.move_folder, "");
                        });
                }
                if ui.button("Move").clicked() {
                    if let Some(target) = self.move_collection.clone() {
                        self.move_to(&cid, &rid, &target, self.move_folder.clone());
                    }
                }
                if ui.button("Cancel").clicked() {
                    self.move_request = None;
                }
            });
        }
        self.environment_window(ctx);
        self.runner_window(ctx);
        self.vault_window(ctx);
        if self.show_debug {
            egui::Window::new("Debug console").default_size([900.,550.]).vscroll(true).show(ctx,|ui| {
                let events=self.service.debug.snapshot();
                ui.horizontal(|ui|{
                    ui.label(format!("{} / 256 session events",events.len()));
                    if ui.button("Clear console").clicked(){self.service.debug.clear();}
                    if ui.button("Copy redacted events").clicked(){ui.output_mut(|output|output.copied_text=serde_json::to_string_pretty(&serde_json::json!({"schema_version":1,"events":events})).unwrap_or_default());}
                });
                ui.label("Filter console");ui.text_edit_singleline(&mut self.debug_filter);
                ui.small("Session memory only. Network paths, query values, bodies and header values are omitted. Script values use conservative context redaction; encoded/custom data may need review before sharing.");
                for (index,event) in events.iter().enumerate(){let text=serde_json::to_string(&event).unwrap_or_default();if !self.debug_filter.is_empty() && !text.to_lowercase().contains(&self.debug_filter.to_lowercase()){continue;}
                    egui::CollapsingHeader::new(format!("{} {} {}",event.time,event.kind,event.level)).id_salt(("debug_event",index,&event.time)).show(ui,|ui|{ui.monospace(serde_json::to_string_pretty(&event.details).unwrap_or_default());});
                }
                if ui.button("Close console").clicked(){self.show_debug=false;}
            });
        }
        if self.show_inspector {
            egui::Window::new("Variables and effective headers").default_size([900.,500.]).vscroll(true).show(ctx,|ui| {
                ui.checkbox(&mut self.reveal_inspector,"Reveal values including credentials");
                if let Some(tab)=self.tab_manager.active_tab() {
                    let request=self.contextual_request(tab);
                    let vars = self.variables_for_request(&request);
                    let origins=tab.collection_id.as_ref().and_then(|id|self.collections.iter().find(|c| &c.id==id)).map(|c|c.variable_origins(&tab.request.id)).unwrap_or_default();
                    render_variable_inspector(ui,&vars,origins,self.environment(),self.reveal_inspector);
                    ui.separator();ui.strong("Effective configured headers");
                    match vars.resolve_request(&request).and_then(|r|rustapi::http::effective_headers(&r)) {
                        Ok(headers)=>{for (key,value) in headers {
                            // Default masking covers every header value, including custom credential names.
                            ui.horizontal(|ui|{ui.monospace(key);ui.monospace(if self.reveal_inspector{&value}else{"[masked]"});});
                        }},Err(error)=>{ui.colored_label(egui::Color32::LIGHT_RED,error.to_string());}
                    }
                    ui.small("Active scopes only. Narrower scopes override broader ones. Header view covers configured/auth/body fields; cookies, Host and Content-Length are transport managed. Multipart boundaries are generated when sending.");
                } else {ui.label("Open a request to inspect its active context.");}
                if ui.button("Close inspector").clicked(){self.show_inspector=false;self.reveal_inspector=false;}
            });
        }
        if self.show_globals {
            egui::Window::new("Global variables (.local/globals.json)")
                .default_size([600., 400.])
                .vscroll(true)
                .show(ctx, |ui| {
                    ui.small("Global variables have the lowest scope precedence (global < collection < env < overrides < iteration < local < vault). They persist across app launches and are stored in .local/.");
                    let mut remove_key = None;
                    for (k, v) in &mut self.global_variables.variables {
                        ui.horizontal(|ui| {
                            ui.monospace(k);
                            ui.text_edit_singleline(v);
                            if ui.small_button("🗑").clicked() {
                                remove_key = Some(k.clone());
                            }
                        });
                    }
                    if let Some(k) = remove_key {
                        self.global_variables.variables.remove(&k);
                        let _ = self.global_variables.save(&storage::get_app_data_dir());
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut self.global_key).on_hover_text("New variable name");
                        ui.text_edit_singleline(&mut self.global_value).on_hover_text("Value");
                        if ui.button("Add global variable").clicked() && !self.global_key.trim().is_empty() {
                            self.global_variables.variables.insert(self.global_key.trim().to_string(), self.global_value.clone());
                            self.global_key.clear();
                            self.global_value.clear();
                            let _ = self.global_variables.save(&storage::get_app_data_dir());
                        }
                    });
                    if ui.button("Save & Close").clicked() {
                        let _ = self.global_variables.save(&storage::get_app_data_dir());
                        self.show_globals = false;
                    }
                });
        }
        if self.show_overrides {
            egui::Window::new("Local environment overrides (.local/environment-overrides.json)")
                .default_size([700., 450.])
                .vscroll(true)
                .show(ctx, |ui| {
                    let active_env = self.environment().map(|e| (e.id.clone(), e.name.clone()));
                    if let Some((env_id, env_name)) = active_env {
                        ui.strong(format!("Active environment: {}", env_name));
                        ui.small("Local overrides take precedence over the tracked environment file, but are kept in .local/ and never committed or exported.");
                        let mut env_map = self.environment_overrides.get_overrides(&env_id, &env_name);
                        let mut changed = false;
                        let mut remove_key = None;
                        for (k, v) in &mut env_map {
                            ui.horizontal(|ui| {
                                ui.monospace(k);
                                ui.text_edit_singleline(v);
                                if ui.small_button("🗑").clicked() {
                                    remove_key = Some(k.clone());
                                }
                            });
                        }
                        if let Some(k) = remove_key {
                            env_map.remove(&k);
                            changed = true;
                        }
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.override_key).on_hover_text("Variable name");
                            ui.text_edit_singleline(&mut self.override_value).on_hover_text("Local override value");
                            if ui.button("Add override").clicked() && !self.override_key.trim().is_empty() {
                                env_map.insert(self.override_key.trim().to_string(), self.override_value.clone());
                                self.override_key.clear();
                                self.override_value.clear();
                                changed = true;
                            }
                        });
                        if changed {
                            self.environment_overrides.overrides.insert(env_id, env_map);
                            let _ = self.environment_overrides.save(&storage::get_app_data_dir());
                        }
                    } else {
                        ui.label("Select an active environment to manage its local overrides.");
                    }
                    if ui.button("Close").clicked() {
                        let _ = self.environment_overrides.save(&storage::get_app_data_dir());
                        self.show_overrides = false;
                    }
                });
        }
        if self.show_cookies {
            egui::Window::new("Workspace cookies")
                .default_width(600.)
                .vscroll(true)
                .show(ctx, |ui| {
                    ui.label(if self.service.cookies.persistent() {
                        "Encrypted persistent jar (unlocked)"
                    } else {
                        "Session jar"
                    });
                    if let Some(error) = self.service.cookies.last_error() {
                        ui.colored_label(egui::Color32::LIGHT_RED, error);
                    }
                    if !self.service.cookies.persistent()
                        && ui.button("Keep jar in encrypted vault").clicked()
                    {
                        if self.vault_unlocked {
                            self.result(self.service.cookies.unlock(
                                &storage::get_app_data_dir().join(".local/cookies.json"),
                                &self.vault_password,
                            ));
                        } else {
                            self.error = Some("Unlock Local encrypted secrets first".into());
                        }
                    }
                    for cookie in self.service.cookies.entries() {
                        ui.push_id((&cookie.domain, &cookie.path, &cookie.name), |ui| {
                            ui.horizontal(|ui| {
                                ui.label(format!(
                                    "{} · {}{} · Secure={} HttpOnly={} SameSite={}",
                                    cookie.name,
                                    cookie.domain,
                                    cookie.path,
                                    cookie.secure,
                                    cookie.http_only,
                                    cookie.same_site
                                ));
                                if ui.small_button("Edit").clicked() {
                                    self.cookie_url = format!(
                                        "{}://{}{}",
                                        if cookie.secure { "https" } else { "http" },
                                        cookie.domain.trim_start_matches('.'),
                                        cookie.path
                                    );
                                    self.cookie_value = format!(
                                        "{}={}; Path={}{}{}{}{}",
                                        cookie.name,
                                        cookie.value,
                                        cookie.path,
                                        if cookie.host_only {
                                            String::new()
                                        } else {
                                            format!("; Domain={}", cookie.domain)
                                        },
                                        if cookie.secure { "; Secure" } else { "" },
                                        if cookie.http_only { "; HttpOnly" } else { "" },
                                        if cookie.same_site.is_empty() {
                                            String::new()
                                        } else {
                                            format!("; SameSite={}", cookie.same_site)
                                        }
                                    );
                                    if let Some(expires) = cookie
                                        .expiration_date
                                        .and_then(|t| chrono::DateTime::from_timestamp(t as i64, 0))
                                    {
                                        self.cookie_value.push_str(&format!(
                                            "; Expires={}",
                                            expires.format("%a, %d %b %Y %H:%M:%S GMT")
                                        ));
                                    }
                                }
                                if ui.small_button("Delete").clicked() {
                                    self.result(self.service.cookies.remove(
                                        &cookie.domain,
                                        &cookie.path,
                                        &cookie.name,
                                    ));
                                }
                            });
                        });
                    }
                    if ui.button("Clear all cookies").clicked() {
                        self.service.cookies.clear();
                    }
                    ui.label("Origin URL");
                    ui.text_edit_singleline(&mut self.cookie_url);
                    ui.label("Set-Cookie value (use Max-Age=0 to delete a cookie)");
                    ui.add(egui::TextEdit::singleline(&mut self.cookie_value).password(true));
                    if ui.button("Set cookie").clicked() {
                        let result = self
                            .service
                            .cookies
                            .set(&self.cookie_url, &self.cookie_value);
                        self.result(result);
                    }
                    ui.separator();
                    ui.label("Browser capture / sync (manual merge; deletions are local)");
                    ui.hyperlink_to(
                        "Browser connector setup",
                        "https://github.com/wulfnb/rustapi/tree/main/browser-connector",
                    );
                    if ui.button("Export browser connector").clicked() {
                        if let Some(parent) = rfd::FileDialog::new().pick_folder() {
                            if let Some(path) = self.result(rustapi::http::export_connector(&parent)) {
                                ui.ctx().copy_text(path.display().to_string());
                            }
                        }
                    }
                    ui.small("Export creates an unpacked Chromium extension and copies its directory path.");
                    if self.cookie_bridge.is_none()
                        && ui.button("Enable browser connection").clicked()
                    {
                        self.cookie_bridge = self.result(rustapi::http::CookieBridge::start(
                            self.service.cookies.clone(),
                        ));
                    }
                    if let Some(bridge) = &self.cookie_bridge {
                        ui.label(format!("Listening on {}", bridge.server.address));
                        if ui.button("Copy private pairing URL").clicked() {
                            ui.ctx().copy_text(bridge.pairing.clone());
                        }
                        if ui.button("Disable browser connection").clicked() {
                            self.cookie_bridge = None;
                        }
                    }
                    if ui.button("Close").clicked() {
                        self.show_cookies = false;
                    }
                });
        }
        if self.show_history {
            let history = storage::load_history();
            if let Some(action) = render_history_panel(ctx, &mut self.show_history, &history) {
                match action {
                    HistoryAction::LoadRequest(r) => self.tab_manager.open_tab(*r, None),
                    HistoryAction::ClearHistory => {
                        let result = storage::save_history(&RequestHistory::new(100));
                        self.result(result);
                    }
                }
            }
        }
        if self.show_code {
            egui::Window::new("Generate code")
                .default_size([700., 500.])
                .show(ctx, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for lang in CodeGenLanguage::all() {
                            ui.selectable_value(&mut self.code_language, *lang, lang.as_str());
                        }
                    });
                    ui.checkbox(&mut self.include_secrets, "Include credentials in code");
                    if let Some(tab) = self.tab_manager.active_tab() {
                        let request = self.contextual_request(tab);
                        let vars = self.variables_for_request(&request);
                        let code = match vars.resolve_request(&request) {
                            Ok(r) => crate::http::generate_code(
                                &if self.include_secrets {
                                    r
                                } else {
                                    rustapi::secrets::redact_request(&r)
                                },
                                self.code_language,
                                None,
                            ),
                            Err(e) => e.to_string(),
                        };
                        let mut code = code;
                        if !self.include_secrets {
                            for v in self.secrets.values().chain(self.session_secrets.values()) {
                                if !v.is_empty() {
                                    code = code.replace(v, "[redacted]");
                                }
                            }
                        }
                        egui::ScrollArea::both().max_height(360.).show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut code.as_str())
                                    .code_editor()
                                    .desired_width(f32::INFINITY),
                            );
                        });
                        if ui.button("Copy").clicked() {
                            ui.output_mut(|o| o.copied_text = code);
                        }
                    }
                    if ui.button("Close").clicked() {
                        self.show_code = false;
                    }
                });
        }
    }
    fn collection_editor_cache(&mut self, c: &Collection) {
        self.edit_draft = Some(c.clone());
    }
    fn move_to(&mut self, cid: &str, rid: &str, target: &str, folder: Option<String>) {
        let Some(mut source) = self.collections.iter().find(|c| c.id == cid).cloned() else {
            return;
        };
        if self.moving_folder {
            let Some(f) = source.remove_folder(rid) else {
                return;
            };
            fn contains(f: &Folder, id: &str) -> bool {
                f.id == id || f.folders.iter().any(|c| contains(c, id))
            }
            if folder.as_ref().is_some_and(|id| contains(&f, id)) {
                self.error = Some("A folder cannot be moved into itself or its descendants".into());
                return;
            }
            let mut dest = if cid == target {
                source.clone()
            } else {
                let Some(c) = self.collections.iter().find(|c| c.id == target).cloned() else {
                    return;
                };
                c
            };
            if let Some(fid) = folder {
                let Some(parent) = dest.folder_mut(&fid) else {
                    return;
                };
                parent.folders.push(f);
            } else {
                dest.folders.push(f);
            }
            if self.commit_collection(dest) && (cid == target || self.commit_collection(source)) {
                self.detach_missing_tabs();
                self.move_request = None;
            }
            return;
        }
        let Some(r) = source.remove_request(rid) else {
            return;
        };
        if cid == target {
            if let Some(fid) = folder {
                if let Some(f) = source.folder_mut(&fid) {
                    f.requests.push(r);
                }
            } else {
                source.requests.push(r);
            }
            if self.commit_collection(source) {
                self.move_request = None;
            }
        } else {
            let Some(mut dest) = self.collections.iter().find(|c| c.id == target).cloned() else {
                return;
            };
            if dest.request(rid).is_some() {
                self.error = Some("Target contains this request ID".into());
                return;
            }
            if let Some(fid) = folder {
                let Some(f) = dest.folder_mut(&fid) else {
                    return;
                };
                f.requests.push(r);
            } else {
                dest.requests.push(r);
            }
            // Save destination first: a failed second write retains a recoverable source copy.
            if self.commit_collection(dest) && self.commit_collection(source) {
                for t in &mut self.tab_manager.tabs {
                    if t.collection_id.as_deref() == Some(cid) && t.request.id == rid {
                        t.collection_id = Some(target.into());
                    }
                }
                self.move_request = None;
            }
        }
        self.persist_drafts();
    }
    fn environment_window(&mut self, ctx: &egui::Context) {
        if !self.show_environments {
            return;
        }
        egui::Window::new("Environments")
            .default_width(620.)
            .vscroll(true)
            .show(ctx, |ui| {
                if ui.button("New environment").clicked() {
                    self.environments.push(Environment::new("New Environment"));
                }
                let mut save = None;
                let mut delete = None;
                let mut export_environment = None;
                for (i, e) in self.environments.iter_mut().enumerate() {
                    egui::CollapsingHeader::new(&e.name)
                        .id_salt(&e.id)
                        .show(ui, |ui| {
                            ui.text_edit_singleline(&mut e.name);
                            ui.label("Use {{vault:name}} for sensitive values");
                            render_key_value_editor(ui, &mut e.variables);
                            if ui.button("Save environment").clicked() {
                                e.updated_at = chrono::Utc::now();
                                save = Some(e.clone());
                            }
                            if ui.button("Export environment (redacted)").clicked() {
                                export_environment = Some(e.clone());
                            }
                            if ui.button("Delete environment").clicked() {
                                delete = Some(i);
                            }
                        });
                }
                if let Some(e) = save {
                    self.result(storage::save_environment(&e));
                }
                if let Some(environment) = export_environment {
                    if let Some(path) = self.dialogs.export(&environment.name) {
                        let result = rustapi::export::environment(&environment, false).and_then(
                            |exported| {
                                rustapi::workspace::atomic_write(
                                    &path,
                                    &serde_json::to_vec_pretty(&exported.document)?,
                                )
                            },
                        );
                        self.result(result);
                    }
                }
                if let Some(i) = delete {
                    if self
                        .result(storage::delete_environment(&self.environments[i]))
                        .is_some()
                    {
                        let e = self.environments.remove(i);
                        if self.active_environment_id.as_ref() == Some(&e.id) {
                            self.active_environment_id = None;
                        }
                    }
                }
                if ui.button("Save all and close").clicked() {
                    let mut good = true;
                    for e in self.environments.clone() {
                        if self.result(storage::save_environment(&e)).is_none() {
                            good = false;
                        }
                    }
                    if good {
                        self.show_environments = false;
                    }
                }
            });
    }
    fn runner_window(&mut self, ctx: &egui::Context) {
        if !self.show_runner {
            return;
        }
        egui::Window::new("Collection runner")
            .default_size([700., 500.])
            .show(ctx, |ui| {
                egui::ComboBox::from_id_salt("run_collection")
                    .selected_text(
                        self.run_collection
                            .as_ref()
                            .and_then(|id| self.collections.iter().find(|c| &c.id == id))
                            .map(|c| c.name.as_str())
                            .unwrap_or("Select collection"),
                    )
                    .show_ui(ui, |ui| {
                        for c in &self.collections {
                            if ui
                                .selectable_value(
                                    &mut self.run_collection,
                                    Some(c.id.clone()),
                                    &c.name,
                                )
                                .changed()
                            {
                                self.run_folder = None;
                            }
                        }
                    });
                if let Some(c) = self
                    .run_collection
                    .as_ref()
                    .and_then(|id| self.collections.iter().find(|c| &c.id == id))
                {
                    egui::ComboBox::from_id_salt("run_folder")
                        .selected_text("Select folder")
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.run_folder, None, "Entire collection");
                            folder_choices(ui, &c.folders, &mut self.run_folder, "");
                        });
                }
                ui.label("Uses the active environment and saved collection requests");
                ui.horizontal(|ui| {
                    ui.label("Iterations (0 = number of data rows, or 1)");
                    ui.add(
                        egui::DragValue::new(&mut self.run_options.iterations).range(0..=100_000),
                    );
                });
                ui.checkbox(&mut self.run_options.stop_on_failure, "Stop on failure");
                ui.horizontal(|ui| {
                    ui.label("Delay between requests (ms)");
                    ui.add(egui::DragValue::new(&mut self.run_options.delay_ms).range(0..=60_000));
                });
                ui.horizontal(|ui| {
                    ui.label("CSV / JSON iteration data");
                    ui.text_edit_singleline(&mut self.data_path);
                    if ui.button("Choose").clicked() {
                        if let Some(p) = rfd::FileDialog::new()
                            .add_filter("Data", &["csv", "json"])
                            .pick_file()
                        {
                            self.data_path = p.to_string_lossy().into();
                        }
                    }
                });
                if self.run_cancel.is_some() {
                    if ui.button("Cancel run").clicked() {
                        if let Some(c) = &self.run_cancel {
                            c.cancel();
                        }
                    }
                } else if ui.button("Run").clicked() {
                    self.start_run();
                }
                if let Some(report) = &self.report {
                    ui.label(if report.passed() {
                        "Run passed"
                    } else if report.cancelled {
                        "Run cancelled"
                    } else {
                        "Run failed"
                    });
                    egui::ScrollArea::vertical()
                        .max_height(250.)
                        .show(ui, |ui| {
                            for e in &report.entries {
                                ui.collapsing(
                                    format!(
                                        "{} · {} · {} · {} ms",
                                        if e.passed() { "PASS" } else { "FAIL" },
                                        e.iteration,
                                        e.name,
                                        e.time_ms
                                    ),
                                    |ui| {
                                        ui.label(if e.skipped {
                                            "Skipped".into()
                                        } else {
                                            format!("HTTP {}", e.status)
                                        });
                                        if let Some(error) = &e.error {
                                            ui.label(error);
                                        }
                                        for t in &e.tests {
                                            ui.label(format!(
                                                "{} {}",
                                                if t.passed { "PASS" } else { "FAIL" },
                                                t.name
                                            ));
                                        }
                                    },
                                );
                            }
                        });
                    for (label, bytes) in [
                        (
                            "Export JSON",
                            serde_json::to_vec_pretty(report).unwrap_or_default(),
                        ),
                        ("Export JUnit", report.junit().into_bytes()),
                    ] {
                        if ui.button(label).clicked() {
                            if let Some(p) = rfd::FileDialog::new().save_file() {
                                let result = rustapi::workspace::atomic_write(&p, &bytes);
                                self.result(result);
                            }
                        }
                    }
                }
                if ui.button("Close").clicked() {
                    self.show_runner = false;
                }
            });
    }
    fn vault_window(&mut self, ctx: &egui::Context) {
        if !self.show_vault {
            return;
        }
        egui::Window::new("Local encrypted secrets")
            .default_width(550.)
            .show(ctx, |ui| {
                ui.label(
                    "Use {{vault:name}} in requests and variables. The password is never saved.",
                );
                let path = storage::get_app_data_dir().join(".local/vault.json");
                if !self.vault_unlocked {
                    ui.label("Vault password (12+ characters)");
                    ui.add(egui::TextEdit::singleline(&mut self.vault_password).password(true));
                    if ui.button("Unlock / create vault").clicked() {
                        if let Some(secrets) =
                            self.result(rustapi::secrets::load(&path, &self.vault_password))
                        {
                            self.secrets = secrets;
                            let token_path =
                                storage::get_app_data_dir().join(".local/oauth-tokens.json");
                            self.vault_unlocked = self
                                .result(
                                    self.service
                                        .unlock_tokens(&token_path, &self.vault_password),
                                )
                                .is_some();
                            let cookie_path =
                                storage::get_app_data_dir().join(".local/cookies.json");
                            if self.vault_unlocked && cookie_path.exists() {
                                self.result(
                                    self.service
                                        .cookies
                                        .unlock(&cookie_path, &self.vault_password),
                                );
                            }
                        }
                    }
                } else {
                    let mut delete = None;
                    for key in self.secrets.keys() {
                        ui.horizontal(|ui| {
                            ui.label(key);
                            if ui.button("Delete").clicked() {
                                delete = Some(key.clone());
                            }
                        });
                    }
                    if let Some(key) = delete {
                        let mut next = self.secrets.clone();
                        next.remove(&key);
                        if self
                            .result(rustapi::secrets::save(&path, &self.vault_password, &next))
                            .is_some()
                        {
                            self.secrets = next;
                        }
                    }
                    ui.label("Name");
                    ui.text_edit_singleline(&mut self.secret_name);
                    ui.label("Value");
                    ui.add(egui::TextEdit::singleline(&mut self.secret_value).password(true));
                    if ui.button("Save secret").clicked() && !self.secret_name.trim().is_empty() {
                        let mut next = self.secrets.clone();
                        next.insert(self.secret_name.trim().into(), self.secret_value.clone());
                        if self
                            .result(rustapi::secrets::save(&path, &self.vault_password, &next))
                            .is_some()
                        {
                            self.secrets = next;
                            self.secret_value.clear();
                        }
                    }
                    if ui.button("Lock vault").clicked() {
                        self.platform_tools.cancel();
                        self.service.lock_tokens();
                        self.cookie_bridge = None;
                        self.service.cookies.lock();
                        self.secrets.clear();
                        self.session_secrets.clear();
                        self.vault_password.clear();
                        self.vault_unlocked = false;
                    }
                    ui.separator();
                    ui.label("OAuth tokens (values hidden)");
                    for token in self.service.token_summaries() {
                        ui.horizontal(|ui| {
                            ui.label(format!(
                                "{} · {}s remaining · refresh: {}",
                                token.label, token.valid_for_seconds, token.refreshable
                            ));
                            if ui.button("Forget token").clicked() {
                                self.result(self.service.forget_token(&token.id));
                            }
                        });
                    }
                }
                if ui.button("Close").clicked() {
                    self.show_vault = false;
                }
            });
    }
}
fn edit_variables(ui: &mut egui::Ui, vars: &mut std::collections::HashMap<String, String>) {
    let mut rows: Vec<_> = vars.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    let mut delete = None;
    for (i, (k, v)) in rows.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            ui.horizontal(|ui| {
                ui.text_edit_singleline(k);
                ui.text_edit_singleline(v);
                if ui.small_button("Remove").clicked() {
                    delete = Some(i);
                }
            });
        });
    }
    if let Some(i) = delete {
        rows.remove(i);
    }
    if ui.button("Add variable").clicked() {
        rows.push((format!("variable_{}", rows.len() + 1), String::new()));
    }
    *vars = rows.into_iter().collect();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_response_attribution_save_failure_and_draft_recovery() {
        let profile_guard = crate::test_support::Profile::new();
        let profile = &profile_guard.directory;
        let mut app = RustApiApp::default();
        let mut c = Collection::new("Collection");
        let a = ApiRequest::new("A");
        let b = ApiRequest::new("B");
        let z = ApiRequest::new("C");
        let cid = c.id.clone();
        let bid = b.id.clone();
        let zid = z.id.clone();
        c.requests = vec![a.clone(), b.clone(), z];
        assert!(app.commit_collection(c));
        app.tab_manager.open_tab(b.clone(), Some(cid.clone()));
        let tabid = app.tab_manager.active_tab().unwrap().id.clone();
        app.action(Action::Delete(cid.clone(), a.id));
        app.tab_manager.active_tab_mut().unwrap().request.url = "http://edited".into();
        app.tab_manager.active_tab_mut().unwrap().is_modified = true;
        assert!(app.save_tab(&tabid, None));
        assert_eq!(app.collections[0].request(&zid).unwrap().name, "C");
        assert_eq!(
            app.collections[0].request(&bid).unwrap().url,
            "http://edited"
        );
        app.tab_manager.active_tab_mut().unwrap().pending =
            Some(("execution".into(), Cancellation::default()));
        let sent = app.tab_manager.active_tab().unwrap().request.clone();
        app.tab_manager.open_tab(ApiRequest::new("different"), None);
        let mut sent_env = Environment::new("Sent environment");
        sent_env.variables.push(KeyValue::new("token", "old"));
        let sent_env_id = sent_env.id.clone();
        app.environments.push(sent_env);
        app.environments.push(Environment::new("Other environment"));
        app.active_environment_id = Some(app.environments[1].id.clone());
        app.variables.insert("concurrent".into(), "keep".into());
        app.sender
            .send(AsyncMessage::Response {
                epoch: app.epoch,
                tab_id: tabid.clone(),
                execution: "execution".into(),
                request: Box::new(sent),
                response: Box::new(ApiResponse {
                    status: 201,
                    ..Default::default()
                }),
                variables: Variables {
                    environment: BTreeMap::from([("token".into(), "new".into())]),
                    ..Default::default()
                },
                before: Box::new(Variables {
                    environment: BTreeMap::from([("token".into(), "old".into())]),
                    ..Default::default()
                }),
                environment_id: Some(sent_env_id),
                collection_id: Some(cid.clone()),
            })
            .unwrap();
        app.process_messages();
        assert_eq!(app.environments[0].variables[0].value, "new");
        assert!(app.environments[1].variables.is_empty());
        assert_eq!(app.variables["concurrent"], "keep");
        assert!(app.tab_manager.active_tab().unwrap().response.is_none());
        assert_eq!(
            app.tab_manager.tabs[0].response.as_ref().unwrap().status,
            201
        );
        assert_eq!(storage::load_history().entries[0].request.id, bid);
        app.tab_manager.active_tab_idx = Some(0);
        app.collections[0].file_path = Some(profile.path().to_string_lossy().into());
        app.tab_manager.active_tab_mut().unwrap().is_modified = true;
        assert!(!app.save_tab(&tabid, None));
        assert!(app.tab_manager.active_tab().unwrap().is_modified);
        assert!(app.error.is_some());
        app.persist_drafts();
        let recovered = storage::load_drafts::<TabManager>().unwrap();
        assert!(recovered.tabs[0].is_modified);
        assert!(recovered.tabs[0].response.is_none());
        assert!(recovered.tabs[0].pending.is_none());
        let second = Collection::new("Second");
        let second_id = second.id.clone();
        assert!(app.commit_collection(second));
        app.tab_manager.active_tab_mut().unwrap().collection_id = None;
        assert!(app.save_tab(&tabid, Some((second_id, None))));
        assert!(!app.tab_manager.active_tab().unwrap().is_modified);
    }
}

#[cfg(test)]
#[path = "../tests/unit/app.rs"]
mod interaction_tests;
