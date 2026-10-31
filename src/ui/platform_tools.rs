use super::tool_dialogs as dialogs;
#[cfg(test)]
#[path = "../../tests/unit/platform_ui.rs"]
mod tests;
use crate::models::{ApiRequest, Collection};
use rustapi::{
    http::{Cancellation, HttpService},
    platform::{
        ai,
        collaboration::{self, CatalogEntry, Config, Role, TeamServer, User},
        external_secrets,
    },
    policy::Policy,
};
use std::{
    collections::BTreeMap,
    sync::{
        mpsc::{self, Receiver},
        Arc,
    },
};
pub enum PlatformAction {
    Import(Box<Collection>),
    Draft(Box<ApiRequest>),
    Secrets(BTreeMap<String, String>),
    ExternalSecrets(BTreeMap<String, String>),
    Linked {
        local_id: String,
        remote_id: String,
        revision: String,
    },
}
impl PlatformAction {
    fn import(collection: Collection) -> Self {
        Self::Import(Box::new(collection))
    }
    fn draft(request: ApiRequest) -> Self {
        Self::Draft(Box::new(request))
    }
}

enum Item {
    Catalog(Vec<CatalogEntry>),
    Action(PlatformAction),
    Models(Vec<String>),
}
pub struct PlatformTools {
    pub open: bool,
    endpoint: String,
    token_key: String,
    catalog: Vec<CatalogEntry>,
    selected_remote: String,
    selected_local: String,
    result: String,
    receiver: Option<Receiver<Result<Item, String>>>,
    cancel: Option<Cancellation>,
    server: Option<TeamServer>,
    config: Config,
    config_path: Option<std::path::PathBuf>,
    new_token: zeroize::Zeroizing<String>,
    user_name: String,
    user_role: Role,
    policy: Policy,
    hosts: String,
    ai_endpoint: String,
    model: String,
    models: Vec<String>,
    prompt: String,
    context: bool,
}
impl Default for PlatformTools {
    fn default() -> Self {
        Self {
            open: false,
            endpoint: "http://127.0.0.1:9090".into(),
            token_key: "team_token".into(),
            catalog: vec![],
            selected_remote: String::new(),
            selected_local: String::new(),
            result: String::new(),
            receiver: None,
            cancel: None,
            server: None,
            config: Config::default(),
            config_path: None,
            new_token: zeroize::Zeroizing::new(String::new()),
            user_name: "administrator".into(),
            user_role: Role::Admin,
            policy: Policy::default(),
            hosts: String::new(),
            ai_endpoint: "http://127.0.0.1:11434".into(),
            model: String::new(),
            models: vec![],
            prompt: String::new(),
            context: false,
        }
    }
}
impl Drop for PlatformTools {
    fn drop(&mut self) {
        self.cancel();
    }
}
impl PlatformTools {
    pub fn cancel(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            cancel.cancel();
        }
        self.receiver = None;
    }
    fn start(
        &mut self,
        ctx: &egui::Context,
        work: impl FnOnce(Cancellation) -> anyhow::Result<Item> + Send + 'static,
    ) {
        let cancel = Cancellation::default();
        self.cancel = Some(cancel.clone());
        let (sender, receiver) = mpsc::sync_channel(1);
        self.receiver = Some(receiver);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = work(cancel).map_err(|error| error.to_string());
            let _ = sender.send(result);
            ctx.request_repaint();
        });
    }
    pub fn window(
        &mut self,
        ctx: &egui::Context,
        collections: &[Collection],
        service: Arc<HttpService>,
        secrets: &BTreeMap<String, String>,
    ) -> Option<PlatformAction> {
        let mut action = None;
        if let Some(receiver) = &self.receiver {
            if let Ok(result) = receiver.try_recv() {
                self.cancel = None;
                self.receiver = None;
                match result {
                    Ok(Item::Catalog(catalog)) => {
                        self.catalog = catalog;
                        self.result = "Catalog refreshed".into();
                    }
                    Ok(Item::Models(models)) => self.models = models,
                    Ok(Item::Action(value)) => {
                        action = Some(value);
                        self.result = "Completed".into();
                    }
                    Err(error) => self.result = error,
                }
            }
        }
        if !self.open {
            return action;
        }
        let root = crate::storage::get_app_data_dir();
        egui::Window::new("Optional self-hosted tools").default_size([820.,650.]).vscroll(true).show(ctx,|ui|{
            ui.label("Local use remains account-free. Server connections and model requests happen only when selected below.");
            if self.selected_local.is_empty(){if let Some(collection)=collections.first(){self.selected_local=collection.id.clone();}}
            egui::ComboBox::from_id_salt("platform_collection").selected_text(collections.iter().find(|c|c.id==self.selected_local).map(|c|c.name.as_str()).unwrap_or("Select local collection")).show_ui(ui,|ui|{for collection in collections{ui.selectable_value(&mut self.selected_local,collection.id.clone(),&collection.name);}});
            egui::CollapsingHeader::new("Team collaboration and API catalog").show(ui,|ui|{
                ui.label("Team server URL");ui.text_edit_singleline(&mut self.endpoint);ui.label("Unlocked vault key containing your team token");ui.text_edit_singleline(&mut self.token_key);let token=secrets.get(&self.token_key).cloned();
                ui.horizontal(|ui|{
                    if ui.add_enabled(self.cancel.is_none(),egui::Button::new("Refresh team catalog")).clicked(){let endpoint=self.endpoint.clone();let root=root.clone();let token=token.clone();self.start(ctx,move |_|Ok(Item::Catalog(collaboration::Client::for_workspace(&root,&endpoint,&token.ok_or_else(||anyhow::anyhow!("Unlock the team token in Local encrypted secrets"))?)?.catalog()?)));}
                    if ui.add_enabled(self.cancel.is_none(),egui::Button::new("Push selected collection")).clicked(){if let Some(collection)=collections.iter().find(|c|c.id==self.selected_local){let mut collection=collection.clone();let local_id=collection.id.clone();let remote_id=collection.imported_settings.get("team.remote_id").and_then(serde_json::Value::as_str).unwrap_or(&collection.id).to_owned();let revision=collection.imported_settings.get("team.revision").and_then(serde_json::Value::as_str).map(String::from).unwrap_or_else(||"new".into());collection.id=remote_id.clone();let endpoint=self.endpoint.clone();let root=root.clone();let token=token.clone();self.start(ctx,move |_|{let client=collaboration::Client::for_workspace(&root,&endpoint,&token.ok_or_else(||anyhow::anyhow!("Team token is locked/missing"))?)?;let revision=client.push(&collection,&revision)?;Ok(Item::Action(PlatformAction::Linked{local_id,remote_id,revision}))});}}
                });
                ui.small("Push redacts literal credential fields and preserves vault references. A conflicting revision must be pulled and reviewed before retrying.");
                for entry in &self.catalog{if ui.selectable_label(self.selected_remote==entry.id,format!("{} · {} · {} requests · {}",entry.name,entry.version,entry.requests,entry.updated_at)).clicked(){self.selected_remote=entry.id.clone();}}
                if ui.add_enabled(self.cancel.is_none()&&!self.selected_remote.is_empty(),egui::Button::new("Import remote as local copy")).clicked(){let endpoint=self.endpoint.clone();let id=self.selected_remote.clone();let root=root.clone();let token=token.clone();self.start(ctx,move |_|{let revision=collaboration::Client::for_workspace(&root,&endpoint,&token.ok_or_else(||anyhow::anyhow!("Team token is locked/missing"))?)?.pull(&id)?;let mut collection=revision.collection;collection.id=uuid::Uuid::new_v4().to_string();collection.file_path=None;collection.imported_settings.insert("team.remote_id".into(),id.into());collection.imported_settings.insert("team.revision".into(),revision.revision.into());Ok(Item::Action(PlatformAction::import(collection)))});}
                if !self.selected_remote.is_empty()&&ui.button("Copy published documentation URL").clicked(){ui.ctx().copy_text(format!("{}/v1/docs/{}",self.endpoint.trim_end_matches('/'),self.selected_remote));}
            });
            egui::CollapsingHeader::new("Host a team server").show(ui,|ui|{
                if let Some(path)=&self.config_path{ui.label(path.display().to_string());}
                ui.small("Binds to 127.0.0.1. Use a TLS reverse proxy for remote access. User/role changes require a server restart.");
                ui.add_enabled_ui(self.server.is_none(),|ui|{
                    ui.horizontal(|ui|{if ui.button("Load server configuration").clicked(){if let Some(path)=dialogs::open(){match rustapi::tools_cli::read(&path,1024*1024).and_then(|text|Ok(serde_json::from_str::<Config>(&text)?)).and_then(|config|{config.validate()?;Ok(config)}){Ok(config)=>{self.config=config;self.config_path=Some(path);},Err(error)=>self.result=error.to_string()}}}
if ui.button("Save server configuration").clicked(){if let Some(path)=dialogs::save(){self.result=self.config.validate().and_then(|_|rustapi::workspace::atomic_write(&path,&serde_json::to_vec_pretty(&self.config)?)).map(|_|{self.config_path=Some(path);"Server configuration saved".into()}).unwrap_or_else(|error|error.to_string());}}});
                    ui.horizontal(|ui|{ui.label("Port (0 = random)");ui.add(egui::DragValue::new(&mut self.config.port));ui.checkbox(&mut self.config.public_documentation,"Publish documentation without authentication");});
                    ui.text_edit_singleline(&mut self.user_name);egui::ComboBox::from_id_salt("server_role").selected_text(format!("{:?}",self.user_role)).show_ui(ui,|ui|{for role in [Role::Admin,Role::Editor,Role::Viewer]{ui.selectable_value(&mut self.user_role,role,format!("{role:?}"));}});
                    if ui.button("Create 30-day token user").clicked(){match User::issue(self.user_name.clone(),self.user_role,30){Ok((user,token))=>{self.config.users.push(user);self.new_token=zeroize::Zeroizing::new(token);},Err(error)=>self.result=error.to_string()}}
                    let mut remove=None;for(index,user)in self.config.users.iter().enumerate(){ui.horizontal(|ui|{ui.label(format!("{} · {:?} · expires {:?}",user.name,user.role,user.expires_at));if ui.small_button("Revoke user").clicked(){remove=Some(index);}});}
if let Some(index)=remove{self.config.users.remove(index);}
                });
                if !self.new_token.is_empty(){ui.label("New token (copy before closing; only its hash is saved)");ui.add(egui::TextEdit::singleline(&mut *self.new_token).password(true));if ui.button("Copy new team token").clicked(){ui.ctx().copy_text(self.new_token.to_string());}
if ui.button("Use new token for this session").clicked(){action=Some(PlatformAction::Secrets(BTreeMap::from([(self.token_key.clone(),self.new_token.to_string())])));}}
                if self.server.is_none()&&ui.button("Start self-hosted server").clicked(){let path=crate::storage::owned_path(&root,&[".local","team-data"]);match path.and_then(|path|TeamServer::start(&path,self.config.clone())){Ok(server)=>self.server=Some(server),Err(error)=>self.result=error.to_string()}}
                if let Some(server)=&self.server{ui.label(format!("Listening on http://{}",server.server.address));if ui.button("Use this server URL").clicked(){self.endpoint=format!("http://{}",server.server.address);}
if ui.button("Stop team server").clicked(){self.server=None;}}
            });
            egui::CollapsingHeader::new("AI request drafting (Ollama)").show(ui,|ui|{
                ui.label("Model server URL");ui.text_edit_singleline(&mut self.ai_endpoint);
                if ui.add_enabled(self.cancel.is_none(),egui::Button::new("List local models")).clicked(){let endpoint=self.ai_endpoint.clone();let service=service.clone();self.start(ctx,move|cancel|Ok(Item::Models(ai::models(&service,&endpoint,&cancel)?)));}
                ui.text_edit_singleline(&mut self.model);egui::ComboBox::from_id_salt("ai_model").selected_text(&self.model).show_ui(ui,|ui|{for model in &self.models{ui.selectable_value(&mut self.model,model.clone(),model);}});
                ui.label("Describe the request");ui.add(egui::TextEdit::multiline(&mut self.prompt).desired_rows(4));ui.checkbox(&mut self.context,"Include selected collection as redacted reference data");ui.small("Sends the prompt/context to this endpoint. The generated request opens as an unsent draft; generated scripts and transport overrides are discarded.");
                if ui.add_enabled(self.cancel.is_none(),egui::Button::new("Generate request draft")).clicked(){let endpoint=self.ai_endpoint.clone();let model=self.model.clone();let prompt=self.prompt.clone();let collection=if self.context{collections.iter().find(|c|c.id==self.selected_local).cloned()}else{None};let service=service.clone();self.start(ctx,move|cancel|Ok(Item::Action(PlatformAction::draft(ai::draft(&service,&endpoint,&model,&prompt,collection.as_ref(),&cancel)?))));}
            });
            egui::CollapsingHeader::new("External secrets and MCP tools").show(ui,|ui|{
                ui.label("Configure secret-providers.json with Vault KV v2 or AWS Secrets Manager bindings. Values resolve as {{vault:ext/alias}} and remain in memory.");
                if ui.add_enabled(self.cancel.is_none(),egui::Button::new("Refresh external secrets")).clicked(){let root=root.clone();let service=service.clone();self.start(ctx,move|cancel|Ok(Item::Action(PlatformAction::ExternalSecrets(external_secrets::load(&root,&service,&cancel)?))));}
                ui.label("MCP stdio command: rustapi mcp serve --workspace <directory>. Add --allow-execution only when the MCP client may run saved profiles.");
            });
            egui::CollapsingHeader::new("Workspace execution policy").show(ui,|ui|{
                if ui.button("Load workspace policy").clicked(){match Policy::load(&root){Ok(policy)=>{self.hosts=policy.allowed_hosts.join("\n");self.policy=policy;},Err(error)=>self.result=error.to_string()}}
                ui.checkbox(&mut self.policy.require_encryption,"Require encrypted transports");ui.checkbox(&mut self.policy.allow_insecure_tls,"Allow certificate verification overrides");ui.checkbox(&mut self.policy.allow_scripts,"Allow scripts");ui.checkbox(&mut self.policy.allow_external_secrets,"Allow external secret managers");ui.checkbox(&mut self.policy.allow_ai,"Allow AI endpoint calls");ui.checkbox(&mut self.policy.allow_mcp_execution,"Allow explicitly enabled MCP execution");ui.label("Allowed destination hosts, one per line (empty = unrestricted; *.example.test supported)");ui.add(egui::TextEdit::multiline(&mut self.hosts).desired_rows(3));
                if ui.button("Save and apply workspace policy").clicked(){self.policy.allowed_hosts=self.hosts.lines().map(str::trim).filter(|s|!s.is_empty()).map(String::from).collect();self.result=self.policy.validate().and_then(|_|{let path=crate::storage::owned_path(&root,&["workspace-policy.json"])?;rustapi::workspace::atomic_write(&path,&serde_json::to_vec_pretty(&self.policy)?)?;service.reload_policy(&root)}).map(|_|"Policy applied to new requests".into()).unwrap_or_else(|error|error.to_string());}
            });
            if self.cancel.is_some(){ui.label("Operation in progress…");if ui.button("Cancel platform operation").clicked(){self.cancel();}}
            ui.label(&self.result);if ui.button("Close self-hosted tools").clicked(){self.open=false;}
        });
        if self.cancel.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        action
    }
}
