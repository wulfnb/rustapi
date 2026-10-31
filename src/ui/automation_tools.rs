use super::tool_dialogs as dialogs;
#[cfg(test)]
#[path = "../../tests/unit/automation_ui.rs"]
mod tests;
use crate::models::Collection;
use rustapi::{
    automation::{self, RunProfile, RunRecord},
    http::{Cancellation, HttpService},
    performance::LoadProfile,
};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
enum Event {
    Finished(Result<String, String>),
    Message(String),
    SchedulerStopped(Result<(), String>),
}
#[derive(Clone, Default)]
struct Mailbox(Arc<Mutex<std::collections::VecDeque<Event>>>);
impl Mailbox {
    fn send(&self, event: Event) -> Result<(), ()> {
        let mut queue = self.0.lock().unwrap_or_else(|p| p.into_inner());
        if queue.len() >= 64 {
            let index = queue
                .iter()
                .position(|e| matches!(e, Event::Message(_)))
                .unwrap_or(0);
            queue.remove(index);
        }
        queue.push_back(event);
        Ok(())
    }
    fn try_recv(&self) -> Result<Event, ()> {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .pop_front()
            .ok_or(())
    }
}
pub struct AutomationTools {
    form: RunProfile,
    profiles: Vec<RunProfile>,
    history: Vec<RunRecord>,
    before: String,
    after: String,
    load: LoadProfile,
    result: String,
    sender: Mailbox,
    receiver: Mailbox,
    job: Option<Cancellation>,
    scheduler: Option<Cancellation>,
}
impl Default for AutomationTools {
    fn default() -> Self {
        let sender = Mailbox::default();
        let receiver = sender.clone();
        Self {
            form: RunProfile::default(),
            profiles: vec![],
            history: vec![],
            before: String::new(),
            after: String::new(),
            load: LoadProfile::default(),
            result: String::new(),
            sender,
            receiver,
            job: None,
            scheduler: None,
        }
    }
}
impl Drop for AutomationTools {
    fn drop(&mut self) {
        if let Some(cancel) = &self.job {
            cancel.cancel();
        }
        if let Some(cancel) = &self.scheduler {
            cancel.cancel();
        }
    }
}
impl AutomationTools {
    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        collections: &[Collection],
        service: Arc<HttpService>,
        secrets: &BTreeMap<String, String>,
    ) {
        while let Ok(event) = self.receiver.try_recv() {
            match event {
                Event::Finished(result) => {
                    self.job = None;
                    self.result = result.unwrap_or_else(|e| e);
                }
                Event::Message(message) => self.result = message,
                Event::SchedulerStopped(result) => {
                    self.scheduler = None;
                    if let Err(error) = result {
                        self.result = error;
                    }
                }
            }
        }
        let root = crate::storage::get_app_data_dir();
        egui::CollapsingHeader::new("Saved runs, datasets and scheduling").show(ui,|ui| {
            if ui.button("Refresh saved run profiles").clicked(){match automation::profiles(&root){Ok(profiles)=>self.profiles=profiles,Err(error)=>self.result=error.to_string()}}
            egui::ComboBox::from_id_salt("saved_profile").selected_text(&self.form.name).show_ui(ui,|ui|{for profile in &self.profiles{if ui.selectable_label(profile.id==self.form.id,&profile.name).clicked(){self.form=profile.clone();}}});
            ui.horizontal(|ui|{ui.label("Profile name");ui.text_edit_singleline(&mut self.form.name);if ui.button("New run profile").clicked(){self.form=RunProfile::default();}});
            ui.label("Choose a saved collection");
            for collection in collections {
                if ui.button(&collection.name).clicked(){let path=collection.file_path.as_ref().map(std::path::PathBuf::from).unwrap_or_else(||root.join("collections").join(format!("{}.json",collection.id)));match root.canonicalize().and_then(|base|path.canonicalize().map(|path|(base,path))){Ok((base,path))=>match path.strip_prefix(base){Ok(relative)=>self.form.collection=relative.to_string_lossy().replace('\\',"/"),Err(_)=>self.result="Import/save the collection inside this workspace before creating a run profile.".into()},Err(error)=>self.result=error.to_string()}}
            }
            ui.horizontal(|ui|{ui.label("Collection path");ui.text_edit_singleline(&mut self.form.collection);});
            ui.horizontal(|ui|{ui.label("Iterations (0 = dataset size)");ui.add(egui::DragValue::new(&mut self.form.options.iterations).range(0..=10000));ui.label("Delay ms");ui.add(egui::DragValue::new(&mut self.form.options.delay_ms).range(0..=60000));});
            ui.checkbox(&mut self.form.options.stop_on_failure,"Stop on failed assertion");
            let mut enabled=self.form.environment.is_some();if ui.checkbox(&mut enabled,"Environment file").changed(){self.form.environment=enabled.then(String::new);}
            if let Some(path)=&mut self.form.environment{ui.text_edit_singleline(path);}
            let mut enabled=self.form.dataset.is_some();if ui.checkbox(&mut enabled,"Managed dataset").changed(){self.form.dataset=enabled.then(String::new);}
            if let Some(path)=&mut self.form.dataset{ui.text_edit_singleline(path);}
            if ui.button("Import CSV / JSON dataset").clicked(){if let Some(source)=dialogs::open(){let name=source.file_name().and_then(|n|n.to_str()).unwrap_or("dataset.json");match automation::import_dataset(&root,&source,name){Ok(_)=>self.form.dataset=Some(format!("datasets/{name}")),Err(error)=>self.result=error.to_string()}}}
            ui.small("Input paths are relative to this workspace. Use vault references for credentials.");
            let mut enabled=self.form.interval_seconds.is_some();if ui.checkbox(&mut enabled,"Schedule while scheduler is running").changed(){self.form.interval_seconds=enabled.then_some(3600);}
            if let Some(interval)=&mut self.form.interval_seconds{ui.horizontal(|ui|{ui.label("Interval seconds");ui.add(egui::DragValue::new(interval).range(60..=2592000));});}
            let mut enabled=self.form.alert_url_env.is_some();if ui.checkbox(&mut enabled,"Failure webhook").changed(){self.form.alert_url_env=enabled.then(||"RUSTAPI_ALERT_URL".into());}
            if let Some(name)=&mut self.form.alert_url_env{ui.label("Environment variable containing webhook URL");ui.text_edit_singleline(name);}
            ui.horizontal(|ui| {
                if ui.button("Save run profile").clicked(){self.result=automation::save(&root,&self.form).map(|path|format!("Profile saved: {}",path.display())).unwrap_or_else(|error|error.to_string());}
                if ui.add_enabled(self.job.is_none(),egui::Button::new("Run profile now")).clicked(){
                    let profile=self.form.clone();let root=root.clone();let service=service.clone();let secrets=secrets.clone();let sender=self.sender.clone();let cancel=Cancellation::default();self.job=Some(cancel.clone());
                    std::thread::spawn(move||{let result=automation::save(&root,&profile).and_then(|_|automation::run(&root,&service,&profile,secrets,&cancel)).and_then(|record|Ok(serde_json::to_string_pretty(&record)?)).map_err(|error|error.to_string());let _=sender.send(Event::Finished(result));});
                }
                if self.scheduler.is_none()&&ui.button("Start workspace scheduler").clicked(){let root=root.clone();let service=service.clone();let secrets=secrets.clone();let sender=self.sender.clone();let cancel=Cancellation::default();self.scheduler=Some(cancel.clone());std::thread::spawn(move||{let result=automation::watch(&root,&service,secrets,&cancel,|record|{let message=record.map(|r|format!("Scheduled run {}: {}",r.profile_name,if r.report.passed(){"passed"}else{"failed"})).unwrap_or_else(|_|"Scheduled run failed; check inputs and vault configuration".into());let _=sender.send(Event::Message(message));}).map_err(|error|error.to_string());let _=sender.send(Event::SchedulerStopped(result));});}
                if let Some(cancel)=&self.scheduler {if ui.button("Stop scheduler").clicked(){cancel.cancel();}}
            });
            if self.scheduler.is_some(){ui.label("Scheduler active. Runs are sequential. Closing the app stops it.");}
            if ui.button("Refresh run history").clicked(){match automation::history(&root){Ok(history)=>self.history=history,Err(error)=>self.result=error.to_string()}}
            for(label,id)in[("Before run",&mut self.before),("After run",&mut self.after)] {egui::ComboBox::from_id_salt(label).selected_text(if id.is_empty(){label}else{id.as_str()}).show_ui(ui,|ui|{for record in &self.history{ui.selectable_value(id,record.id.clone(),format!("{} · {} · {}",record.started_at,record.profile_name,if record.report.passed(){"passed"}else{"failed"}));}});}
            if ui.button("Compare run results").clicked(){if let(Some(before),Some(after))=(self.history.iter().find(|r|r.id==self.before),self.history.iter().find(|r|r.id==self.after)){self.result=serde_json::to_string_pretty(&automation::compare(before,after)).unwrap_or_default();}}
        });
        egui::CollapsingHeader::new("Performance testing").show(ui,|ui| {
            if ui.button("Use selected run profile for load").clicked(){self.load.run=self.form.clone();}
            ui.label(format!("Run: {}",self.load.run.name));
            ui.horizontal(|ui|{ui.label("Virtual users");ui.add(egui::DragValue::new(&mut self.load.users).range(1..=64));ui.label("Duration seconds");ui.add(egui::DragValue::new(&mut self.load.duration_seconds).range(1..=3600));});
            ui.horizontal(|ui|{ui.label("Ramp seconds");ui.add(egui::DragValue::new(&mut self.load.ramp_seconds).range(0..=3600));ui.label("Pacing ms");ui.add(egui::DragValue::new(&mut self.load.pacing_ms).range(0..=60000));ui.label("Request cap");ui.add(egui::DragValue::new(&mut self.load.max_requests).range(1..=100000));});
            ui.horizontal(|ui|{ui.label("Maximum error fraction");ui.add(egui::DragValue::new(&mut self.load.max_error_rate).range(0.0..=1.0).speed(0.01));});
            let mut p95=self.load.max_p95_ms.is_some();if ui.checkbox(&mut p95,"p95 latency threshold").changed(){self.load.max_p95_ms=p95.then_some(1000);}
            if let Some(limit)=&mut self.load.max_p95_ms{ui.add(egui::DragValue::new(limit).range(1..=3600000));}
            if ui.button("Add load stage").clicked(){self.load.stages.push(rustapi::performance::Stage{users:self.load.users,duration_seconds:10});}
            let mut remove=None;for(index,stage)in self.load.stages.iter_mut().enumerate(){ui.horizontal(|ui|{ui.label(format!("Stage {} users / seconds",index+1));ui.add(egui::DragValue::new(&mut stage.users).range(1..=64));ui.add(egui::DragValue::new(&mut stage.duration_seconds).range(1..=3600));if ui.small_button("Remove stage").clicked(){remove=Some(index);}});}
            if let Some(index)=remove{self.load.stages.remove(index);}
            ui.small("Stages replace constant/ramp timing. Users have isolated cookie/token sessions and use saved assertions/datasets.");
            ui.horizontal(|ui| {
                if ui.button("Save load profile").clicked(){if let Some(path)=dialogs::save(){self.result=self.load.validate().and_then(|_|rustapi::workspace::atomic_write(&path,&serde_json::to_vec_pretty(&self.load)?)).map(|_|"Load profile saved".into()).unwrap_or_else(|error|error.to_string());}}
                if ui.button("Load performance profile").clicked(){if let Some(path)=dialogs::open(){match rustapi::tools_cli::read(&path,8*1024*1024).and_then(|text|Ok(serde_json::from_str(&text)?)){Ok(profile)=>self.load=profile,Err(error)=>self.result=error.to_string()}}}
                if ui.add_enabled(self.job.is_none(),egui::Button::new("Start load test")).clicked(){let profile=self.load.clone();let root=root.clone();let secrets=secrets.clone();let sender=self.sender.clone();let cancel=Cancellation::default();self.job=Some(cancel.clone());std::thread::spawn(move||{let result=rustapi::performance::run(&root,&profile,secrets,&cancel).and_then(|report|Ok(serde_json::to_string_pretty(&report)?)).map_err(|error|error.to_string());let _=sender.send(Event::Finished(result));});}
            });
        });
        if let Some(cancel) = &self.job {
            ui.label("Run in progress…");
            if ui.button("Cancel active run").clicked() {
                cancel.cancel();
            }
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
        }
        if self.scheduler.is_some() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs(1));
        }
        if !self.result.is_empty() {
            ui.label(&self.result);
        }
    }
}
