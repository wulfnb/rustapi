use super::tool_dialogs as dialogs;
#[cfg(test)]
#[path = "../../tests/unit/flow_ui.rs"]
mod tests;
use crate::models::Collection;
use rustapi::{
    automation::{self, RunProfile},
    flows::{Flow, Node},
    http::{Cancellation, HttpService},
};
use std::{
    collections::BTreeMap,
    sync::{
        mpsc::{self, Receiver},
        Arc, Mutex,
    },
};
pub struct FlowEditor {
    pub open: bool,
    flow: Flow,
    selected: String,
    requests: Vec<(String, String)>,
    profiles: Vec<RunProfile>,
    result: String,
    cancel: Option<Cancellation>,
    receiver: Option<Receiver<Result<String, String>>>,
    last: Arc<Mutex<Option<String>>>,
}
impl Default for FlowEditor {
    fn default() -> Self {
        Self {
            open: false,
            flow: Flow::default(),
            selected: String::new(),
            requests: vec![],
            profiles: vec![],
            result: String::new(),
            cancel: None,
            receiver: None,
            last: Arc::new(Mutex::new(None)),
        }
    }
}
impl Drop for FlowEditor {
    fn drop(&mut self) {
        if let Some(cancel) = &self.cancel {
            cancel.cancel();
        }
    }
}
impl FlowEditor {
    pub fn window(
        &mut self,
        ctx: &egui::Context,
        collections: &[Collection],
        service: Arc<HttpService>,
        secrets: &BTreeMap<String, String>,
    ) {
        if let Some(receiver) = &self.receiver {
            if let Ok(result) = receiver.try_recv() {
                self.result = result.unwrap_or_else(|error| error);
                self.cancel = None;
                self.receiver = None;
            }
        }
        if !self.open {
            return;
        }
        let root = crate::storage::get_app_data_dir();
        egui::Window::new("Visual request flows").default_size([850.,650.]).vscroll(true).show(ctx,|ui| {
            ui.horizontal(|ui|{ui.label("Flow name");ui.text_edit_singleline(&mut self.flow.name);});
            egui::CollapsingHeader::new("Build from a saved collection").show(ui,|ui|{for collection in collections{if ui.button(&collection.name).clicked(){let path=collection.file_path.as_ref().map(std::path::PathBuf::from).unwrap_or_else(||root.join("collections").join(format!("{}.json",collection.id)));match root.canonicalize().and_then(|base|path.canonicalize().map(|path|(base,path))){Ok((base,path))=>{if let Ok(path)=path.strip_prefix(base){let requests=collection.ordered_requests(None);self.requests=requests.iter().map(|r|(r.id.clone(),r.name.clone())).collect();let mut nodes:Vec<Node>=requests.iter().enumerate().map(|(index,r)|Node{id:uuid::Uuid::new_v4().to_string(),request_id:r.id.clone(),success:None,failure:None,position:[30.+(index%3)as f32*230.,30.+(index/3)as f32*110.]}).collect();for index in 0..nodes.len().saturating_sub(1){nodes[index].success=Some(nodes[index+1].id.clone());}self.flow=Flow{profile:RunProfile{collection:path.to_string_lossy().replace('\\',"/"),..Default::default()},start:nodes.first().map(|n|n.id.clone()).unwrap_or_default(),nodes,..Default::default()};}else{self.result="Save/import the collection inside this workspace first.".into();}},Err(error)=>self.result=error.to_string()}}}});
            ui.horizontal(|ui|{if ui.button("Load flow").clicked(){if let Some(path)=dialogs::open(){match rustapi::tools_cli::read(&path,8*1024*1024).and_then(|text|Ok(serde_json::from_str::<Flow>(&text)?)).and_then(|flow|{flow.validate()?;Ok(flow)}){Ok(flow)=>{self.flow=flow;self.reload_requests(&root);},Err(error)=>self.result=error.to_string()}}}
if ui.button("Save flow").clicked(){self.result=self.flow.save(&root).map(|p|format!("Flow saved: {}",p.display())).unwrap_or_else(|error|error.to_string());}});
            if ui.button("Load saved run profiles").clicked(){match automation::profiles(&root){Ok(profiles)=>self.profiles=profiles,Err(error)=>self.result=error.to_string()}}
            egui::ComboBox::from_id_salt("flow_run_profile").selected_text(&self.flow.profile.name).show_ui(ui,|ui|{for profile in &self.profiles{if ui.selectable_label(profile.id==self.flow.profile.id,&profile.name).clicked(){self.flow.profile=profile.clone();}}});
            if ui.button("Refresh flow request names").clicked(){self.reload_requests(&root);}
            ui.small("Drag nodes to arrange them. Green follows a successful check; red follows failure. Explicit assertions define success; otherwise HTTP 2xx is required.");
            let height=self.flow.nodes.iter().map(|n|n.position[1]+100.).fold(400.,f32::max).min(10000.);let(canvas,_)=ui.allocate_exact_size(egui::vec2(ui.available_width().max(200.),height),egui::Sense::hover());let last=self.last.lock().unwrap_or_else(|p|p.into_inner()).clone();
            for node in &self.flow.nodes{for(target,color)in[(&node.success,egui::Color32::GREEN),(&node.failure,egui::Color32::LIGHT_RED)]{if let Some(target)=target.as_ref().and_then(|id|self.flow.nodes.iter().find(|n|&n.id==id)){let from=canvas.min+egui::vec2(node.position[0]+90.,node.position[1]+60.);let to=canvas.min+egui::vec2(target.position[0]+90.,target.position[1]);if from.distance(to)>1.{ui.painter().arrow(from,to-from,egui::Stroke::new(1.5_f32,color));}}}}
            for node in &mut self.flow.nodes{let rect=egui::Rect::from_min_size(canvas.min+egui::vec2(node.position[0],node.position[1]),egui::vec2(180.,60.));let response=ui.interact(rect,ui.id().with(&node.id),egui::Sense::click_and_drag());if response.clicked(){self.selected=node.id.clone();}
if response.dragged()&&self.cancel.is_none(){let delta=response.drag_delta();node.position[0]=(node.position[0]+delta.x).clamp(0.,(canvas.width()-180.).max(0.));node.position[1]=(node.position[1]+delta.y).max(0.);}
                let color=if last.as_ref()==Some(&node.id){egui::Color32::from_rgb(35,90,50)}else{egui::Color32::from_rgb(45,50,60)};ui.painter().rect_filled(rect,6.,color);ui.painter().rect_stroke(rect,6.,egui::Stroke::new(if self.selected==node.id{2_f32}else{1_f32},if self.selected==node.id{egui::Color32::from_rgb(240,130,50)}else{egui::Color32::GRAY}));let name=self.requests.iter().find(|(id,_)|id==&node.request_id).map(|(_,name)|name.as_str()).unwrap_or(&node.request_id);ui.painter().text(rect.center(),egui::Align2::CENTER_CENTER,rustapi::utils::truncate_str(name,25),egui::FontId::proportional(13.),egui::Color32::WHITE);}
            let choices=self.flow.nodes.iter().map(|n|(n.id.clone(),self.requests.iter().find(|(id,_)|id==&n.request_id).map(|(_,name)|name.clone()).unwrap_or(n.id.clone()))).collect::<Vec<_>>();let mut remove=None;
            if let Some(node)=self.flow.nodes.iter_mut().find(|n|n.id==self.selected){ui.label("Selected node");egui::ComboBox::from_id_salt("flow_node_request").selected_text(&node.request_id).show_ui(ui,|ui|{for(id,name)in &self.requests{ui.selectable_value(&mut node.request_id,id.clone(),name);}});for(label,edge)in[("On success",&mut node.success),("On failure",&mut node.failure)]{egui::ComboBox::from_id_salt(label).selected_text(edge.as_deref().unwrap_or("End")).show_ui(ui,|ui|{ui.selectable_value(edge,None,"End");for(id,name)in &choices{ui.selectable_value(edge,Some(id.clone()),name);}});}ui.horizontal(|ui|{if ui.button("Start at selected node").clicked(){self.flow.start=node.id.clone();}
if ui.button("Delete selected node").clicked(){remove=Some(node.id.clone());}});}
            if let Some(id)=remove{self.flow.nodes.retain(|n|n.id!=id);for node in &mut self.flow.nodes{if node.success.as_ref()==Some(&id){node.success=None;}
if node.failure.as_ref()==Some(&id){node.failure=None;}}
if self.flow.start==id{self.flow.start=self.flow.nodes.first().map(|n|n.id.clone()).unwrap_or_default();}self.selected.clear();}
            if ui.button("Add flow node").clicked(){if let Some((request_id,_))=self.requests.first(){let id=uuid::Uuid::new_v4().to_string();self.flow.nodes.push(Node{id:id.clone(),request_id:request_id.clone(),success:None,failure:None,position:[30.,30.]});self.selected=id;}}
            ui.horizontal(|ui|{ui.label("Step cap");ui.add(egui::DragValue::new(&mut self.flow.max_steps).range(1..=1000));ui.label("Seconds");ui.add(egui::DragValue::new(&mut self.flow.duration_seconds).range(1..=3600));
                if ui.add_enabled(self.cancel.is_none(),egui::Button::new("Run visual flow")).clicked(){let flow=self.flow.clone();let root=root.clone();let secrets=secrets.clone();let service=service.clone();let cancel=Cancellation::default();self.cancel=Some(cancel.clone());let(sender,receiver)=mpsc::sync_channel(1);self.receiver=Some(receiver);let last=self.last.clone();let context=ctx.clone();std::thread::spawn(move||{let result=rustapi::flows::run(&root,&service,&flow,secrets,&cancel,|step|{*last.lock().unwrap_or_else(|p|p.into_inner())=Some(step.node.clone());context.request_repaint();}).and_then(|report|Ok(serde_json::to_string_pretty(&report)?)).map_err(|error|error.to_string());let _=sender.send(result);context.request_repaint();});}
                if let Some(cancel)=&self.cancel{if ui.button("Cancel flow").clicked(){cancel.cancel();}}
            });ui.label(&self.result);if ui.button("Close flow editor").clicked(){self.open=false;}
        });
        if self.cancel.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
    fn reload_requests(&mut self, root: &std::path::Path) {
        match automation::inputs(root, &self.flow.profile) {
            Ok((collection, _, _)) => {
                self.requests = collection
                    .ordered_requests(self.flow.profile.options.folder.as_deref())
                    .iter()
                    .map(|r| (r.id.clone(), r.name.clone()))
                    .collect()
            }
            Err(error) => self.result = error.to_string(),
        }
    }
}
