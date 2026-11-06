//! Bounded local CAJ recovery. Unknown HN encodings fail instead of silently dropping pages.
use std::{fs,io::{Read,Write},path::Path,collections::{BTreeMap,BTreeSet}};
use crate::engine::{Engine,Cancel,cancelled};
use serde_json::json;
fn u32le(b:&[u8],offset:usize)->Result<usize,String>{let a=b.get(offset..offset.checked_add(4).ok_or("Invalid CAJ offset")?).ok_or("Truncated CAJ header")?;Ok(u32::from_le_bytes(a.try_into().unwrap())as usize)}
// The legacy parser needs random access. Bound its allocation and check cancellation
// between chunks rather than letting a large input exhaust the application process.
const MAX_INPUT: usize = 128 * 1024 * 1024;
fn read_bounded(input: &Path, cancel: &Cancel) -> Result<Vec<u8>, String> {
 let mut file=fs::File::open(input).map_err(|e|e.to_string())?;
 if file.metadata().map_err(|e|e.to_string())?.len()>MAX_INPUT as u64{return Err("CAJ exceeds the current 128 MiB parser limit".into())}
 let mut bytes=Vec::new();let mut chunk=[0u8;65536];
 loop {cancelled(cancel)?;let n=file.read(&mut chunk).map_err(|e|e.to_string())?;if n==0{break}if bytes.len()+n>MAX_INPUT{return Err("CAJ exceeds the current 128 MiB parser limit".into())}bytes.try_reserve(n).map_err(|_|"Not enough memory to parse CAJ")?;bytes.extend_from_slice(&chunk[..n]);}
 Ok(bytes)
}
pub fn convert(engine:&Engine,input:&Path,output:&Path,temp:&Path,cancel:&Cancel)->Result<(),String>{
 let bytes=read_bounded(input,cancel)?;cancelled(cancel)?;
 if bytes.starts_with(b"%PDF"){fs::copy(input,output).map_err(|e|e.to_string())?;return Ok(())}
 if bytes.starts_with(b"HN")||bytes.starts_with(&[0xc8]){return hn(engine,&bytes,output,temp,cancel)}
 if !bytes.starts_with(b"CAJ"){return Err("Unsupported or encrypted CAJ variant".into())}
 let count=u32le(&bytes,0x10)?;if count==0||count>20000{return Err("Invalid CAJ page count".into())}
 let pointer=u32le(&bytes,0x14)?;let start=u32le(&bytes,pointer)?;let end=bytes.windows(6).rposition(|w|w==b"endobj").ok_or("No PDF objects in CAJ")?+6;
 let payload=bytes.get(start..end).ok_or("Invalid CAJ PDF offsets")?;
 let obj_re=regex::bytes::Regex::new(r"(?m)(?:^|[\r\n])\s*(\d+)\s+0\s+obj\b").unwrap();let parent_re=regex::bytes::Regex::new(r"/Parent\s+(\d+)\s+0\s+R").unwrap();let page_re=regex::bytes::Regex::new(r"/Type\s*/Page\b").unwrap();let pages_re=regex::bytes::Regex::new(r"/Type\s*/Pages\b").unwrap();
 let captures:Vec<_>=obj_re.captures_iter(payload).take(200001).collect();if captures.len()>200000{return Err("CAJ exceeds the 200,000-object parser limit".into())}let mut used=BTreeSet::new();let mut nodes=BTreeMap::new();let mut actual_pages=0;
 for (i,c)in captures.iter().enumerate(){cancelled(cancel)?;let id=std::str::from_utf8(&c[1]).ok().and_then(|s|s.parse::<usize>().ok()).ok_or("Invalid object ID")?;used.insert(id);let begin=c.get(0).unwrap().end();let end=captures.get(i+1).map(|c|c.get(0).unwrap().start()).unwrap_or(payload.len());let body=&payload[begin..end];let dict_end=body.windows(6).position(|w|w==b"stream").unwrap_or(body.len());let dict=&body[..dict_end];if page_re.is_match(dict)||pages_re.is_match(dict){let parent=parent_re.captures(dict).and_then(|p|std::str::from_utf8(&p[1]).ok().and_then(|s|s.parse::<usize>().ok()));let is_page=page_re.is_match(dict);if is_page{actual_pages+=1;}nodes.insert(id,(parent,is_page));}}
 if actual_pages!=count{return Err(format!("CAJ page validation failed: expected {count}, recovered {actual_pages}"))}
 let mut recovered=Vec::new();let missing:BTreeSet<_>=nodes.values().filter_map(|(p,_)|*p).filter(|p|!used.contains(p)).collect();
 for parent in &missing{let kids=nodes.iter().filter(|(_,v)|v.0==Some(*parent)).map(|(id,_)|format!("{id} 0 R")).collect::<Vec<_>>().join(" ");recovered.extend_from_slice(format!("{parent} 0 obj << /Type /Pages /Kids [{kids}] /Count {count} >> endobj\n").as_bytes());used.insert(*parent);}
 let mut roots:Vec<_>=nodes.iter().filter(|(_,v)|v.0.is_none()).map(|(id,_)|*id).collect();roots.extend(missing);if roots.is_empty(){return Err("CAJ page tree has no recoverable root".into())}
 let root=used.iter().max().copied().unwrap_or(0)+1;let catalog=root+1;let kids=roots.iter().map(|id|format!("{id} 0 R")).collect::<Vec<_>>().join(" ");recovered.extend_from_slice(format!("{root} 0 obj << /Type /Pages /Count {count} /Kids [{kids}] >> endobj\n{catalog} 0 obj << /Type /Catalog /Pages {root} 0 R >> endobj\ntrailer << /Root {catalog} 0 R /Size {} >>\n%%EOF",catalog+1).as_bytes());let raw=temp.join("recovered.pdf");{let mut file=fs::File::create(&raw).map_err(|e|e.to_string())?;file.write_all(b"%PDF-1.7\n").map_err(|e|e.to_string())?;for chunk in payload.chunks(65536){cancelled(cancel)?;file.write_all(chunk).map_err(|e|e.to_string())?;}file.write_all(b"\n").and_then(|_|file.write_all(&recovered)).map_err(|e|e.to_string())?;}engine.request(json!({"action":"repair","input":raw,"output":output}),cancel)?;let info=engine.request(json!({"action":"info","input":output}),cancel)?;if info["pages"].as_u64()!=Some(count as u64){return Err("CAJ conversion lost pages; output rejected".into())}Ok(())
}
fn hn(engine:&Engine,b:&[u8],output:&Path,temp:&Path,cancel:&Cancel)->Result<(),String>{
 let (count,table)=if b[0]==0xc8{(u32le(b,8)?,0x50)}else if b.get(4..6)==Some(&[0xc8,0][..]){(u32le(b,0x90)?,0xd8)}else{let toc=u32le(b,0x158)?;(u32le(b,0x90)?,0x15c+toc.checked_mul(0x134).ok_or("Invalid HN table")?)};
 if count==0||count>20000{return Err("Invalid HN page count".into())}let mut images=Vec::new();
 for i in 0..count{cancelled(cancel)?;let at=table+i*20;let data=u32le(b,at)?;let text=u32le(b,at+4)?;let count_bytes=b.get(at+8..at+10).ok_or("Truncated HN page table")?;let num=u16::from_le_bytes(count_bytes.try_into().unwrap());if num!=1{return Err("This HN variant contains text or multiple image fragments per page and is not yet supported".into())}let header=data.checked_add(text).ok_or("Invalid HN offset")?;let kind=u32le(b,header)?;let image=u32le(b,header+4)?;let length=u32le(b,header+8)?;if kind!=1||image!=header+12{return Err("This HN image encoding is not supported; no partial output was written".into())}let slice=b.get(image..image.checked_add(length).ok_or("Invalid HN image length")?).ok_or("Truncated HN image")?;if !slice.starts_with(&[0xff,0xd8]){return Err("Invalid HN JPEG image".into())}let p=temp.join(format!("hn-{i}.jpg"));fs::write(&p,slice).map_err(|e|e.to_string())?;images.push(p);}
 engine.request(json!({"action":"images_pdf","inputs":images,"output":output}),cancel)?;Ok(())
}
#[cfg(test)]mod tests{use super::*;#[test]fn bounds(){assert!(u32le(&[0;3],0).is_err());assert!(u32le(&[0;10],usize::MAX).is_err());assert_eq!(u32le(&[1,0,0,0],0).unwrap(),1);}}

#[cfg(test)]mod allocation_tests {
 use super::*;use std::sync::{Arc,atomic::AtomicBool};
 #[test]fn oversized_sparse_input_is_rejected_before_allocation(){let f=tempfile::NamedTempFile::new().unwrap();f.as_file().set_len(MAX_INPUT as u64+1).unwrap();let c=Arc::new(AtomicBool::new(false));assert!(read_bounded(f.path(),&c).unwrap_err().contains("128 MiB"));}
 #[test]fn read_can_be_cancelled(){let f=tempfile::NamedTempFile::new().unwrap();let c=Arc::new(AtomicBool::new(true));assert!(read_bounded(f.path(),&c).is_err());}
}
