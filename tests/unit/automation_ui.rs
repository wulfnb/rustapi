use super::*;
use crate::test_support::{Harness, Profile};
use dialogs::{Kind, Selections};
#[test]
fn saved_profiles_datasets_runs_history_and_load_controls() {
    let profile = Profile::new();
    let root = profile.directory.path();
    let mut collection = Collection::new("Runnable GUI collection");
    collection.requests.push(crate::models::ApiRequest::new(
        "Invalid URL reports failure",
    ));
    crate::storage::save_collection(&collection, None).unwrap();
    let collections = vec![collection];
    let service = Arc::new(HttpService::for_workspace(root));
    let dataset = root.join("source.csv");
    std::fs::write(&dataset, "id\n1\n").unwrap();
    let load_file = root.join("load.json");
    let _dialogs = Selections::new(vec![
        (Kind::Open, Some(dataset)),
        (Kind::Save, Some(load_file.clone())),
        (Kind::Open, Some(load_file)),
    ]);
    let mut h = Harness::new(AutomationTools::default(), |ctx, state| {
        egui::CentralPanel::default().show(ctx, |ui| {
            state.render(ui, &collections, service.clone(), &BTreeMap::new())
        });
    });
    h.click("Saved runs, datasets and scheduling");
    h.click("Runnable GUI collection");
    h.click("Import CSV / JSON dataset");
    assert_eq!(h.state.form.dataset.as_deref(), Some("datasets/source.csv"));
    h.click("Schedule while scheduler is running");
    h.click("Failure webhook");
    h.click("Save run profile");
    assert!(
        h.state.result.contains("Profile saved"),
        "{}",
        h.state.result
    );
    h.click("Refresh saved run profiles");
    assert_eq!(h.state.profiles.len(), 1);
    h.click("Run profile now");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while h.state.job.is_some() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.refresh();
    }
    assert!(h.state.job.is_none());
    h.click("Refresh run history");
    assert_eq!(h.state.history.len(), 1);
    h.state.before = h.state.history[0].id.clone();
    h.state.after = h.state.before.clone();
    h.click("Compare run results");
    h.click("Performance testing");
    h.click("Use selected run profile for load");
    h.click("p95 latency threshold");
    h.click("Add load stage");
    assert_eq!(h.state.load.stages.len(), 1);
    h.click("Remove stage");
    h.click("Save load profile");
    assert_eq!(h.state.result, "Load profile saved");
    h.click("Load performance profile");
    for i in 0..100 {
        h.state.sender.send(Event::Message(i.to_string())).unwrap();
    }
    h.state
        .sender
        .send(Event::Finished(Ok("Done".into())))
        .unwrap();
    assert_eq!(h.state.sender.0.lock().unwrap().len(), 64);
    h.refresh();
    assert_eq!(h.state.result, "Done");
}
