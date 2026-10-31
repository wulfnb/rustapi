use super::tool_dialogs as dialogs;
#[cfg(test)]
#[path = "../../tests/unit/protocol_ui.rs"]
mod tests;
use rustapi::{
    http::{Cancellation, HttpService},
    protocols::{Event, Protocol, Session, Summary},
    variables::Variables,
};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        mpsc::{self, Receiver, SyncSender},
        Arc, Mutex,
    },
};
struct Entry {
    event: Event,
    preview: String,
}
pub struct ProtocolConsole {
    pub open: bool,
    profile: Session,
    initial: String,
    outgoing: String,
    events: Arc<Mutex<VecDeque<Entry>>>,
    completion: Option<Receiver<Result<Summary, String>>>,
    commands: Option<SyncSender<String>>,
    cancel: Option<Cancellation>,
    result: String,
}
impl Default for ProtocolConsole {
    fn default() -> Self {
        Self {
            open: false,
            profile: Session::default(),
            initial: String::new(),
            outgoing: String::new(),
            events: Arc::new(Mutex::new(VecDeque::new())),
            completion: None,
            commands: None,
            cancel: None,
            result: String::new(),
        }
    }
}
impl Drop for ProtocolConsole {
    fn drop(&mut self) {
        if let Some(cancel) = &self.cancel {
            cancel.cancel();
        }
    }
}
impl ProtocolConsole {
    pub fn window(
        &mut self,
        ctx: &egui::Context,
        service: Arc<HttpService>,
        secrets: &BTreeMap<String, String>,
    ) {
        if let Some(receiver) = &self.completion {
            if let Ok(result) = receiver.try_recv() {
                self.result = result
                    .map(|summary| {
                        format!(
                            "{} · {} events · {} bytes",
                            summary.reason, summary.events, summary.bytes
                        )
                    })
                    .unwrap_or_else(|error| error);
                self.cancel = None;
                self.commands = None;
                self.completion = None;
            }
        }
        if !self.open {
            return;
        }
        egui::Window::new("Live protocol console").default_size([820.,640.]).vscroll(true).show(ctx,|ui| {
            ui.horizontal(|ui|{for (kind,label) in [(Protocol::Sse,"SSE"),(Protocol::WebSocket,"WebSocket"),(Protocol::Mqtt,"MQTT"),(Protocol::SocketIo,"Socket.IO"),(Protocol::Grpc,"gRPC")]{ui.selectable_value(&mut self.profile.protocol,kind,label);}});
            ui.horizontal(|ui|{ui.label("Endpoint");ui.text_edit_singleline(&mut self.profile.request.url);});
            ui.horizontal(|ui|{ui.label("Session seconds");ui.add(egui::DragValue::new(&mut self.profile.duration_seconds).range(1..=3600));ui.label("Event cap");ui.add(egui::DragValue::new(&mut self.profile.max_events).range(1..=10000));});
            match self.profile.protocol {
                Protocol::Mqtt=>{ui.small("mqtt(s)://host:port · MQTT 3.1.1");for(label,field,secret)in[("Topic",&mut self.profile.topic,false),("Client ID",&mut self.profile.mqtt_client_id,false),("Username",&mut self.profile.mqtt_username,false),("Password / vault reference",&mut self.profile.mqtt_password,true)]{ui.label(label);ui.add(egui::TextEdit::singleline(field).password(secret));}ui.horizontal(|ui|{ui.checkbox(&mut self.profile.subscribe,"Subscribe");ui.checkbox(&mut self.profile.retain,"Retain");ui.label("QoS");ui.add(egui::DragValue::new(&mut self.profile.qos).range(0..=2));});},
                Protocol::SocketIo=>{ui.small("Engine.IO v4 over WebSocket; JSON events. Binary events and acknowledgements are unsupported.");ui.label("Event");ui.text_edit_singleline(&mut self.profile.socket_event);ui.label("Namespace");ui.text_edit_singleline(&mut self.profile.socket_namespace);ui.label("Connection auth JSON");ui.add(egui::TextEdit::multiline(&mut self.profile.socket_auth).password(true).desired_rows(2));},
                Protocol::Grpc=>{ui.small("Requires grpcurl on PATH or RUSTAPI_GRPCURL. Reflection, descriptor sets, unary and server streams.");egui::ComboBox::from_id_salt("grpc_action").selected_text(&self.profile.grpc_action).show_ui(ui,|ui|{for action in ["list","describe","invoke"]{ui.selectable_value(&mut self.profile.grpc_action,action.into(),action);}});ui.label("Service/method or symbol");ui.text_edit_singleline(&mut self.profile.grpc_method);ui.label("Descriptor set path (optional)");ui.text_edit_singleline(&mut self.profile.grpc_descriptor);},
                Protocol::Sse=>{ui.small("HTTP(S) event stream. Events appear while connected.");let mut method=self.profile.request.method.as_str().to_owned();ui.label("HTTP method");if ui.text_edit_singleline(&mut method).changed(){self.profile.request.method=method.into();}},
                Protocol::WebSocket=>{ui.small("ws:// or wss://; text/binary frames and live sends.");}
            }
            if matches!(self.profile.protocol,Protocol::Grpc|Protocol::Sse){ui.label("Request body");ui.add(egui::TextEdit::multiline(&mut self.profile.request.body).code_editor().desired_rows(4));}else{ui.label("Initial message (optional)");ui.add(egui::TextEdit::multiline(&mut self.initial).code_editor().desired_rows(3));}
            egui::CollapsingHeader::new("Protocol authentication and headers").show(ui,|ui|{super::render_auth_editor(ui,&mut self.profile.request.auth);super::render_key_value_editor(ui,&mut self.profile.request.headers);});
            ui.horizontal(|ui|{
                if ui.button("Load protocol profile").clicked(){if let Some(path)=dialogs::open(){match rustapi::tools_cli::read(&path,8*1024*1024).and_then(|text|Ok(serde_json::from_str::<Session>(&text)?)){Ok(profile)=>{self.initial=profile.messages.first().cloned().unwrap_or_default();self.profile=profile;},Err(error)=>self.result=error.to_string()}}}
                if ui.button("Save protocol profile").clicked(){self.profile.messages=if self.initial.is_empty(){vec![]}else{vec![self.initial.clone()]};if let Some(path)=dialogs::save(){self.result=serde_json::to_vec_pretty(&self.profile).map_err(anyhow::Error::from).and_then(|bytes|rustapi::workspace::atomic_write(&path,&bytes)).map(|_|"Protocol profile saved".into()).unwrap_or_else(|error|error.to_string());}}
                if ui.add_enabled(self.cancel.is_none(),egui::Button::new("Connect protocol")).clicked(){
                    self.profile.messages=if self.initial.is_empty(){vec![]}else{vec![self.initial.clone()]};let profile=self.profile.clone();let service=service.clone();
                    let globals=crate::storage::GlobalVariables::load(&crate::storage::get_app_data_dir()).map(|g|g.variables).unwrap_or_default();let variables=Variables{global:globals,secrets:secrets.clone(),..Default::default()};
                    let cancel=Cancellation::default();self.cancel=Some(cancel.clone());let events=self.events.clone();events.lock().unwrap_or_else(|p|p.into_inner()).clear();let (sender,receiver)=mpsc::sync_channel(1);self.completion=Some(receiver);let(command_sender,command_receiver)=mpsc::sync_channel(16);self.commands=Some(command_sender);let context=ctx.clone();self.result="Connecting…".into();
                    std::thread::spawn(move||{let result=rustapi::protocols::run(&service,&profile,&variables,&cancel,Some(&command_receiver),|event|{let preview=rustapi::utils::truncate_str(&event.data.to_string(),4096);let mut queue=events.lock().unwrap_or_else(|p|p.into_inner());if queue.len()>=1000{queue.pop_front();}queue.push_back(Entry{event,preview});context.request_repaint();}).map_err(|error|error.to_string());let _=sender.send(result);context.request_repaint();});
                }
                if let Some(cancel)=&self.cancel{if ui.button("Disconnect protocol").clicked(){cancel.cancel();}}
            });
            ui.small("Profiles store configuration. Use {{vault:name}} for credentials. Received events stay in memory.");
            if let Some(commands)=&self.commands{if matches!(self.profile.protocol,Protocol::WebSocket|Protocol::SocketIo|Protocol::Mqtt){ui.horizontal(|ui|{ui.text_edit_singleline(&mut self.outgoing);if ui.button("Send live message").clicked(){if commands.try_send(self.outgoing.clone()).is_err(){self.result="Outgoing queue is full or disconnected".into();}else{self.outgoing.clear();}}});}}
            ui.label(&self.result);ui.separator();let events=self.events.lock().unwrap_or_else(|p|p.into_inner());for(index,entry)in events.iter().enumerate().rev().take(100){ui.push_id(index,|ui|{ui.horizontal(|ui|{ui.label(format!("{} ms · {}",entry.event.elapsed_ms,entry.event.kind));if ui.small_button("Copy event").clicked(){ui.ctx().copy_text(entry.event.data.to_string());}});ui.monospace(&entry.preview);});}
            if ui.button("Close protocol console").clicked(){self.open=false;}
        });
        if self.cancel.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}
