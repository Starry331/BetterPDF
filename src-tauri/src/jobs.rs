use crate::{engine::{Engine,Cancel,cancelled,selected_pages},export};
use serde::{Serialize,Deserialize};
use serde_json::{Value,json};
use std::{fs,path::{Path,PathBuf}};
use tempfile::TempDir;
#[derive(Clone,Serialize,Deserialize)]
pub struct Task{pub id:String,pub tool:String,pub state:String,pub progress:f64,pub message:String,pub outputs:Vec<String>,pub warnings:Vec<String>}
pub const TOOLS:&[&str]=&["merge","split","rotate","delete","extract","reorder","resize","crop","to_docx","to_pptx","to_xlsx","to_images","to_pdfa","to_txt","to_md","to_epub","word_pdf","ppt_pdf","excel_pdf","images_pdf","epub_pdf","caj_pdf","file_pdf","compress","repair","ocr","grayscale","watermark","remove_watermark","protect","unlock","rasterize","flatten","file_images"];
fn ext(p:&Path)->String{p.extension().unwrap_or_default().to_string_lossy().to_lowercase()}
fn str_opt<'a>(v:&'a Value,k:&str,default:&'a str)->&'a str{v[k].as_str().unwrap_or(default)}
pub fn validate(tool:&str,inputs:&[PathBuf],options:&Value)->Result<(),String>{
 if !TOOLS.contains(&tool){return Err("Unknown tool".into())}if inputs.is_empty(){return Err("Choose at least one file".into())}if inputs.len()>20000{return Err("Too many input files".into())}
 let formats:&[&str]=match tool{"word_pdf"=>&["doc","docx"],"ppt_pdf"=>&["ppt","pptx"],"excel_pdf"=>&["xls","xlsx"],"images_pdf"=>&["png","jpg","jpeg","tif","tiff","bmp","webp"],"epub_pdf"=>&["epub"],"caj_pdf"=>&["caj","nh"],"file_pdf"|"file_images"=>&["pdf","doc","docx","ppt","pptx","xls","xlsx","png","jpg","jpeg","tif","tiff","bmp","webp","txt","md","html","htm","epub","caj","nh"],_=>&["pdf"]};
 for p in inputs{if !p.is_file(){return Err("An input file is missing".into())}if !formats.contains(&ext(p).as_str()){return Err(format!("Unsupported input format for {tool}: {}",ext(p)))}}
 let numeric=|key:&str,min:f64,max:f64|->Result<(),String>{if let Some(value)=options.get(key){let n=value.as_f64().ok_or_else(||format!("Invalid {key}"))?;if !n.is_finite()||n<min||n>max{return Err(format!("{key} must be between {min} and {max}"))}}Ok(())};
 if ["ocr","to_images","file_images","rasterize","grayscale"].contains(&tool){numeric("dpi",if tool=="ocr"{72.}else{36.},if tool=="ocr"{288.}else{600.})?;}
 if tool=="split"{numeric("chunk",1.,20000.)?;if options.get("chunk").is_some()&&options["chunk"].as_u64().is_none(){return Err("Pages per file must be an integer".into())}}
 if tool=="resize"{numeric("width",36.,14400.)?;numeric("height",36.,14400.)?;}
 if tool=="crop"{for key in ["left","top","right","bottom"]{numeric(key,0.,10000.)?;}}
 if tool=="rotate"{numeric("angle",-360.,360.)?;if options.get("angle").is_some()&&options["angle"].as_f64().unwrap()%90.!=0.{return Err("Rotation must be a multiple of 90".into())}}
 if tool=="watermark"{numeric("size",4.,200.)?;numeric("opacity",0.,1.)?;}
 if tool=="compress"&&! ["lossless","screen","balanced","print"].contains(&str_opt(options,"quality","lossless")){return Err("Invalid compression preset".into())}
 if tool=="grayscale"&&! ["gray","mono"].contains(&str_opt(options,"colorMode","gray")){return Err("Invalid color mode".into())}
 Ok(())
}
fn export_images(engine:&Engine,input:&Path,output:&Path,options:&Value,cancel:&Cancel,progress:&dyn Fn(f64,&str))->Result<(),String>{
 let password=str_opt(options,"password","");let count=engine.request(json!({"action":"info","input":input,"password":password}),cancel)?["pages"].as_u64().ok_or("Invalid page count")?as usize;
 let selected=selected_pages(str_opt(options,"pages",""),count,true)?;fs::create_dir_all(output).map_err(|e|e.to_string())?;
 for (index,page)in selected.iter().enumerate(){cancelled(cancel)?;let mut page_options=options.clone();page_options["pages"]=json!((page+1).to_string());engine.request(json!({"action":"to_images","input":input,"password":password,"output":output,"options":page_options}),cancel)?;progress((index+1)as f64/selected.len()as f64,"Exporting page images");}Ok(())
}
fn office(engine:&Engine,input:&Path,output:&Path,temp:&Path,cancel:&Cancel)->Result<(),String>{office_convert(engine,input,output,temp,"pdf",cancel)}
pub fn office_convert(engine:&Engine,input:&Path,output:&Path,temp:&Path,format:&str,cancel:&Cancel)->Result<(),String>{
 static OFFICE_LOCK:std::sync::Mutex<()>=std::sync::Mutex::new(());
 let _guard=loop{crate::engine::cancelled(cancel)?;match OFFICE_LOCK.try_lock(){Ok(g)=>break g,Err(std::sync::TryLockError::WouldBlock)=>std::thread::sleep(std::time::Duration::from_millis(40)),Err(_)=>return Err("Office worker unavailable".into())}};
 if !["pdf","docx","xlsx","pptx"].contains(&format){return Err("Unsupported Office output".into())}
 let profile=temp.join(format!("office-profile-{}",uuid::Uuid::new_v4()));fs::create_dir_all(profile.join("user")).map_err(|e|e.to_string())?;
 // Never activate macros, external links or document JavaScript.
 fs::write(profile.join("user/registrymodifications.xcu"),r#"<?xml version="1.0"?><oor:items xmlns:oor="http://openoffice.org/2001/registry"><item oor:path="/org.openoffice.Office.Common/Security/Scripting"><prop oor:name="MacroSecurityLevel" oor:op="fuse"><value>3</value></prop><prop oor:name="DisableMacrosExecution" oor:op="fuse"><value>true</value></prop></item><item oor:path="/org.openoffice.Office.Common/Misc"><prop oor:name="UseSystemFileDialog" oor:op="fuse"><value>false</value></prop></item></oor:items>"#).map_err(|e|e.to_string())?;
 let dir=temp.join(format!("office-output-{}",uuid::Uuid::new_v4()));fs::create_dir(&dir).map_err(|e|e.to_string())?;
 let mut cmd=engine.command("soffice",&[input,temp],&[temp])?;let url=tauri::Url::from_directory_path(&profile).map_err(|_|"Invalid Office profile path")?.to_string();cmd.arg(format!("-env:UserInstallation={url}")).args(["--headless","--nologo","--nodefault","--nolockcheck","--norestore","--convert-to",format,"--outdir"]).arg(&dir).arg(input);
 engine.execute(cmd,cancel,300)?;let result=dir.join(input.file_stem().unwrap_or_default()).with_extension(format);if !result.exists(){return Err("Office conversion produced no output".into())}fs::copy(result,output).map_err(|e|e.to_string())?;Ok(())
}
fn convert_pdf(engine:&Engine,input:&Path,output:&Path,temp:&Path,options:&Value,cancel:&Cancel)->Result<Vec<String>,String>{
 let suffix=ext(input);match suffix.as_str(){
 "pdf"=>{engine.request(json!({"action":"unlock","input":input,"password":str_opt(options,"password",""),"output":output}),cancel)?;},
 "doc"|"docx"|"ppt"|"pptx"|"xls"|"xlsx"=>{office(engine,input,output,temp,cancel)?;return Ok(vec!["Office pagination depends on available fonts. Review font substitutions and spreadsheet print areas.".into()]);},
 "png"|"jpg"|"jpeg"|"bmp"|"webp"|"tif"|"tiff"=>{engine.request(json!({"action":"convert","input":input,"output":output}),cancel)?;},
 "epub"=>{engine.request(json!({"action":"convert","input":input,"output":output}),cancel)?;},
 "html"|"htm"=>{engine.request(json!({"action":"convert","input":input,"output":output,"resourceRoot":input.parent().ok_or("Missing HTML resource directory")?}),cancel)?;},
 "txt"|"md"=>{
  let text=fs::read_to_string(input).map_err(|_|"Text input must use UTF-8 encoding")?;
  let html=if suffix=="txt"{format!("<html><head><meta charset='UTF-8'></head><body><pre>{}</pre></body></html>",text.replace('&',"&amp;").replace('<',"&lt;").replace('>',"&gt;"))}else if suffix=="md"{let mut body=String::new();pulldown_cmark::html::push_html(&mut body,pulldown_cmark::Parser::new(&text));format!("<html><head><meta charset='UTF-8'></head><body>{body}</body></html>")}else{text};
  // MuPDF's HTML renderer has no networking or JavaScript execution.
  let local=temp.join(format!("text-{}.html",uuid::Uuid::new_v4()));fs::write(&local,html).map_err(|e|e.to_string())?;engine.request(json!({"action":"convert","input":local,"output":output}),cancel)?;
 },
 "caj"|"nh"=>{crate::caj::convert(engine,input,output,temp,cancel)?;},
 _=>return Err(format!("Unsupported input format: {suffix}"))
 }
 let _=options;Ok(vec![])
}
fn gs(engine:&Engine,input:&Path,output:&Path,args:&[String],cancel:&Cancel)->Result<(),String>{let mut cmd=engine.command("gs",&[input],&[output])?;cmd.args(["-dSAFER","-dBATCH","-dNOPAUSE","-dQUIET","-sDEVICE=pdfwrite"]).args(args).arg(format!("-sOutputFile={}",output.display())).arg(input);engine.execute(cmd,cancel,900)?;Ok(())}
fn one(engine:&Engine,tool:&str,inputs:&[PathBuf],output:&Path,temp:&Path,options:&Value,cancel:&Cancel,progress:&dyn Fn(f64,&str))->Result<Vec<String>,String>{
 cancelled(cancel)?;let input=&inputs[0];let password=str_opt(options,"password","");let mut warnings=Vec::new();
 match tool{
 "word_pdf"|"ppt_pdf"|"excel_pdf"|"epub_pdf"|"caj_pdf"|"file_pdf"=>{warnings.extend(convert_pdf(engine,input,output,temp,options,cancel)?);},
 "to_docx"|"to_pptx"|"to_xlsx"|"to_md"|"to_txt"|"to_epub"=>{let assets=temp.join("assets");fs::create_dir_all(&assets).map_err(|e|e.to_string())?;let layout=engine.request(json!({"action":"layout","input":input,"password":password,"assetDir":assets}),cancel)?;progress(0.65,"Rebuilding editable content");cancelled(cancel)?;warnings.extend(export::export(&layout,tool,output)?);},
 "split"=>{fs::create_dir_all(output).map_err(|e|e.to_string())?;let count=engine.request(json!({"action":"info","input":input,"password":password}),cancel)?["pages"].as_u64().ok_or("Invalid page count")? as usize;let chunk=options["chunk"].as_u64().unwrap_or(1)as usize;if chunk==0{return Err("Pages per file must be positive".into())}for start in (0..count).step_by(chunk){cancelled(cancel)?;let end=(start+chunk).min(count);let dest=output.join(format!("pages-{:05}-{:05}.pdf",start+1,end));engine.request(json!({"action":"extract","input":input,"password":password,"output":dest,"options":{"pages":format!("{}-{}",start+1,end)}}),cancel)?;progress(end as f64/count as f64,"Splitting pages");}},
 "to_images"=>{export_images(engine,input,output,options,cancel,progress)?;},
 "file_images"=>{let converted=temp.join("converted.pdf");warnings.extend(convert_pdf(engine,input,&converted,temp,options,cancel)?);export_images(engine,&converted,output,options,cancel,progress)?;},
 "ocr"=>{
  let count=engine.request(json!({"action":"info","input":input,"password":password}),cancel)?["pages"].as_u64().ok_or("Invalid page count")? as usize;
  let lang=str_opt(options,"language","chi_sim+chi_tra+eng");if !["chi_sim+chi_tra+eng","chi_sim+eng","chi_tra+eng","eng"].contains(&lang){return Err("Unsupported OCR language".into())}
  let dpi=options["dpi"].as_f64().unwrap_or(200.);if !(72.0..=288.0).contains(&dpi){return Err("OCR resolution must be 72–288 DPI".into())}
  let auto_rotate=options["autoRotate"].as_bool().unwrap_or(true);let mut orientation_uncertain=false;
  let mut pages=Vec::new();for i in 0..count{cancelled(cancel)?;let image=temp.join(format!("ocr-{i}.png"));let base=temp.join(format!("ocr-{i}"));engine.request(json!({"action":"render","input":input,"password":password,"page":i,"scale":dpi/72.,"output":image}),cancel)?;
   if auto_rotate{let mut detect=engine.command("tesseract",&[&image],&[temp])?;detect.arg(&image).args(["stdout","-l","osd","--dpi",&dpi.round().to_string(),"--psm","0"]);
    match engine.execute(detect,cancel,90){Ok(result)=>{let value=|key:&str|result.lines().find_map(|line|line.strip_prefix(key)).and_then(|v|v.trim().parse::<f64>().ok());let confidence=value("Orientation confidence:").unwrap_or(0.);let angle=value("Rotate:").unwrap_or(0.);if confidence>=5.&&[90.,180.,270.].contains(&angle){let source=temp.join("orientation-source.pdf");let rotated=temp.join("orientation-rotated.pdf");engine.request(json!({"action":"extract","input":input,"password":password,"output":source,"options":{"pages":(i+1).to_string()}}),cancel)?;engine.request(json!({"action":"rotate","input":source,"output":rotated,"options":{"angle":angle}}),cancel)?;engine.request(json!({"action":"render","input":rotated,"page":0,"scale":dpi/72.,"output":image}),cancel)?;let _=fs::remove_file(source);let _=fs::remove_file(rotated);}else if confidence<5.{orientation_uncertain=true;}},Err(e)=>{cancelled(cancel)?;if e=="Operation timed out"{return Err(e)}orientation_uncertain=true;}}
   }
   let mut cmd=engine.command("tesseract",&[&image],&[temp])?;cmd.arg(&image).arg(&base).args(["-l",lang,"--dpi",&dpi.round().to_string(),"--psm","3","pdf"]);engine.execute(cmd,cancel,300)?;pages.push(base.with_extension("pdf"));let _=fs::remove_file(image);progress((i+1)as f64/count as f64*0.9,"Recognizing text");}
  if orientation_uncertain{warnings.push("Orientation could not be determined confidently on some pages; their original direction was preserved. Review those pages.".into());}
  engine.request(json!({"action":"merge","inputs":pages,"output":output}),cancel)?;warnings.push("OCR rebuilds pages as images with searchable text. Review recognition accuracy; existing interactive forms are not retained.".into());
 },
 "to_pdfa"=>{let decrypted=temp.join("input.pdf");engine.request(json!({"action":"unlock","input":input,"password":password,"output":decrypted}),cancel)?;
  let icc=engine.resources.join("runtime/ghostscript/iccprofiles/srgb.icc");if !icc.exists(){return Err("Missing bundled PDF/A color profile".into())}let profile=temp.join("PDFA.ps");let iccstr=icc.to_string_lossy().replace('\\',"\\\\").replace('(',"\\(").replace(')',"\\)");
  fs::write(&profile,format!("/ICCProfile ({iccstr}) def\n[/_objdef {{icc_PDFA}} /type /stream /OBJ pdfmark\n[{{icc_PDFA}} << /N 3 >> /PUT pdfmark\n[{{icc_PDFA}} ICCProfile (r) file /PUT pdfmark\n[/_objdef {{OutputIntent_PDFA}} /type /dict /OBJ pdfmark\n[{{OutputIntent_PDFA}} << /Type /OutputIntent /S /GTS_PDFA1 /DestOutputProfile {{icc_PDFA}} /OutputConditionIdentifier (sRGB) >> /PUT pdfmark\n[{{Catalog}} << /OutputIntents [{{OutputIntent_PDFA}}] >> /PUT pdfmark\n")).map_err(|e|e.to_string())?;
  let mut cmd=engine.command("gs",&[&profile,&decrypted],&[output])?;cmd.args(["-dSAFER","-dBATCH","-dNOPAUSE","-dQUIET","-sDEVICE=pdfwrite","-dPDFA=2","-dPDFACompatibilityPolicy=2","-sColorConversionStrategy=RGB"]).arg(format!("--permit-file-read={}",icc.display())).arg(format!("-sOutputFile={}",output.display())).arg(profile).arg(decrypted);engine.execute(cmd,cancel,900)?;warnings.push("PDF/A-2b output generated. Independent veraPDF validation is required for archival certification.".into());
 },
 "compress"=>{
  let quality=str_opt(options,"quality","lossless");let unlocked=temp.join("unlocked.pdf");engine.request(json!({"action":"unlock","input":input,"password":password,"output":unlocked}),cancel)?;
  if quality=="lossless"{let mut cmd=engine.command("qpdf",&[&unlocked],&[output])?;cmd.args(["--object-streams=generate","--recompress-flate","--compression-level=9"]).arg(&unlocked).arg(output);engine.execute(cmd,cancel,600)?;}else{let preset=match quality{"screen"=>"/screen","balanced"=>"/ebook","print"=>"/printer",_=>return Err("Invalid compression preset".into())};gs(engine,&unlocked,output,&[format!("-dPDFSETTINGS={preset}")],cancel)?;warnings.push("Lossy compression may change image quality, forms and annotations.".into());}
  if fs::metadata(output).map_err(|e|e.to_string())?.len()>=fs::metadata(input).map_err(|e|e.to_string())?.len(){warnings.push("This document could not be made smaller with the selected preset.".into());}
 },
 "grayscale" if str_opt(options,"colorMode","gray")=="mono"=>{
  let decrypted=temp.join("mono-input.pdf");engine.request(json!({"action":"unlock","input":input,"password":password,"output":decrypted}),cancel)?;
  let count=engine.request(json!({"action":"info","input":input,"password":password}),cancel)?["pages"].as_u64().unwrap_or(0)as usize;let selected=selected_pages(str_opt(options,"pages",""),count,true)?;let mut paths=Vec::new();for i in selected{let out=temp.join(format!("mono-{i}.pbm"));let mut cmd=engine.command("mutool",&[&decrypted],&[&out])?;cmd.args(["draw","-F","pbm","-c","mono","-r",&options["dpi"].as_u64().unwrap_or(144).to_string(),"-o"]).arg(&out).arg(&decrypted).arg((i+1).to_string());engine.execute(cmd,cancel,300)?;paths.push(out);}engine.request(json!({"action":"images_pdf","inputs":paths,"output":output}),cancel)?;
 },
 _=>{engine.request(json!({"action":tool,"inputs":inputs,"password":password,"output":output,"options":options}),cancel)?;}
 }
 if output.is_file()&&ext(output)=="pdf"{engine.request(json!({"action":"info","input":output,"password":if tool=="protect"{str_opt(options,"newPassword","")}else{""}}),cancel)?;}
 Ok(warnings)
}
pub fn run(engine:&Engine,tool:&str,inputs:&[PathBuf],output:&Path,options:&Value,cancel:&Cancel,progress:&dyn Fn(f64,&str))->Result<(Vec<String>,Vec<String>),String>{
 validate(tool,inputs,options)?;cancelled(cancel)?;
 if output.exists(){return Err("Output already exists. Choose a new name.".into())}let parent=output.parent().ok_or("Invalid destination")?;if !parent.is_dir(){return Err("Destination folder does not exist".into())}
 let work=TempDir::new_in(parent).map_err(|e|e.to_string())?;let stage=work.path().join(output.file_name().ok_or("Invalid destination name")?);
 let batch=inputs.len()>1&&tool!="merge"&&tool!="images_pdf";let mut warnings=Vec::new();
 if batch{fs::create_dir(&stage).map_err(|e|e.to_string())?;for (i,input)in inputs.iter().enumerate(){let suffix=match tool{"to_docx"=>"docx","to_pptx"=>"pptx","to_xlsx"=>"xlsx","to_txt"=>"txt","to_md"=>"md","to_epub"=>"epub","file_images"|"to_images"|"split"=>"",_=>"pdf"};let stem=format!("{:03}-{}",i+1,input.file_stem().unwrap_or_default().to_string_lossy());let dest=if suffix.is_empty(){stage.join(stem)}else{stage.join(format!("{stem}.{suffix}"))};let per=tempfile::tempdir_in(work.path()).map_err(|e|e.to_string())?;warnings.extend(one(engine,tool,&[input.clone()],&dest,per.path(),options,cancel,&|p,m|progress((i as f64+p)/inputs.len()as f64,m))?);}}
 else{warnings.extend(one(engine,tool,inputs,&stage,work.path(),options,cancel,progress)?);}
 cancelled(cancel)?;
 if stage.is_file(){fs::File::open(&stage).and_then(|f|f.sync_all()).map_err(|e|e.to_string())?;if fs::hard_link(&stage,output).is_err(){let f=tempfile::NamedTempFile::new_in(parent).map_err(|e|e.to_string())?;fs::copy(&stage,f.path()).map_err(|e|e.to_string())?;f.as_file().sync_all().map_err(|e|e.to_string())?;f.persist_noclobber(output).map_err(|e|e.to_string())?;}}else if stage.is_dir(){fs::create_dir(output).map_err(|e|e.to_string())?;for entry in fs::read_dir(&stage).map_err(|e|e.to_string())?{let entry=entry.map_err(|e|e.to_string())?;if let Err(e)=fs::rename(entry.path(),output.join(entry.file_name())){let _=fs::remove_dir_all(output);return Err(e.to_string())}}}else{return Err("No output produced".into())}
 warnings.sort();warnings.dedup();Ok((vec![output.to_string_lossy().into()],warnings))
}

pub fn preview(engine:&Engine,tool:&str,inputs:&[PathBuf],options:&Value,cache:&Path)->Result<Value,String>{
 validate(tool,inputs,options)?;let password=str_opt(options,"password","");
 if ext(&inputs[0])!="pdf"{return Ok(json!({"pages":0,"selected":inputs.len(),"message":"Input files will be converted locally. Review the output before sharing."}));}
 let cancel=crate::engine::fresh_cancel();let info=engine.info(&inputs[0],password)?;let count=info["pages"].as_u64().ok_or("Invalid page count")?as usize;let selected=selected_pages(str_opt(options,"pages",""),count,tool!="delete")?;let mut first=*selected.first().ok_or("No pages to preview")?;let mut affected=0;let mut instances=0;
 if tool=="remove_watermark"{let result=engine.request(json!({"action":"watermark_matches","input":inputs[0],"password":password,"options":options}),&cancel)?;let matched=result["matches"].as_array().ok_or("Invalid watermark results")?;if matched.is_empty(){return Err("No matching independent watermark found".into())}first=matched[0]["page"].as_u64().ok_or("Invalid matched page")?as usize;affected=matched.len();instances=result["count"].as_u64().unwrap_or(0);}

 let dir=tempfile::Builder::new().prefix("preview-").tempdir_in(cache).map_err(|e|e.to_string())?;let image=dir.path().join("preview.png");
 let visual=["rotate","crop","resize","watermark","remove_watermark","grayscale","rasterize"];
 if visual.contains(&tool){
  let single=dir.path().join("source.pdf");engine.request(json!({"action":"extract","input":inputs[0],"password":password,"output":single,"options":{"pages":(first+1).to_string()}}),&cancel)?;
  let pdf=dir.path().join("preview.pdf");let mut opts=options.clone();opts["pages"]=json!("1");opts["password"]=json!("");
  one(engine,tool,&[single.clone()],&pdf,dir.path(),&opts,&cancel,&|_,_|{})?;
  engine.request(json!({"action":"render","input":pdf,"page":0,"scale":0.65,"output":image}),&cancel)?;
 }else{engine.request(json!({"action":"render","input":inputs[0],"password":password,"page":first,"scale":0.65,"output":image}),&cancel)?;}
 // Keep only the small preview image. Intermediate PDFs disappear with the TempDir.
 let kept=cache.join(format!("preview-image-{}.png",uuid::Uuid::new_v4()));fs::rename(&image,&kept).map_err(|e|e.to_string())?;
 trim_previews(cache,&kept);
 Ok(json!({"pages":count,"selected":selected.len(),"affected":affected,"instances":instances,"message":if visual.contains(&tool){"First selected page after processing"}else{"Source page preview; conversion fidelity must be checked in the output"},"image":kept}))
}

pub fn watermark_candidates(engine:&Engine,input:&Path,password:&str,page:usize,cache:&Path)->Result<Value,String>{
 validate("remove_watermark",&[input.to_path_buf()],&json!({}))?;let cancel=crate::engine::fresh_cancel();let mut data=engine.request(json!({"action":"watermark_candidates","input":input,"password":password,"page":page}),&cancel)?;
 let image=cache.join(format!("preview-image-{}.png",uuid::Uuid::new_v4()));engine.request(json!({"action":"render","input":input,"password":password,"page":page,"scale":0.65,"output":image}),&cancel)?;trim_previews(cache,&image);data["image"]=json!(image);Ok(data)
}

fn trim_previews(cache:&Path,kept:&Path){let mut old:Vec<_>=fs::read_dir(cache).into_iter().flatten().flatten().filter(|e|e.file_name().to_string_lossy().starts_with("preview-image-")&&e.path()!=kept).collect();old.sort_by_key(|e|e.metadata().and_then(|m|m.modified()).ok());let remove=old.len().saturating_sub(7);for e in old.into_iter().take(remove){let _=fs::remove_file(e.path());}}
