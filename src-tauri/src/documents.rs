use crate::engine::{Engine,fresh_cancel};
use serde::Serialize;
use serde_json::{Value,json};
use std::{fs,path::{Path,PathBuf}};
use uuid::Uuid;
#[derive(Clone,Serialize)]
#[serde(rename_all="camelCase")]
pub struct SessionView{pub format:String,pub id:String,pub path:String,pub name:String,pub pages:usize,pub bounds:Value,pub outline:Value,pub revision:u64,pub dirty:bool,pub can_undo:bool,pub can_redo:bool}
pub struct Session{pub original_stamp:Option<(u64,std::time::SystemTime)>,pub view:SessionView,pub dir:PathBuf,pub versions:Vec<PathBuf>,pub cursor:usize,pub password:String,pub saved_cursor:usize,pub page_revisions:std::collections::HashMap<usize,u64>,pub revision_floor:u64}
fn file_stamp(path:&Path)->Option<(u64,std::time::SystemTime)>{let m=fs::metadata(path).ok()?;Some((m.len(),m.modified().ok()?))}
impl Session{
 pub fn open(engine:&Engine,cache:&Path,path:&Path,password:String)->Result<Self,String>{
  if crate::office::supported(path){
   let original=path.canonicalize().map_err(|e|e.to_string())?;
   let temp=tempfile::tempdir_in(cache).map_err(|e|e.to_string())?;
   let ext=crate::office::format(&original);let modern=match ext.as_str(){"doc"=>"docx","xls"=>"xlsx","ppt"=>"pptx",_=>ext.as_str()};
   let source=temp.path().join("source").with_extension(modern);
   if ext==modern{fs::copy(&original,&source).map_err(|e|e.to_string())?;}else{crate::jobs::office_convert(engine,&original,&source,temp.path(),modern,&fresh_cancel())?;}
   crate::office::page(&source,0,"","")?;
   let preview=temp.path().join("preview.pdf");crate::jobs::office_convert(engine,&source,&preview,temp.path(),"pdf",&fresh_cancel())?;
   let mut session=Self::open(engine,cache,&preview,String::new())?;
   fs::copy(&source,session.current().with_extension(modern)).map_err(|e|e.to_string())?;
   session.original_stamp=file_stamp(&original);session.view.format=modern.into();session.view.path=original.to_string_lossy().into();session.view.name=original.file_name().unwrap_or_default().to_string_lossy().into();return Ok(session)
  }
  let path=path.canonicalize().map_err(|e|e.to_string())?;if !path.is_file(){return Err("Not a file".into())}let info=engine.info(&path,&password)?;
  let id=Uuid::new_v4().to_string();let dir=cache.join(&id);fs::create_dir_all(&dir).map_err(|e|e.to_string())?;
  let snapshot=dir.join("original.pdf");fs::copy(&path,&snapshot).map_err(|e|e.to_string())?;
  let view=SessionView{format:"pdf".into(),id,path:path.to_string_lossy().into(),name:path.file_name().unwrap_or_default().to_string_lossy().into(),pages:info["pages"].as_u64().unwrap_or(0) as usize,bounds:info["bounds"].clone(),outline:info["outline"].clone(),revision:0,dirty:false,can_undo:false,can_redo:false};
  Ok(Self{original_stamp:file_stamp(&path),view,dir,versions:vec![snapshot],cursor:0,password,saved_cursor:0,page_revisions:Default::default(),revision_floor:0})
 }
 pub fn current(&self)->&Path{&self.versions[self.cursor]}
 pub fn office_source(&self)->PathBuf{self.current().with_extension(&self.view.format)}
 pub fn office_page(&self,offset:usize,query:&str,part:&str)->Result<crate::office::Page,String>{if self.view.format=="pdf"{return Err("Not an Office document".into())}crate::office::page(&self.office_source(),offset,query,part)}
 pub fn office_edit(&mut self,engine:&Engine,id:usize,value:&str,kind:&str,style:&Value,revision:u64,cancel:&crate::engine::Cancel)->Result<SessionView,String>{
  if self.view.format=="pdf"||revision!=self.view.revision{return Err("Document changed. Reload before editing.".into())}
  let path=self.dir.join(format!("edit-{}.pdf",Uuid::new_v4()));let source=path.with_extension(&self.view.format);
  let result=(||{crate::office::edit(&self.office_source(),&source,id,value,kind,style)?;
   let temp=tempfile::tempdir_in(&self.dir).map_err(|e|e.to_string())?;
   crate::jobs::office_convert(engine,&source,&path,temp.path(),"pdf",cancel)?;crate::engine::cancelled(cancel)?;engine.info(&path,"")})();
  let info=match result{Ok(v)=>v,Err(e)=>{let _=fs::remove_file(&source);let _=fs::remove_file(&path);return Err(e)}};
  if self.saved_cursor>self.cursor{self.saved_cursor=usize::MAX;}
  for old in self.versions.drain(self.cursor+1..){let _=fs::remove_file(old.with_extension(&self.view.format));let _=fs::remove_file(old);}
  self.versions.push(path);self.cursor+=1;self.view.pages=info["pages"].as_u64().ok_or("Invalid preview")?as usize;self.view.bounds=info["bounds"].clone();self.view.outline=info["outline"].clone();self.trim_history();self.update();Ok(self.view.clone())
 }
 pub fn can_apply(&self,edit:&Value,base:u64)->bool{if base>self.view.revision||base<self.revision_floor{return false}if base==self.view.revision{return true}if edit["kind"]=="widget"{return false}let Some(page)=edit["page"].as_u64() else{return false};page<(self.view.pages as u64)&&self.page_revisions.get(&(page as usize)).copied().unwrap_or(self.revision_floor)<=base}
 pub fn preview_edit(&self,engine:&Engine,mut edit:Value)->Result<String,String>{
  if self.view.format!="pdf"{return Err("Preview requires a PDF document".into())}
  let page=edit["page"].as_u64().ok_or("Missing page")?;
  if page>=self.view.pages as u64{return Err("Page out of range".into())}
  let temp=tempfile::tempdir_in(&self.dir).map_err(|e|e.to_string())?;
  let source=temp.path().join("source.pdf");let edited=temp.path().join("edited.pdf");
  engine.request(json!({"action":"extract","input":self.current(),"password":self.password,"output":source,"options":{"pages":(page+1).to_string()}}),&fresh_cancel())?;
  edit["page"]=json!(0);
  engine.request(json!({"action":"edit","input":source,"output":edited,"edit":edit}),&fresh_cancel())?;
  let image=temp.path().join("preview.png");
  engine.request(json!({"action":"render","input":edited,"page":0,"scale":0.65,"output":image}),&fresh_cancel())?;
  let target=self.dir.join("edit-preview.png");fs::rename(image,&target).map_err(|e|e.to_string())?;
  Ok(target.to_string_lossy().into())
 }
 pub fn edit(&mut self,engine:&Engine,edit:Value)->Result<SessionView,String>{
  if self.view.format!="pdf"{return Err("Use the Office content editor for this document".into())}
  let edited_page=edit["page"].as_u64().unwrap_or(0)as usize;let global=edit["kind"]=="widget";
  let path=self.dir.join(format!("edit-{}.pdf",Uuid::new_v4()));let info=engine.request(json!({"action":"edit","input":self.current(),"password":self.password,"output":path,"edit":edit}),&fresh_cancel())?;
  if self.saved_cursor>self.cursor{self.saved_cursor=usize::MAX;}for old in self.versions.drain(self.cursor+1..){let _=fs::remove_file(old);}self.versions.push(path);self.cursor+=1;
  self.view.bounds=info["bounds"].clone();self.trim_history();self.update();if global{self.revision_floor=self.view.revision;}else{self.page_revisions.insert(edited_page,self.view.revision);}Ok(self.view.clone())
 }
 pub fn adopt(&mut self,engine:&Engine,source:&Path,name:String,revision:u64)->Result<SessionView,String>{
  let info=engine.info(source,"")?;let target=self.dir.join(format!("remote-{}.pdf",Uuid::new_v4()));fs::copy(source,&target).map_err(|e|e.to_string())?;
  for old in self.versions.drain(..){let _=fs::remove_file(old);}self.versions.push(target);self.cursor=0;self.saved_cursor=usize::MAX;self.password.clear();self.view.name=name;self.view.pages=info["pages"].as_u64().ok_or("Invalid snapshot")?as usize;self.view.bounds=info["bounds"].clone();self.view.outline=info["outline"].clone();self.view.revision=revision;self.revision_floor=revision;self.page_revisions.clear();self.view.dirty=true;self.view.can_undo=false;self.view.can_redo=false;
  for entry in fs::read_dir(&self.dir).into_iter().flatten().flatten(){if entry.file_name().to_string_lossy().starts_with("render-"){let _=fs::remove_file(entry.path());}}Ok(self.view.clone())
 }
 fn trim_history(&mut self){let mut bytes:u64=self.versions.iter().filter_map(|p|fs::metadata(p).ok()).map(|m|m.len()).sum();if self.view.format!="pdf"{bytes+=self.versions.iter().filter_map(|p|fs::metadata(p.with_extension(&self.view.format)).ok()).map(|m|m.len()).sum::<u64>();}while self.versions.len()>2&&(self.versions.len()>32||bytes>512*1024*1024){let old=self.versions.remove(1);bytes=bytes.saturating_sub(fs::metadata(&old).map(|m|m.len()).unwrap_or(0));if self.view.format!="pdf"{bytes=bytes.saturating_sub(fs::metadata(old.with_extension(&self.view.format)).map(|m|m.len()).unwrap_or(0));let _=fs::remove_file(old.with_extension(&self.view.format));}let _=fs::remove_file(old);self.cursor-=1;if self.saved_cursor==1{self.saved_cursor=usize::MAX;}else if self.saved_cursor!=usize::MAX&&self.saved_cursor>1{self.saved_cursor-=1;}}}
 pub fn history(&mut self,direction:&str)->Result<SessionView,String>{match direction{"undo" if self.cursor>0=>self.cursor-=1,"redo" if self.cursor+1<self.versions.len()=>self.cursor+=1,_=>return Err("No history in that direction".into())}self.update();self.revision_floor=self.view.revision;Ok(self.view.clone())}
 fn update(&mut self){self.view.revision+=1;self.view.dirty=self.cursor!=self.saved_cursor;self.view.can_undo=self.cursor>0;self.view.can_redo=self.cursor+1<self.versions.len();for e in fs::read_dir(&self.dir).into_iter().flatten().flatten(){if e.file_name().to_string_lossy().starts_with("render-"){let _=fs::remove_file(e.path());}}}
 pub fn render(&self,engine:&Engine,page:usize,scale:f64)->Result<PathBuf,String>{
  if page>=self.view.pages||!scale.is_finite()||!(0.1..=4.0).contains(&scale){return Err("Invalid page or zoom".into())}
  let scale=(scale*20.).round()/20.;let path=self.dir.join(format!("render-{}-{page}-{}.png",self.view.revision,(scale*100.)as u32));
  if !path.exists(){engine.request(json!({"action":"render","input":self.current(),"password":self.password,"page":page,"scale":scale,"output":path}),&fresh_cancel())?;self.trim_cache();}Ok(path)
 }
 fn trim_cache(&self){let mut entries:Vec<_>=fs::read_dir(&self.dir).into_iter().flatten().flatten().filter(|e|e.file_name().to_string_lossy().starts_with("render-")).collect();entries.sort_by_key(|e|e.metadata().and_then(|m|m.modified()).ok());let mut bytes:u64=entries.iter().filter_map(|e|e.metadata().ok()).map(|m|m.len()).sum();for e in entries{if bytes<96*1024*1024{break}if let Ok(m)=e.metadata(){if fs::remove_file(e.path()).is_ok(){bytes=bytes.saturating_sub(m.len());}}}}
 pub fn save_original(&mut self)->Result<(),String>{
  let output=PathBuf::from(&self.view.path);
  if crate::office::format(&output)!=self.view.format{return Err("Legacy document requires Save As in the compatible format.".into())}
  if self.original_stamp.is_none()||file_stamp(&output)!=self.original_stamp{return Err("Original file changed outside BPDF. Use Save As to preserve both versions.".into())}
  let temp=tempfile::NamedTempFile::new_in(output.parent().ok_or("Invalid output path")?).map_err(|e|e.to_string())?;
  fs::copy(if self.view.format=="pdf"{self.current().to_path_buf()}else{self.office_source()},temp.path()).map_err(|e|e.to_string())?;
  fs::set_permissions(temp.path(),fs::metadata(&output).map_err(|e|e.to_string())?.permissions()).map_err(|e|e.to_string())?;
  temp.as_file().sync_all().map_err(|e|e.to_string())?;
  if file_stamp(&output)!=self.original_stamp{return Err("Original file changed during save. Use Save As.".into())}
  temp.persist(&output).map_err(|e|e.to_string())?;
  self.original_stamp=file_stamp(&output);self.saved_cursor=self.cursor;self.view.dirty=false;Ok(())
 }
 pub fn save(&mut self,output:&Path)->Result<(),String>{if crate::office::format(output)!=self.view.format{return Err(format!("Save this document as .{}",self.view.format))}if output.exists(){return Err("Output already exists. Choose a new name to preserve the original.".into())}let parent=output.parent().ok_or("Invalid output path")?;let temp=tempfile::NamedTempFile::new_in(parent).map_err(|e|e.to_string())?;fs::copy(if self.view.format=="pdf"{self.current().to_path_buf()}else{self.office_source()},temp.path()).map_err(|e|e.to_string())?;temp.as_file().sync_all().map_err(|e|e.to_string())?;temp.persist_noclobber(output).map_err(|e|e.to_string())?;self.saved_cursor=self.cursor;self.view.dirty=false;Ok(())}
}
impl Drop for Session{fn drop(&mut self){let _=fs::remove_dir_all(&self.dir);}}

#[cfg(test)]
mod tests {
 use super::*;
 #[test]
 fn history_tracks_the_last_saved_version_and_never_overwrites() {
  let temp=tempfile::tempdir().unwrap();let dir=temp.path().join("session");fs::create_dir(&dir).unwrap();
  let original=dir.join("original.pdf");let edited=dir.join("edited.pdf");fs::write(&original,b"original").unwrap();fs::write(&edited,b"edited").unwrap();
  let mut s=Session{original_stamp:None,view:SessionView{format:"pdf".into(),id:"test".into(),path:original.to_string_lossy().into(),name:"test.pdf".into(),pages:1,bounds:json!([[0,0,100,100]]),outline:json!([]),revision:1,dirty:true,can_undo:true,can_redo:false},dir,versions:vec![original.clone(),edited],cursor:1,password:String::new(),saved_cursor:0,page_revisions:Default::default(),revision_floor:0};
  let output=temp.path().join("copy.pdf");s.save(&output).unwrap();assert!(!s.view.dirty);assert_eq!(fs::read(&output).unwrap(),b"edited");
  assert!(s.history("undo").unwrap().dirty);assert!(!s.history("redo").unwrap().dirty);
  assert!(s.save(&original).is_err());assert_eq!(fs::read(&original).unwrap(),b"original");
  // Explicit Save updates the source atomically; outside modifications stop it.
  s.original_stamp=file_stamp(&original);s.save_original().unwrap();assert_eq!(fs::read(&original).unwrap(),b"edited");assert!(!s.view.dirty);
  fs::write(&original,b"external change").unwrap();assert!(s.save_original().is_err());assert_eq!(fs::read(&original).unwrap(),b"external change");
 }
 #[test]
 fn office_failure_keeps_committed_snapshot_and_cleans_staging(){
  use std::io::Write;
  let temp=tempfile::tempdir().unwrap();let dir=temp.path().join("session");fs::create_dir(&dir).unwrap();let original=dir.join("original.pdf");fs::write(&original,b"preview").unwrap();
  let source=original.with_extension("docx");let mut z=zip::ZipWriter::new(fs::File::create(&source).unwrap());z.start_file("word/document.xml",zip::write::SimpleFileOptions::default()).unwrap();z.write_all(b"<document><r><t>Original</t></r></document>").unwrap();z.finish().unwrap();
  let mut s=Session{original_stamp:None,view:SessionView{format:"docx".into(),id:"test".into(),path:source.to_string_lossy().into(),name:"test.docx".into(),pages:1,bounds:json!([[0,0,100,100]]),outline:json!([]),revision:0,dirty:false,can_undo:false,can_redo:false},dir,versions:vec![original.clone()],cursor:0,password:String::new(),saved_cursor:0,page_revisions:Default::default(),revision_floor:0};
  let engine=Engine{resources:temp.path().join("missing-runtime")};let cancel=fresh_cancel();assert!(s.office_edit(&engine,0,"New","text",&json!({}),0,&cancel).is_err());assert_eq!(s.view.revision,0);assert_eq!(s.office_page(0,"","").unwrap().items[0].text,"Original");assert_eq!(fs::read_dir(&s.dir).unwrap().count(),2);
  cancel.store(true,std::sync::atomic::Ordering::Relaxed);assert!(s.office_edit(&engine,0,"New","text",&json!({}),0,&cancel).is_err());assert_eq!(fs::read_dir(&s.dir).unwrap().count(),2);
 }

}

#[cfg(test)]
mod preview_tests {
 use super::*;
 #[test]
 fn edit_preview_preserves_document_and_reports_overflow() {
  let cache=tempfile::tempdir().unwrap();
  let engine=Engine{resources:PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources")};
  let source=cache.path().join("source.pdf");
  engine.request(json!({"action":"fixture","output":source,"pages":1}),&fresh_cancel()).unwrap();
  let session=Session::open(&engine,cache.path(),&source,String::new()).unwrap();
  let before=fs::read(session.current()).unwrap();
  let edit=json!({"kind":"text","insert":true,"page":0,"rect":[50,180,500,260],"size":18,"text":"Preview only","font":"Helvetica"});
  let image=session.preview_edit(&engine,edit.clone()).unwrap();
  assert!(fs::metadata(image).unwrap().len()>1000);
  assert_eq!(fs::read(session.current()).unwrap(),before);assert!(!session.view.dirty);assert_eq!(session.versions.len(),1);
  let mut overflow=edit;overflow["targetRect"]=json!([50,180,60,185]);
  assert!(session.preview_edit(&engine,overflow).unwrap_err().contains("TEXT_OVERFLOW"));
  assert_eq!(fs::read(session.current()).unwrap(),before);
 }
}
