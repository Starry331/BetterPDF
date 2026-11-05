#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod native_menu;mod collaboration;mod engine;mod documents;mod jobs;mod export;mod caj;
use engine::{Engine,Cancel,fresh_cancel};
use documents::{Session,SessionView};
use jobs::Task;
use parking_lot::Mutex;
use serde_json::{Value,json};
use std::{collections::HashMap,path::PathBuf,sync::{Arc,mpsc,atomic::Ordering}};
use tauri::{Manager,Emitter,State};
struct Job{id:String,tool:String,inputs:Vec<PathBuf>,output:PathBuf,options:Value,cancel:Cancel}
struct AppState{hosts:Mutex<HashMap<String,collaboration::Host>>,clients:Mutex<HashMap<String,Arc<Mutex<collaboration::Client>>>>,engine:Engine,cache:PathBuf,docs:Mutex<HashMap<String,Arc<Mutex<Session>>>>,tasks:Arc<Mutex<Vec<Task>>>,cancels:Mutex<HashMap<String,Cancel>>,queue:mpsc::Sender<Job>}
fn doc(state:&AppState,id:&str)->Result<Arc<Mutex<Session>>,String>{state.docs.lock().get(id).cloned().ok_or_else(||"Document is closed".into())}
async fn blocking<T:Send+'static>(f:impl FnOnce()->Result<T,String>+Send+'static)->Result<T,String>{tauri::async_runtime::spawn_blocking(f).await.map_err(|e|e.to_string())?}
#[tauri::command]
async fn open_document(state:State<'_,AppState>,path:String,password:String)->Result<SessionView,String>{let engine=state.engine.clone();let cache=state.cache.clone();let session=blocking(move||Session::open(&engine,&cache,&PathBuf::from(path),password)).await?;let view=session.view.clone();state.docs.lock().insert(view.id.clone(),Arc::new(Mutex::new(session)));Ok(view)}
#[tauri::command]fn close_document(state:State<AppState>,id:String){state.hosts.lock().remove(&id);state.clients.lock().remove(&id);state.docs.lock().remove(&id);}
#[tauri::command]async fn render_page(state:State<'_,AppState>,id:String,page:usize,scale:f64)->Result<String,String>{let doc=doc(&state,&id)?;let engine=state.engine.clone();blocking(move||doc.lock().render(&engine,page,scale).map(|p|p.to_string_lossy().into())).await}
#[tauri::command]async fn page_data(state:State<'_,AppState>,id:String,page:usize)->Result<Value,String>{let doc=doc(&state,&id)?;let engine=state.engine.clone();blocking(move||{let d=doc.lock();if page>=d.view.pages{return Err("Invalid page".into())}engine.request(json!({"action":"page","input":d.current(),"password":d.password,"page":page}),&fresh_cancel())}).await}
#[tauri::command]async fn search_document(state:State<'_,AppState>,id:String,query:String)->Result<Value,String>{if query.is_empty()||query.len()>1000{return Err("Enter a search phrase".into())}let doc=doc(&state,&id)?;let engine=state.engine.clone();let (path,password)={let d=doc.lock();(d.current().to_path_buf(),d.password.clone())};blocking(move||engine.request(json!({"action":"search","input":path,"password":password,"query":query}),&fresh_cancel())).await}
#[tauri::command]async fn edit_document(state:State<'_,AppState>,id:String,edit:Value,expected_revision:Option<u64>)->Result<SessionView,String>{
 let document=doc(&state,&id)?;let engine=state.engine.clone();let client=state.clients.lock().get(&id).cloned();let cache=state.cache.clone();
 blocking(move||{if let Some(client)=client{let mut client=client.lock();let base=expected_revision.ok_or("Missing revision")?;let snapshot=client.exchange(Some((edit,base)),&cache)?.ok_or("Missing edited snapshot")?;let result=document.lock().adopt(&engine,&snapshot.path,snapshot.name,snapshot.revision);let _=std::fs::remove_file(&snapshot.path);if result.is_ok(){client.accepted(snapshot.revision);}result}else{let mut d=document.lock();if let Some(expected)=expected_revision{if !d.can_apply(&edit,expected){return Err("COLLAB_CONFLICT: document changed; refresh before editing".into())}}d.edit(&engine,edit)}}).await
}
#[tauri::command]async fn document_history(state:State<'_,AppState>,id:String,direction:String)->Result<SessionView,String>{if state.hosts.lock().contains_key(&id)||state.clients.lock().contains_key(&id){return Err("Undo is unavailable during collaboration. End the session to edit a local copy.".into())}let d=doc(&state,&id)?;blocking(move||d.lock().history(&direction)).await}
#[tauri::command]async fn save_document(state:State<'_,AppState>,id:String,output:String)->Result<(),String>{let doc=doc(&state,&id)?;blocking(move||doc.lock().save(&PathBuf::from(output))).await}
#[tauri::command]fn list_tasks(state:State<AppState>)->Vec<Task>{state.tasks.lock().clone()}
#[tauri::command]fn cancel_task(state:State<AppState>,id:String)->Result<(),String>{if let Some(c)=state.cancels.lock().get(&id){c.store(true,Ordering::Relaxed);Ok(())}else{Err("Task not found".into())}}
#[tauri::command]fn capabilities(state:State<AppState>)->HashMap<String,bool>{["mutool","qpdf","tesseract","gs","soffice"].into_iter().map(|name|(name.into(),state.engine.tool(name).is_ok())).collect()}
#[tauri::command]fn submit_task(state:State<AppState>,tool:String,inputs:Vec<String>,output:String,options:Value)->Result<String,String>{
 if !jobs::TOOLS.contains(&tool.as_str()){return Err("Unknown tool".into())}if inputs.is_empty(){return Err("Choose input files".into())}let inputs=inputs.into_iter().map(|p|PathBuf::from(p).canonicalize().map_err(|e|e.to_string())).collect::<Result<Vec<_>,_>>()?;
 let id=uuid::Uuid::new_v4().to_string();let cancel=fresh_cancel();let task=Task{id:id.clone(),tool:tool.clone(),state:"queued".into(),progress:0.,message:"Queued".into(),outputs:vec![],warnings:vec![]};
 state.tasks.lock().push(task);state.cancels.lock().insert(id.clone(),cancel.clone());state.queue.send(Job{id:id.clone(),tool,inputs,output:PathBuf::from(output),options,cancel}).map_err(|e|e.to_string())?;Ok(id)
}
#[tauri::command]async fn preview_task(state:State<'_,AppState>,tool:String,inputs:Vec<String>,options:Value)->Result<Value,String>{let engine=state.engine.clone();let cache=state.cache.clone();blocking(move||jobs::preview(&engine,&tool,&inputs.into_iter().map(PathBuf::from).collect::<Vec<_>>(),&options,&cache)).await}
#[tauri::command]async fn print_document(state:State<'_,AppState>,id:String)->Result<(),String>{let d=doc(&state,&id)?;let path=d.lock().current().to_path_buf();blocking(move||{
 #[cfg(target_os="macos")] {let status=std::process::Command::new("/usr/bin/open").args(["-a","Preview"]).arg(path).status().map_err(|e|e.to_string())?;if !status.success(){return Err("Could not open the system print preview".into())}}
 #[cfg(target_os="windows")] {let status=std::process::Command::new("rundll32.exe").arg("shell32.dll,ShellExec_RunDLL").arg(path).status().map_err(|e|e.to_string())?;if !status.success(){return Err("Could not open the system PDF viewer".into())}}
 Ok(())}).await}
#[tauri::command]async fn prepare_tool_input(state:State<'_,AppState>,id:String)->Result<String,String>{let d=doc(&state,&id)?;let engine=state.engine.clone();let output=state.cache.join(format!("document-{}.pdf",uuid::Uuid::new_v4()));blocking(move||{let d=d.lock();engine.request(json!({"action":"unlock","input":d.current(),"password":d.password,"output":output}),&fresh_cancel())?;Ok(output.to_string_lossy().into())}).await}
#[tauri::command]fn lan_address()->String{std::net::UdpSocket::bind("0.0.0.0:0").and_then(|s|{s.connect("192.168.0.1:9")?;s.local_addr()}).map(|a|a.ip().to_string()).unwrap_or_else(|_|"127.0.0.1".into())}
#[tauri::command]async fn collaboration_host(state:State<'_,AppState>,id:String,address:String)->Result<Value,String>{
 let original=doc(&state,&id)?;let engine=state.engine.clone();let cache=state.cache.clone();let bind:std::net::SocketAddr=format!("{address}:0").parse().map_err(|_|"Enter a local IPv4 address")?;
 let (session,host)=blocking(move||{let d=original.lock();if std::fs::metadata(d.current()).map_err(|e|e.to_string())?.len()>2*1024*1024*1024{return Err("Shared document exceeds 2 GB".into())}let temp=tempfile::Builder::new().suffix(".pdf").tempfile_in(&cache).map_err(|e|e.to_string())?;engine.request(json!({"action":"unlock","input":d.current(),"password":d.password,"output":temp.path()}),&fresh_cancel())?;let mut s=Session::open(&engine,&cache,temp.path(),String::new())?;s.view.name=d.view.name.clone();let session=Arc::new(Mutex::new(s));let host=collaboration::Host::start(session.clone(),engine,bind)?;Ok((session,host))}).await?;
 let view=session.lock().view.clone();let invitation=host.invitation.clone();state.docs.lock().insert(view.id.clone(),session);state.hosts.lock().insert(view.id.clone(),host);Ok(json!({"document":view,"invitation":invitation,"role":"host"}))
}
#[tauri::command]async fn collaboration_join(state:State<'_,AppState>,invitation:String)->Result<Value,String>{
 let engine=state.engine.clone();let cache=state.cache.clone();let (session,client)=blocking(move||{let mut client=collaboration::Client::new(&invitation)?;let snapshot=client.exchange(None,&cache)?.ok_or("Missing snapshot")?;let result=Session::open(&engine,&cache,&snapshot.path,String::new());let _=std::fs::remove_file(snapshot.path);let mut session=result?;session.view.name=snapshot.name;session.view.revision=snapshot.revision;client.accepted(snapshot.revision);Ok((session,client))}).await?;let view=session.view.clone();state.docs.lock().insert(view.id.clone(),Arc::new(Mutex::new(session)));state.clients.lock().insert(view.id.clone(),Arc::new(Mutex::new(client)));Ok(json!({"document":view,"role":"guest"}))
}
#[tauri::command]async fn collaboration_sync(state:State<'_,AppState>,id:String)->Result<SessionView,String>{
 let document=doc(&state,&id)?;let client=state.clients.lock().get(&id).cloned();let engine=state.engine.clone();let cache=state.cache.clone();blocking(move||{if let Some(client)=client{let mut client=client.lock();if let Some(snapshot)=client.exchange(None,&cache)?{let result=document.lock().adopt(&engine,&snapshot.path,snapshot.name,snapshot.revision);let _=std::fs::remove_file(snapshot.path);let view=result?;client.accepted(snapshot.revision);return Ok(view)}}Ok(document.lock().view.clone())}).await
}
#[tauri::command]fn collaboration_leave(state:State<AppState>,id:String){state.hosts.lock().remove(&id);state.clients.lock().remove(&id);}
#[tauri::command]fn quit_application(app:tauri::AppHandle){app.exit(0);}
#[tauri::command]fn launch_files()->Vec<String>{std::env::args().skip(1).filter(|s|s.to_lowercase().ends_with(".pdf")&&std::path::Path::new(s).is_file()).collect()}
fn main(){
 let app=tauri::Builder::default().plugin(tauri_plugin_dialog::init()).plugin(tauri_plugin_opener::init()).setup(|app|{
  let resources=if cfg!(debug_assertions){PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources")}else{app.path().resource_dir()?.join("resources")};let engine=Engine{resources};let cache=app.path().app_cache_dir()?.join(uuid::Uuid::new_v4().to_string());std::fs::create_dir_all(&cache)?;
  let (tx,rx)=mpsc::channel::<Job>();let tasks:Arc<Mutex<Vec<Task>>>=Arc::new(Mutex::new(Vec::new()));let worker_tasks=tasks.clone();let worker_engine=engine.clone();let handle=app.handle().clone();
  std::thread::spawn(move||{for job in rx{let update=|progress:f64,message:&str|{let mut tasks=worker_tasks.lock();if let Some(t)=tasks.iter_mut().find(|t|t.id==job.id){t.state="running".into();t.progress=progress;t.message=message.into();let _=handle.emit("task-updated",t.clone());}};update(0.01,"Processing locally");let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||jobs::run(&worker_engine,&job.tool,&job.inputs,&job.output,&job.options,&job.cancel,&update))).unwrap_or_else(|_|Err("Task worker failed".into()));let mut tasks=worker_tasks.lock();if let Some(t)=tasks.iter_mut().find(|t|t.id==job.id){match result{Ok((outputs,warnings))=>{t.state="completed".into();t.progress=1.;t.message="Completed".into();t.outputs=outputs;t.warnings=warnings},Err(e)=>{t.state=if e=="CANCELLED"{"cancelled"}else{"failed"}.into();t.message=e;}}let _=handle.emit("task-updated",t.clone());}}});
  app.manage(AppState{hosts:Mutex::new(HashMap::new()),clients:Mutex::new(HashMap::new()),engine,cache,docs:Mutex::new(HashMap::new()),tasks,cancels:Mutex::new(HashMap::new()),queue:tx});Ok(())
 }).on_menu_event(|app,event|{
  if event.id().as_ref()=="quit"{if let Some(window)=app.get_webview_window("main"){let _=window.close();}}
  else{let _=app.emit("menu-action",event.id().as_ref());}
 }).invoke_handler(tauri::generate_handler![native_menu::update_native_menu,open_document,close_document,render_page,page_data,search_document,edit_document,document_history,save_document,list_tasks,cancel_task,capabilities,submit_task,preview_task,print_document,prepare_tool_input,launch_files,quit_application,lan_address,collaboration_host,collaboration_join,collaboration_sync,collaboration_leave]).build(tauri::generate_context!()).expect("Failed to start BPDF");
 app.run(|handle,event|{match event{
  tauri::RunEvent::Exit=>{let state=handle.state::<AppState>();for c in state.cancels.lock().values(){c.store(true,Ordering::Relaxed);}state.hosts.lock().clear();state.clients.lock().clear();state.docs.lock().clear();let _=std::fs::remove_dir_all(&state.cache);},
  #[cfg(target_os="macos")] tauri::RunEvent::Opened{urls}=>{let files:Vec<_>=urls.into_iter().filter_map(|u|u.to_file_path().ok()).map(|p|p.to_string_lossy().to_string()).collect();let _=handle.emit("open-files",files);},_=>{}
 }});
}
