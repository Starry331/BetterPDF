//! Local-only, invitation-pinned TLS collaboration. The host serializes edits.
use crate::{documents::Session,engine::{Engine}};
use base64::{Engine as _,engine::general_purpose::URL_SAFE_NO_PAD as B64};
use parking_lot::Mutex;
use rustls::{ClientConfig,ClientConnection,RootCertStore,ServerConfig,ServerConnection,StreamOwned,pki_types::{CertificateDer,PrivatePkcs8KeyDer,ServerName}};
use serde::{Deserialize,Serialize};
use serde_json::{Value,json};
use std::{collections::VecDeque,fs::File,io::{Read,Write},net::{IpAddr,SocketAddr,TcpListener,TcpStream},path::{Path,PathBuf},sync::{Arc,atomic::{AtomicBool,AtomicUsize,Ordering}},thread,time::Duration};
const MAX_FRAME:usize=256*1024;
const MAX_PDF:u64=2*1024*1024*1024;
fn err(e:impl std::fmt::Display)->String{e.to_string()}
pub fn local_address(ip:IpAddr)->bool{match ip{IpAddr::V4(a)=>a.is_private()||a.is_loopback()||a.is_link_local(),IpAddr::V6(a)=>a.is_loopback()||(a.segments()[0]&0xfe00==0xfc00)||(a.segments()[0]&0xffc0==0xfe80)}}
fn send(s:&mut impl Write,v:&Value)->Result<(),String>{let b=serde_json::to_vec(v).map_err(err)?;if b.len()>MAX_FRAME{return Err("Collaboration message is too large".into())}s.write_all(&(b.len()as u32).to_be_bytes()).map_err(err)?;s.write_all(&b).map_err(err)?;s.flush().map_err(err)}
fn receive(s:&mut impl Read)->Result<Value,String>{let mut n=[0;4];s.read_exact(&mut n).map_err(err)?;let len=u32::from_be_bytes(n)as usize;if len>MAX_FRAME{return Err("Collaboration message exceeds limit".into())}let mut b=vec![0;len];s.read_exact(&mut b).map_err(err)?;serde_json::from_slice(&b).map_err(err)}
#[derive(Clone,Serialize,Deserialize)]
pub struct Invitation{version:u8,address:String,token:String,certificate:String}
impl Invitation{
 fn encode(&self)->Result<String,String>{Ok(format!("bpdf-lan:{}",B64.encode(serde_json::to_vec(self).map_err(err)?)))}
 fn decode(s:&str)->Result<Self,String>{if s.len()>16384{return Err("Invalid invitation".into())}let b=B64.decode(s.trim().strip_prefix("bpdf-lan:").ok_or("Invalid invitation")?).map_err(err)?;let v:Self=serde_json::from_slice(&b).map_err(err)?;let address:SocketAddr=v.address.parse().map_err(err)?;if v.version!=1||!local_address(address.ip())||v.token.len()!=64{return Err("Only local network invitations are accepted".into())}Ok(v)}
}
struct Room{session:Arc<Mutex<Session>>,engine:Engine,token:String,stop:Arc<AtomicBool>,seen:Mutex<VecDeque<String>>}
pub struct Host{pub invitation:String,pub stop:Arc<AtomicBool>}
impl Drop for Host{fn drop(&mut self){self.stop.store(true,Ordering::Release);}}
impl Host{
 pub fn start(session:Arc<Mutex<Session>>,engine:Engine,bind:SocketAddr)->Result<Self,String>{
  if !local_address(bind.ip()){return Err("Select a private LAN IP address or localhost".into())}
  let key=rcgen::generate_simple_self_signed(vec!["bpdf.local".into()]).map_err(err)?;
  let cert=key.cert.der().clone();let private=PrivatePkcs8KeyDer::from(key.signing_key.serialize_der());
  let config=ServerConfig::builder().with_no_client_auth().with_single_cert(vec![cert.clone()],private.into()).map_err(err)?;
  let listener=TcpListener::bind(bind).map_err(err)?;listener.set_nonblocking(true).map_err(err)?;
  let token=format!("{}{}",uuid::Uuid::new_v4().simple(),uuid::Uuid::new_v4().simple());
  let invitation=Invitation{version:1,address:listener.local_addr().map_err(err)?.to_string(),token:token.clone(),certificate:B64.encode(cert)}.encode()?;
  let stop=Arc::new(AtomicBool::new(false));let room=Arc::new(Room{session,engine,token,stop:stop.clone(),seen:Mutex::new(VecDeque::new())});let config=Arc::new(config);let active=Arc::new(AtomicUsize::new(0));
  thread::spawn(move||{while !room.stop.load(Ordering::Acquire){match listener.accept(){Ok((socket,peer))=>{
   if !local_address(peer.ip())||active.load(Ordering::Relaxed)>=8{continue}active.fetch_add(1,Ordering::Relaxed);let room=room.clone();let config=config.clone();let active=active.clone();thread::spawn(move||{let _=socket.set_nonblocking(false);let _=socket.set_read_timeout(Some(Duration::from_secs(15)));let _=socket.set_write_timeout(Some(Duration::from_secs(30)));if let Ok(conn)=ServerConnection::new(config){let mut stream=StreamOwned::new(conn,socket);let result=serve(&room,&mut stream);if let Err(e)=result{#[cfg(test)] eprintln!("Server protocol error: {e}");let _=send(&mut stream,&json!({"ok":false,"error":e}));}stream.conn.send_close_notify();let _=stream.flush();}active.fetch_sub(1,Ordering::Relaxed);});
  },Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>thread::sleep(Duration::from_millis(40)),Err(_)=>break}}});
  Ok(Self{invitation,stop})
 }
}
fn serve(room:&Room,s:&mut(impl Read+Write))->Result<(),String>{
 let q=receive(s)?;if q["token"].as_str()!=Some(&room.token)||room.stop.load(Ordering::Acquire){return Err("Invitation expired or unauthorized".into())}
 // No client-supplied paths reach the engine. Images must use the transfer workflow.
 let mut session=room.session.lock();let op=q["op"].as_str().unwrap_or("");
 if op=="edit"{let id=q["operationId"].as_str().ok_or("Missing operation ID")?;if id.len()!=36{return Err("Invalid operation ID".into())}
  if !room.seen.lock().iter().any(|v|v==id){
   if !q["revision"].as_u64().map(|base|session.can_apply(&q["edit"],base)).unwrap_or(false){return Err("COLLAB_CONFLICT: document changed; refresh before applying your edit".into())}
   let edit=q["edit"].clone();validate_edit(&edit)?;
   if room.stop.load(Ordering::Acquire){return Err("Session ended".into())}session.edit(&room.engine,edit)?;let mut seen=room.seen.lock();seen.push_back(id.to_string());if seen.len()>1024{seen.pop_front();}
  }
 }else if op!="sync"{return Err("Unsupported collaboration operation".into())}
 let revision=session.view.revision;
 if op=="sync"&&q["revision"].as_u64()==Some(revision){return send(s,&json!({"ok":true,"revision":revision,"unchanged":true}))}
 let mut file=File::open(session.current()).map_err(err)?;let size=file.metadata().map_err(err)?.len();if size>MAX_PDF{return Err("Shared PDF exceeds 2 GB limit".into())}let name=session.view.name.clone();drop(session);
 send(s,&json!({"ok":true,"revision":revision,"name":name,"bytes":size}))?;
 let mut chunk=[0u8;64*1024];loop{if room.stop.load(Ordering::Acquire){return Err("Session ended".into())}let n=file.read(&mut chunk).map_err(err)?;if n==0{break}s.write_all(&chunk[..n]).map_err(err)?;}s.flush().map_err(err)
}
fn validate_edit(e:&Value)->Result<(),String>{
 if !["text","image","highlight","note","ink","widget"].contains(&e["kind"].as_str().unwrap_or("")){return Err("Unsupported edit".into())}
 if e.get("path").is_some()||e.get("input").is_some()||e.get("output").is_some(){return Err("Remote file paths are not accepted. Insert or replace images on the host.".into())}
 if e["kind"]=="text"&&!matches!(e["font"].as_str(),None|Some("__original__"|"Helvetica"|"Times-Roman"|"Courier"|"zh-Hans")){return Err("Unsupported font".into())}Ok(())
}
pub struct Client{invite:Invitation,config:Arc<ClientConfig>,pub revision:Option<u64>,pub pending:Option<(String,Value,u64)>}
pub struct Snapshot{pub path:PathBuf,pub name:String,pub revision:u64}
impl Client{
 pub fn new(code:&str)->Result<Self,String>{let invite=Invitation::decode(code)?;let der=B64.decode(&invite.certificate).map_err(err)?;let mut roots=RootCertStore::empty();roots.add(CertificateDer::from(der)).map_err(err)?;let config=ClientConfig::builder().with_root_certificates(roots).with_no_client_auth();Ok(Self{invite,config:Arc::new(config),revision:None,pending:None})}
 pub fn exchange(&mut self,edit:Option<(Value,u64)>,cache:&Path)->Result<Option<Snapshot>,String>{
  if let Some((edit,base))=edit{validate_edit(&edit)?;match &self.pending{Some((_,old,_)) if old!=&edit=>return Err("Retry or discard the pending edit first".into()),Some(_)=>{},None=>self.pending=Some((uuid::Uuid::new_v4().to_string(),edit,base))}}
  let address:SocketAddr=self.invite.address.parse().map_err(err)?;let socket=TcpStream::connect_timeout(&address,Duration::from_secs(5)).map_err(err)?;socket.set_read_timeout(Some(Duration::from_secs(120))).map_err(err)?;socket.set_write_timeout(Some(Duration::from_secs(30))).map_err(err)?;
  let conn=ClientConnection::new(self.config.clone(),ServerName::try_from("bpdf.local").map_err(err)?).map_err(err)?;let mut stream=StreamOwned::new(conn,socket);
  let query=if let Some((id,e,r))=&self.pending{json!({"op":"edit","edit":e,"revision":r,"operationId":id,"token":self.invite.token})}else{json!({"op":"sync","revision":self.revision,"token":self.invite.token})};send(&mut stream,&query)?;let result=receive(&mut stream).map_err(|e|format!("Snapshot header: {e}"))?;
  if result["ok"]!=true{let message=result["error"].as_str().unwrap_or("Collaboration request failed");self.pending=None;return Err(message.into())}
  if result["unchanged"]==true{return Ok(None)}
  let size=result["bytes"].as_u64().filter(|n|*n<=MAX_PDF).ok_or("Invalid snapshot size")?;let revision=result["revision"].as_u64().ok_or("Invalid revision")?;
  let mut temp=tempfile::NamedTempFile::new_in(cache).map_err(err)?;let copied=std::io::copy(&mut stream.take(size),&mut temp).map_err(err)?;if copied!=size{return Err("Incomplete transfer; local document retained".into())}temp.as_file_mut().sync_all().map_err(err)?;
  let path=cache.join(format!("shared-{}.pdf",uuid::Uuid::new_v4()));temp.persist_noclobber(&path).map_err(err)?;
  // Revision advances only when the caller has validated and adopted the snapshot.
  Ok(Some(Snapshot{path,name:result["name"].as_str().unwrap_or("Shared.pdf").chars().filter(|c|!c.is_control()&&*c!='/'&&*c!='\\').take(200).collect(),revision}))
 }
 pub fn accepted(&mut self,revision:u64){self.revision=Some(revision);self.pending=None;}
}
#[cfg(test)]mod tests{
 use super::*;
 #[test]fn framing_is_bounded(){let mut b=&(MAX_FRAME as u32+1).to_be_bytes()[..];assert!(receive(&mut b).is_err());let mut buf=vec![];send(&mut buf,&json!({"ok":true})).unwrap();assert_eq!(receive(&mut &buf[..]).unwrap()["ok"],true);}
 #[test]fn local_only_and_paths_rejected(){assert!(local_address("192.168.1.2".parse().unwrap()));assert!(!local_address("8.8.8.8".parse().unwrap()));assert!(validate_edit(&json!({"kind":"image","path":"/private/file"})).is_err());}
}

#[cfg(test)]mod integration {
 use super::*;
 #[test]
 #[ignore = "requires bundled native runtime"]
 fn lan_roundtrip_conflicts_retry_auth_and_shutdown(){
  let engine=Engine{resources:PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources")};let temp=tempfile::tempdir().unwrap();let source=temp.path().join("fixture.pdf");engine.request(json!({"action":"fixture","output":source,"pages":2}),&crate::engine::fresh_cancel()).unwrap();let session=Arc::new(Mutex::new(Session::open(&engine,temp.path(),&source,String::new()).unwrap()));
  let host=Host::start(session.clone(),engine.clone(),"127.0.0.1:0".parse().unwrap()).unwrap();let mut a=Client::new(&host.invitation).unwrap();let mut b=Client::new(&host.invitation).unwrap();
  for client in [&mut a,&mut b]{let snap=client.exchange(None,temp.path()).unwrap().unwrap();assert_eq!(engine.info(&snap.path,"").unwrap()["pages"],2);client.accepted(snap.revision);std::fs::remove_file(snap.path).unwrap();}
  assert!(a.exchange(None,temp.path()).unwrap().is_none());
  let edit=json!({"kind":"text","insert":true,"page":0,"rect":[50,200,500,250],"size":16,"text":"Shared edit","font":"Helvetica"});
  let first=a.exchange(Some((edit.clone(),0)),temp.path()).unwrap().unwrap();assert_eq!(first.revision,1);
  // Simulate a lost acknowledgement: replay must not apply twice.
  let replay=a.exchange(None,temp.path()).unwrap().unwrap();assert_eq!(replay.revision,1);a.accepted(1);
  assert!(b.exchange(Some((edit,0)),temp.path()).err().unwrap().starts_with("COLLAB_CONFLICT"));
  let changed=b.exchange(None,temp.path()).unwrap().unwrap();assert_eq!(changed.revision,1);let content=engine.request(json!({"action":"page","input":changed.path,"page":0}),&crate::engine::fresh_cancel()).unwrap();assert!(content["lines"].as_array().unwrap().iter().any(|l|l["text"]=="Shared edit"));b.accepted(1);
  // Different pages can commit from the same base without discarding either edit.
  let other=json!({"kind":"text","insert":true,"page":1,"rect":[50,200,500,250],"size":16,"text":"Parallel page edit","font":"Helvetica"});
  let parallel=b.exchange(Some((other,0)),temp.path()).unwrap().unwrap();assert_eq!(parallel.revision,2);b.accepted(2);
  let first_page=engine.request(json!({"action":"page","input":parallel.path,"page":0}),&crate::engine::fresh_cancel()).unwrap();assert!(first_page["lines"].as_array().unwrap().iter().any(|l|l["text"]=="Shared edit"));
  assert!(session.lock().can_apply(&json!({"kind":"text","page":0}),1));assert!(!session.lock().can_apply(&json!({"kind":"widget","page":0}),1));
  let mut invitation=Invitation::decode(&host.invitation).unwrap();invitation.token="0".repeat(64);let mut invalid=Client::new(&invitation.encode().unwrap()).unwrap();assert!(invalid.exchange(None,temp.path()).is_err());
  let unrelated=rcgen::generate_simple_self_signed(vec!["bpdf.local".into()]).unwrap();invitation.certificate=B64.encode(unrelated.cert.der());let mut wrong_cert=Client::new(&invitation.encode().unwrap()).unwrap();assert!(wrong_cert.exchange(None,temp.path()).is_err());
  drop(host);thread::sleep(Duration::from_millis(100));assert!(a.exchange(None,temp.path()).is_err());assert_eq!(session.lock().view.revision,2);
 }
}
