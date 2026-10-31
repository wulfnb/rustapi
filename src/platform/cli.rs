use super::{
    ai,
    collaboration::{Client, Config, Role, TeamServer, User},
    external_secrets, mcp,
};
use anyhow::{bail, ensure, Context, Result};
use std::{path::Path, time::Duration};
pub fn run(command: &str, args: &[String], root: &Path, allow_execution: bool) -> Result<i32> {
    let read = |index: usize, limit| {
        crate::tools_cli::read(
            Path::new(args.get(index).context("Missing input path")?),
            limit,
        )
    };
    let cancel = crate::http::Cancellation::default();
    match command {
        "team"=>match args.first().map(String::as_str){
            Some("init") if args.len()==2=>{let path=Path::new(&args[1]);ensure!(!path.exists(),"Server config already exists");let(user,token)=User::issue("administrator".into(),Role::Admin,30)?;let config=Config{users:vec![user],..Default::default()};crate::workspace::atomic_write(path,&serde_json::to_vec_pretty(&config)?)?;println!("{}",serde_json::json!({"config":path,"token":token,"expires_in_days":30}));},
            Some("token") if args.len()==5=>{let mut config:Config=serde_json::from_str(&read(1,1024*1024)?)?;let role=match args[3].as_str(){"Viewer"=>Role::Viewer,"Editor"=>Role::Editor,"Admin"=>Role::Admin,_=>bail!("Role must be Viewer, Editor or Admin")};let(user,token)=User::issue(args[2].clone(),role,args[4].parse()?)?;let id=user.id.clone();config.users.push(user);config.validate()?;crate::workspace::atomic_write(Path::new(&args[1]),&serde_json::to_vec_pretty(&config)?)?;println!("{}",serde_json::json!({"id":id,"token":token,"restart_required":true}));},
            Some("serve") if args.len()==3=>{let config=serde_json::from_str(&read(1,1024*1024)?)?;let server=TeamServer::start(Path::new(&args[2]),config)?;let signal=cancel.clone();ctrlc::set_handler(move||signal.cancel())?;println!("Team server listening on http://{}",server.server.address);while !cancel.is_cancelled(){std::thread::sleep(Duration::from_millis(100));}},
            Some("catalog"|"pull"|"push")=>{let endpoint=args.get(1).context("Team endpoint missing")?;let token=std::env::var("RUSTAPI_TEAM_TOKEN").context("Set RUSTAPI_TEAM_TOKEN")?;let client=Client::for_workspace(root,endpoint,&token)?;
                match args[0].as_str(){"catalog" if args.len()==2=>println!("{}",serde_json::to_string_pretty(&client.catalog()?)?),"pull" if args.len()==4=>{ensure!(!Path::new(&args[3]).exists(),"Pull output already exists");let revision=client.pull(&args[2])?;crate::workspace::atomic_write(Path::new(&args[3]),&serde_json::to_vec_pretty(&revision.collection)?)?;eprintln!("Revision: {}",revision.revision);},"push" if args.len()==4=>{let collection=crate::tools_cli::collection(Path::new(&args[2]),false)?;println!("{}",serde_json::json!({"revision":client.push(&collection,&args[3])?}));},_=>bail!("Invalid team client arguments")}
            },
            _=>bail!("Usage: rustapi team init CONFIG | token CONFIG NAME ROLE DAYS | serve CONFIG DATA_DIR | catalog ENDPOINT | pull ENDPOINT ID OUTPUT | push ENDPOINT COLLECTION REVISION"),
        },
        "flow"=>{ensure!(args.len()==2,"Usage: rustapi flow save|run FLOW.json");let flow:crate::flows::Flow=serde_json::from_str(&read(1,8*1024*1024)?)?;if args[0]=="save"{println!("{}",flow.save(root)?.display());}else{ensure!(args[0]=="run","Unknown flow command");let(service,secrets)=crate::tools_cli::unlocked(root)?;let signal=cancel.clone();ctrlc::set_handler(move||signal.cancel())?;let report=crate::flows::run(root,&service,&flow,secrets,&cancel,|_|{})?;println!("{}",serde_json::to_string_pretty(&report)?);return Ok(i32::from(!report.passed));}},
        "ai"=>{let service=crate::http::HttpService::for_workspace(root);match args.first().map(String::as_str){Some("models") if args.len()==2=>println!("{}",serde_json::to_string_pretty(&ai::models(&service,&args[1],&cancel)?)?),Some("draft") if args.len()==5=>{ensure!(!Path::new(&args[4]).exists(),"AI draft output already exists");let draft=ai::draft(&service,&args[1],&args[2],&read(3,256*1024)?,None,&cancel)?;let mut collection=crate::models::Collection::new("AI draft — review before sending");collection.requests.push(draft);crate::workspace::atomic_write(Path::new(&args[4]),&serde_json::to_vec_pretty(&collection)?)?;},_=>bail!("Usage: rustapi ai models ENDPOINT | draft ENDPOINT MODEL PROMPT.txt OUTPUT.json")}},
        "mcp"=>{ensure!(args==["serve"],"Usage: rustapi mcp serve [--allow-execution]");let(service,secrets)=crate::tools_cli::unlocked(root)?;mcp::serve(root,&service,&secrets,allow_execution)?;},
        "secrets"=>{ensure!(args==["check"],"Usage: rustapi secrets check");let service=crate::http::HttpService::for_workspace(root);let values=external_secrets::load(root,&service,&cancel)?;println!("{}",serde_json::json!({"resolved":values.keys().collect::<Vec<_>>()}));},
        "policy"=>{match args.first().map(String::as_str){Some("template") if args.len()==2=>{ensure!(!Path::new(&args[1]).exists(),"Policy output already exists");crate::workspace::atomic_write(Path::new(&args[1]),&serde_json::to_vec_pretty(&crate::policy::Policy::default())?)?;},Some("validate") if args.len()==1=>{crate::policy::Policy::load(root)?.validate()?;println!("Policy is valid");},_=>bail!("Usage: rustapi policy template OUTPUT | validate")}},
        _=>bail!("Unknown platform command"),
    }
    Ok(0)
}
