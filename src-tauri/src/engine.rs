use serde_json::{Value,json};
use std::{fs,io::{Read,Seek,SeekFrom},path::{Path,PathBuf},process::{Command,Stdio},sync::{Arc,atomic::{AtomicBool,Ordering}},thread,time::{Duration,Instant}};
use tempfile::NamedTempFile;

#[derive(Clone)]
pub struct Engine { pub resources:PathBuf }
pub type Cancel = Arc<AtomicBool>;
pub fn fresh_cancel()->Cancel{Arc::new(AtomicBool::new(false))}
pub fn cancelled(c:&Cancel)->Result<(),String>{if c.load(Ordering::Relaxed){Err("CANCELLED".into())}else{Ok(())}}
impl Engine {
 pub fn tool(&self,name:&str)->Result<PathBuf,String>{
  let exe=if cfg!(windows){format!("{name}.exe")}else{name.to_string()};
  let mut candidates=vec![self.resources.join("runtime/bin").join(&exe)];
  if name=="soffice"{candidates.insert(0,if cfg!(target_os="macos"){self.resources.join("runtime/office/Contents/MacOS/soffice")}else{self.resources.join("runtime/office/program/soffice.exe")});}
  candidates.into_iter().find(|p|p.is_file()).ok_or_else(||format!("Missing bundled component: {name}. Run npm run prepare:runtime before building."))
 }
 pub fn command(&self,name:&str,read:&[&Path],write:&[&Path])->Result<Command,String>{
  let mut c=Command::new(self.tool(name)?);
  c.env("BPDF_SCOPE_READ",serde_json::to_string(read).map_err(|e|e.to_string())?);
  c.env("BPDF_SCOPE_WRITE",serde_json::to_string(write).map_err(|e|e.to_string())?);
  c.env("TESSDATA_PREFIX",self.resources.join("runtime/tessdata"));c.env("GS_LIB",std::env::join_paths([self.resources.join("runtime/ghostscript/Resource/Init"),self.resources.join("runtime/ghostscript/lib"),self.resources.join("runtime/ghostscript/Resource/Font"),self.resources.join("runtime/ghostscript/fonts")]).map_err(|e|e.to_string())?);c.env("OMP_THREAD_LIMIT","2");
  if name=="soffice"{let fonts=self.resources.join("runtime/office/Contents/Resources/fontconfig");c.env("FONTCONFIG_FILE",fonts.join("fonts.conf")).env("FONTCONFIG_PATH",fonts);}
  #[cfg(unix)] {use std::os::unix::process::CommandExt;c.process_group(0);}
  #[cfg(windows)]{use std::os::windows::process::CommandExt;c.creation_flags(0x08000000);}
  Ok(c)
 }
 pub fn execute(&self,mut cmd:Command,cancel:&Cancel,timeout:u64)->Result<String,String>{
  cancelled(cancel)?;let mut out=NamedTempFile::new().map_err(|e|e.to_string())?;let mut err=NamedTempFile::new().map_err(|e|e.to_string())?;
  let private_temp=tempfile::tempdir().map_err(|e|e.to_string())?;
  #[cfg(target_os="macos")]
  if cmd.get_envs().any(|(key,_)|key=="BPDF_SCOPE_READ"){
   let scope=|name:&str|->Vec<PathBuf>{cmd.get_envs().find(|(key,_)|*key==name).and_then(|(_,v)|v).and_then(|v|v.to_str()).and_then(|v|serde_json::from_str(v).ok()).unwrap_or_default()};
   let read=scope("BPDF_SCOPE_READ");let mut write=scope("BPDF_SCOPE_WRITE");write.extend([out.path().to_owned(),err.path().to_owned(),private_temp.path().to_owned()]);
   let profile=sandbox_profile(&self.resources,&read,&write)?;
   let mut wrapped=Command::new("/usr/bin/sandbox-exec");wrapped.args(["-p",&profile]).arg(cmd.get_program()).args(cmd.get_args());
   for(key,value)in cmd.get_envs(){if key=="BPDF_SCOPE_READ"||key=="BPDF_SCOPE_WRITE"{continue}if let Some(value)=value{wrapped.env(key,value);}else{wrapped.env_remove(key);}}
   if let Some(dir)=cmd.get_current_dir(){wrapped.current_dir(dir);}
   use std::os::unix::process::CommandExt;wrapped.process_group(0);cmd=wrapped;
  }
  cmd.env_remove("BPDF_SCOPE_READ").env_remove("BPDF_SCOPE_WRITE").env("TMPDIR",private_temp.path()).env("XDG_CACHE_HOME",private_temp.path());
  cmd.stdin(Stdio::null()).stdout(out.reopen().map_err(|e|e.to_string())?).stderr(err.reopen().map_err(|e|e.to_string())?);
  let mut child=cmd.spawn().map_err(|e|e.to_string())?;let started=Instant::now();
  let status=loop{let output_exceeded=out.as_file().metadata().map(|m|m.len()>32*1024*1024).unwrap_or(false)||err.as_file().metadata().map(|m|m.len()>8*1024*1024).unwrap_or(false);if cancel.load(Ordering::Relaxed)||started.elapsed().as_secs()>timeout||output_exceeded{
   #[cfg(unix)] unsafe {libc::kill(-(child.id() as i32),libc::SIGKILL);}
   #[cfg(windows)]{use std::os::windows::process::CommandExt;let _=Command::new("taskkill").args(["/PID",&child.id().to_string(),"/T","/F"]).creation_flags(0x08000000).status();}
   let _=child.kill();let _=child.wait();return Err(if cancel.load(Ordering::Relaxed){"CANCELLED"}else if output_exceeded{"Component output exceeded safety limit"}else{"Operation timed out"}.into());
  }if let Some(s)=child.try_wait().map_err(|e|e.to_string())?{break s;}thread::sleep(Duration::from_millis(30));};
  let mut stdout=String::new();out.seek(SeekFrom::Start(0)).map_err(|e|e.to_string())?;out.take(32*1024*1024).read_to_string(&mut stdout).map_err(|e|e.to_string())?;
  if !status.success(){let mut stderr=String::new();let _=err.seek(SeekFrom::Start(0));let _=err.take(4096).read_to_string(&mut stderr);if let Some(line)=stdout.lines().rev().find(|l|l.starts_with("BPDF_RESULT:")){if let Ok(v)=serde_json::from_str::<Value>(&line[12..]){return Err(v["error"].as_str().unwrap_or("PDF operation failed").into());}}return Err(format!("Component failed ({}): {}",status.code().unwrap_or(-1),stderr.chars().take(1000).collect::<String>()));}
  Ok(stdout)
 }
 pub fn request(&self,request:Value,cancel:&Cancel)->Result<Value,String>{
  if request["action"]=="render" && self.tool("bpdf-render").is_ok(){
   let password=NamedTempFile::new().map_err(|e|e.to_string())?;fs::write(password.path(),request["password"].as_str().unwrap_or("")).map_err(|e|e.to_string())?;
   let input=Path::new(request["input"].as_str().ok_or("Missing render input")?);let output=Path::new(request["output"].as_str().ok_or("Missing render output")?);let mut cmd=self.command("bpdf-render",&[input,password.path()],&[output])?;cmd.arg(request["input"].as_str().ok_or("Missing render input")?).arg(password.path()).arg(request["page"].to_string()).arg(request["scale"].to_string()).arg(request["output"].as_str().ok_or("Missing render output")?);self.execute(cmd,cancel,120)?;return Ok(json!({}));
  }
  let file=NamedTempFile::new().map_err(|e|e.to_string())?;fs::write(file.path(),serde_json::to_vec(&request).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
  let mut reads=vec![file.path().to_owned()];let mut writes=Vec::new();
  if let Some(input)=request["input"].as_str(){reads.push(PathBuf::from(input));}
  if let Some(inputs)=request["inputs"].as_array(){for input in inputs{if let Some(input)=input.as_str(){reads.push(PathBuf::from(input));}}}
  if let Some(image)=request["edit"]["path"].as_str(){reads.push(PathBuf::from(image));}
  if let Some(base)=request["resourceRoot"].as_str(){reads.push(PathBuf::from(base));}
  for key in ["output","assetDir"]{if let Some(path)=request[key].as_str(){writes.push(PathBuf::from(path));}}
  let mut cmd=self.command("mutool",&reads.iter().map(PathBuf::as_path).collect::<Vec<_>>(),&writes.iter().map(PathBuf::as_path).collect::<Vec<_>>())?;cmd.arg("run").arg(self.resources.join("engine/worker.js")).arg(file.path());
  let out=self.execute(cmd,cancel,1800)?;let line=out.lines().rev().find_map(|l|l.strip_prefix("BPDF_RESULT:")).ok_or_else(||format!("Engine returned no result: {}",out.chars().take(200).collect::<String>()))?;
  let v:Value=serde_json::from_str(line).map_err(|e|e.to_string())?;if v["ok"]==true{Ok(v["result"].clone())}else{Err(v["error"].as_str().unwrap_or("PDF operation failed").into())}
 }
 pub fn info(&self,path:&Path,password:&str)->Result<Value,String>{self.request(json!({"action":"info","input":path,"password":password}),&fresh_cancel())}
}

#[cfg(target_os="macos")]
fn sandbox_profile(resources:&Path,read:&[PathBuf],write:&[PathBuf])->Result<String,String>{
 fn rule(path:&Path)->Result<String,String>{
  let resolved=if path.exists(){path.canonicalize()}else{path.parent().ok_or("Missing output directory")?.canonicalize().map(|p|p.join(path.file_name().unwrap_or_default()))}.map_err(|e|e.to_string())?;
  let kind=if resolved.is_dir(){"subpath"}else{"literal"};
  Ok(format!("({kind} {})",serde_json::to_string(&resolved.to_string_lossy()).map_err(|e|e.to_string())?))
 }
 let mut profile=String::from("(version 1)(allow default)(deny network*)(deny file-read-data)(deny file-write*)(allow file-read-data (literal \"/\") (subpath \"/System\") (subpath \"/usr\") (subpath \"/Library/Fonts\") (subpath \"/dev\") (subpath \"/private/etc\"))(allow file-write-data (literal \"/dev/null\"))");
 let mut inputs=vec![resources.to_owned()];inputs.extend_from_slice(read);inputs.extend_from_slice(write);
 if let Some(home)=std::env::var_os("HOME"){let fonts=PathBuf::from(home).join("Library/Fonts");if fonts.exists(){inputs.push(fonts);}}
 for path in inputs{profile+=&format!("(allow file-read-data {})",rule(&path)?);let mut parent=path.parent();while let Some(dir)=parent{if let Ok(dir)=dir.canonicalize(){profile+=&format!("(allow file-read-data (literal {}))",serde_json::to_string(&dir.to_string_lossy()).map_err(|e|e.to_string())?);}parent=dir.parent();}}
 for path in write{profile+=&format!("(allow file-write* {})",rule(path)?);}
 Ok(profile)
}

pub fn selected_pages(spec:&str,count:usize,allow_all:bool)->Result<Vec<usize>,String>{
 if spec.trim().is_empty()||spec=="all"{return if allow_all{Ok((0..count).collect())}else{Err("Select at least one page".into())};}
 let mut out=Vec::new();for part in spec.split(','){let part=part.trim();let nums:Vec<_>=part.split('-').collect();if nums.len()>2{return Err("Invalid page range".into())}let a:usize=nums[0].trim().parse().map_err(|_|"Invalid page range")?;let b=if nums.len()==2{nums[1].trim().parse().map_err(|_|"Invalid page range")?}else{a};if a==0||b==0||a>count||b>count{return Err("Page out of range".into())}if a<=b{out.extend((a-1)..b)}else{out.extend(((b-1)..a).rev())}if out.len()>20000{return Err("Too many pages".into())}}
 Ok(out)
}

#[cfg(test)]mod tests{use super::*;#[test]fn ranges(){assert_eq!(selected_pages("3-1,2",3,true).unwrap(),vec![2,1,0,1]);assert!(selected_pages("0",3,true).is_err());assert!(selected_pages("4",3,true).is_err());assert!(selected_pages("",3,false).is_err());assert!(selected_pages("1--2",3,true).is_err());}}

#[cfg(all(test,unix))]
mod process_tests {
 use super::*;
 #[test]
 fn cancellation_stops_running_process_promptly() {
  use std::os::unix::process::CommandExt;
  let engine=Engine{resources:PathBuf::new()};let cancel=fresh_cancel();let signal=cancel.clone();
  let thread=thread::spawn(move||{thread::sleep(Duration::from_millis(120));signal.store(true,Ordering::Relaxed);});
  let mut command=Command::new("/bin/sleep");command.arg("30").process_group(0);let start=Instant::now();
  assert_eq!(engine.execute(command,&cancel,60).unwrap_err(),"CANCELLED");assert!(start.elapsed()<Duration::from_secs(3));thread.join().unwrap();
 }
}

#[cfg(all(test,target_os="macos"))]
mod isolation_tests {
 use super::*;
 #[test]
 fn worker_cannot_read_unselected_files_or_follow_escaping_symlinks() {
  let engine=Engine{resources:PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources")};
  let allowed=tempfile::tempdir().unwrap();let outside=tempfile::tempdir().unwrap();
  let secret=outside.path().join("unselected.txt");fs::write(&secret,"unselected content").unwrap();
  let link=allowed.path().join("escape.txt");std::os::unix::fs::symlink(&secret,&link).unwrap();
  let input=allowed.path().join("input.txt");fs::write(&input,"selected content").unwrap();
  let output=allowed.path().join("result.txt");let forbidden=outside.path().join("result.txt");
  let script=allowed.path().join("probe.js");
  fs::write(&script,format!(r#"
   var selected=readFile({input});selected.save({output});
   function denied(path,write){{try{{if(write)selected.save(path);else readFile(path);return false;}}catch(e){{return true;}}}}
   if(!denied({secret},false)||!denied({link},false)||!denied({forbidden},true))quit(1);
   print('isolation passed');
  "#,input=json!(input),output=json!(output),secret=json!(secret),link=json!(link),forbidden=json!(forbidden))).unwrap();
  let mut command=engine.command("mutool",&[allowed.path()],&[allowed.path()]).unwrap();
  command.arg("run").arg(&script);
  assert!(engine.execute(command,&fresh_cancel(),30).unwrap().contains("isolation passed"));
  assert_eq!(fs::read(&output).unwrap(),b"selected content");assert!(!forbidden.exists());
 }
}
