use serde_json::Value;
use std::{fs,io::Write,path::Path};
use zip::{ZipWriter,write::SimpleFileOptions,CompressionMethod};
use base64::{Engine as _,engine::general_purpose::STANDARD};
fn xml(s:&str)->String{s.chars().filter(|c|*c=='\n'||*c=='\t'||*c=='\r'||*c>=' ').collect::<String>().replace('&',"&amp;").replace('<',"&lt;").replace('>',"&gt;").replace('"',"&quot;").replace('\'',"&apos;")}
fn n(v:&Value)->f64{v.as_f64().unwrap_or(0.)}
fn color(line:&Value)->String{
 (0..3).map(|i|format!("{:02X}",(n(&line["color"][i]).clamp(0.,1.)*255.).round()as u8)).collect()
}
// Recursive whitespace cuts reconstruct columns without alternating left/right
// lines. Work is bounded to 16 levels; rotated/invalid geometry keeps source order.
fn reading_order(layout:&Value)->Value{
 fn gap(lines:&[Value],axis:usize)->Option<(f64,f64)>{
  let mut intervals:Vec<_>=lines.iter().map(|l|(n(&l["rect"][axis]),n(&l["rect"][axis+2]))).collect();
  intervals.sort_by(|a,b|a.0.total_cmp(&b.0));let mut end=intervals.first()?.1;let mut best=None;
  for(start,next_end)in intervals.into_iter().skip(1){if start>end{let width=start-end;if best.map_or(true,|(w,_)|width>w){best=Some((width,(start+end)/2.));}}end=end.max(next_end);}
  best
 }
 fn order(lines:Vec<Value>,depth:usize)->Vec<Value>{
  if lines.len()<2||depth>=16{return lines}
  let mut heights:Vec<_>=lines.iter().map(|l|n(&l["rect"][3])-n(&l["rect"][1])).collect();heights.sort_by(f64::total_cmp);
  let threshold=heights[heights.len()/2].max(8.);
  let mut widths:Vec<_>=lines.iter().map(|l|n(&l["rect"][2])-n(&l["rect"][0])).collect();widths.sort_by(f64::total_cmp);
  // Short aligned cells are table rows, not newspaper columns.
  let split=gap(&lines,0).filter(|(w,_)|*w>=threshold*1.5&&*w<widths[widths.len()/2]).map(|(_,at)|(0,at))
   .or_else(||gap(&lines,1).filter(|(w,_)|*w>=threshold*0.8).map(|(_,at)|(1,at)));
  if let Some((axis,at))=split{
   let (first,last):(Vec<_>,Vec<_>)=lines.into_iter().partition(|l|n(&l["rect"][axis])<at);
   let mut out=order(first,depth+1);out.extend(order(last,depth+1));out
  }else{lines}
 }
 let mut result=layout.clone();
 if let Some(pages)=result["pages"].as_array_mut(){for page in pages{
  let lines=list(page,"lines");
  let valid=lines.iter().all(|l|l["rect"].as_array().map_or(false,|r|r.len()==4&&r.iter().all(|n|n.as_f64().map_or(false,f64::is_finite)))
    &&n(&l["rect"][2])>n(&l["rect"][0])&&n(&l["rect"][3])>n(&l["rect"][1])
    &&(l["dir"].is_null()||(n(&l["dir"][0])>0.9&&n(&l["dir"][1]).abs()<0.1)));
  if valid{page["lines"]=Value::Array(order(lines.to_vec(),0));}
 }}result
}
fn s<'a>(v:&'a Value,k:&str)->&'a str{v[k].as_str().unwrap_or("")}
fn list<'a>(v:&'a Value,k:&str)->&'a [Value]{v[k].as_array().map(Vec::as_slice).unwrap_or(&[])}
fn add(z:&mut ZipWriter<fs::File>,name:&str,data:impl AsRef<[u8]>)->Result<(),String>{z.start_file(name,SimpleFileOptions::default().compression_method(CompressionMethod::Deflated)).map_err(|e|e.to_string())?;z.write_all(data.as_ref()).map_err(|e|e.to_string())}
fn rel(id:&str,kind:&str,target:&str)->String{format!(r#"<Relationship Id="{id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/{kind}" Target="{target}"/>"#)}
fn rels(body:&str)->String{format!(r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{body}</Relationships>"#)}
fn types(overrides:&[(&str,&str)])->String{let mut x=String::from(r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/>"#);for (name,t)in overrides{x+=&format!(r#"<Override PartName="/{name}" ContentType="application/vnd.openxmlformats-officedocument.{t}"/>"#)}x+"</Types>"}
fn image_bytes(im:&Value)->Result<Vec<u8>,String>{fs::read(s(im,"path")).map_err(|e|e.to_string())}
fn text_pages(layout:&Value)->String{list(layout,"pages").iter().map(|p|list(p,"lines").iter().map(|l|s(l,"text")).collect::<Vec<_>>().join("\n")).collect::<Vec<_>>().join("\n\n\u{c}\n\n")}
pub fn export(layout:&Value,kind:&str,output:&Path)->Result<Vec<String>,String>{
 let ordered=reading_order(layout);let layout=&ordered;
 let text=text_pages(layout);if text.trim_matches(|c:char|c.is_whitespace()).is_empty(){return Err("No extractable text. Run OCR before exporting an editable document.".into())}
 match kind{
  "to_txt"=>{fs::write(output,text).map_err(|e|e.to_string())?;return Ok(vec![])},
  "to_md"=>{let mut out=String::new();for (i,p)in list(layout,"pages").iter().enumerate(){out+=&format!("<!-- Page {} -->\n\n",i+1);for l in list(p,"lines"){if n(&l["size"])>=20.{out+="# ";}out+=s(l,"text");out+="\n\n";}for im in list(p,"images"){out+=&format!("![Image](data:image/png;base64,{})\n\n",STANDARD.encode(image_bytes(im)?));}}fs::write(output,out).map_err(|e|e.to_string())?;},
  "to_docx"=>docx(layout,output)?,"to_pptx"=>pptx(layout,output)?,"to_xlsx"=>{let omitted=xlsx(layout,output)?;if !omitted.is_empty(){return Ok(vec![format!("No reliable table was detected on pages {}; these pages were not exported. Review the source before using this workbook.",omitted.iter().map(usize::to_string).collect::<Vec<_>>().join(", "))]);}},"to_epub"=>epub(layout,output)?,_=>return Err("Unknown export format".into())
 }
 let mut warnings=vec!["Layout was reconstructed locally. Check complex columns, tables and reading order before sharing.".into()];
 if list(layout,"pages").iter().any(|p|list(p,"lines").is_empty()){warnings.push("Some pages have no extractable text; their images were retained where supported. OCR is recommended.".into());}
 Ok(warnings)
}
fn docx(v:&Value,path:&Path)->Result<(),String>{
 let mut z=ZipWriter::new(fs::File::create(path).map_err(|e|e.to_string())?);add(&mut z,"[Content_Types].xml",types(&[("word/document.xml","wordprocessingml.document.main+xml")]))?;add(&mut z,"_rels/.rels",rels(&rel("rId1","officeDocument","word/document.xml")))?;
 let mut body=String::new();let mut relationships=String::new();let mut image_id=1;
 for (pi,p)in list(v,"pages").iter().enumerate(){if pi>0{body+="<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>";}let mut prev=Value::Null;let mut paragraph=String::new();
  for l in list(p,"lines"){
   if l["block"]!=prev&&!paragraph.is_empty(){body+=&format!("<w:p>{paragraph}</w:p>");paragraph.clear();}
   let size=(n(&l["size"])*2.).clamp(12.,144.).round()as u32;let font=s(l,"font");let lower=font.to_lowercase();let bold=if lower.contains("bold"){"<w:b/>"}else{""};let italic=if lower.contains("italic")||lower.contains("oblique"){"<w:i/>"}else{""};let color=color(l);let font=xml(font.split_once('+').map(|(_,name)|name).unwrap_or(font));
   paragraph+=&format!(r#"<w:r><w:rPr><w:rFonts w:ascii="{font}" w:hAnsi="{font}" w:eastAsia="{font}"/><w:sz w:val="{size}"/><w:color w:val="{color}"/>{bold}{italic}</w:rPr><w:t xml:space="preserve">{} </w:t></w:r>"#,xml(s(l,"text")));prev=l["block"].clone();
  }if !paragraph.is_empty(){body+=&format!("<w:p>{paragraph}</w:p>");}
  for im in list(p,"images") {let id=image_id;image_id+=1;let w=(n(&im["rect"][2])-n(&im["rect"][0])).max(1.);let h=(n(&im["rect"][3])-n(&im["rect"][1])).max(1.);let scale=(450./w).min(1.);let cx=(w*scale*12700.)as i64;let cy=(h*scale*12700.)as i64;
   add(&mut z,&format!("word/media/image{id}.png"),image_bytes(im)?)?;relationships+=&rel(&format!("image{id}"),"image",&format!("media/image{id}.png"));
   body+=&format!(r#"<w:p><w:r><w:drawing><wp:inline><wp:extent cx="{cx}" cy="{cy}"/><wp:docPr id="{id}" name="Image {id}"/><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:nvPicPr><pic:cNvPr id="{id}" name="Image {id}"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="image{id}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"#);
  }
 }
 body+="<w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/><w:pgMar w:top=\"1000\" w:right=\"1000\" w:bottom=\"1000\" w:left=\"1000\"/></w:sectPr>";
 add(&mut z,"word/document.xml",format!(r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><w:body>{body}</w:body></w:document>"#))?;add(&mut z,"word/_rels/document.xml.rels",rels(&relationships))?;z.finish().map_err(|e|e.to_string())?;Ok(())
}
fn pptx(v:&Value,path:&Path)->Result<(),String>{
 let mut z=ZipWriter::new(fs::File::create(path).map_err(|e|e.to_string())?);let mut overrides=vec![("ppt/presentation.xml".to_owned(),"presentationml.presentation.main+xml")];let mut ids=String::new();let mut relations=String::new();
 let first=&list(v,"pages")[0];let pw=(n(&first["bounds"][2])-n(&first["bounds"][0])).max(1.);let ph=(n(&first["bounds"][3])-n(&first["bounds"][1])).max(1.);let sw=(pw*12700.)as i64;let sh=(ph*12700.)as i64;
 for (i,p)in list(v,"pages").iter().enumerate(){let sn=i+1;let mut shapes=String::new();let mut relationships=String::new();let w=(n(&p["bounds"][2])-n(&p["bounds"][0])).max(1.);let h=(n(&p["bounds"][3])-n(&p["bounds"][1])).max(1.);let sx=sw as f64/w;let sy=sh as f64/h;let mut id=2;
  for l in list(p,"lines"){let x=((n(&l["rect"][0])-n(&p["bounds"][0]))*sx)as i64;let y=((n(&l["rect"][1])-n(&p["bounds"][1]))*sy)as i64;let cx=((n(&l["rect"][2])-n(&l["rect"][0])+6.)*sx)as i64;let cy=((n(&l["rect"][3])-n(&l["rect"][1])+4.)*sy)as i64;let size=(n(&l["size"])*pw/w*100.).round()as i32;let color=color(l);let font=xml(s(l,"font").split_once('+').map(|(_,name)|name).unwrap_or(s(l,"font")));let lower=font.to_lowercase();let bold=if lower.contains("bold"){1}else{0};let italic=if lower.contains("italic")||lower.contains("oblique"){1}else{0};
   shapes+=&format!(r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="Text {id}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/></p:spPr><p:txBody><a:bodyPr wrap="none" lIns="0" tIns="0" rIns="0" bIns="0"/><a:lstStyle/><a:p><a:r><a:rPr lang="zh-CN" sz="{size}" b="{bold}" i="{italic}"><a:solidFill><a:srgbClr val="{color}"/></a:solidFill><a:latin typeface="{font}"/><a:ea typeface="{font}"/></a:rPr><a:t>{}</a:t></a:r></a:p></p:txBody></p:sp>"#,xml(s(l,"text")));id+=1;
  }
  for im in list(p,"images"){let x=((n(&im["rect"][0])-n(&p["bounds"][0]))*sx)as i64;let y=((n(&im["rect"][1])-n(&p["bounds"][1]))*sy)as i64;let cx=((n(&im["rect"][2])-n(&im["rect"][0]))*sx)as i64;let cy=((n(&im["rect"][3])-n(&im["rect"][1]))*sy)as i64;let name=format!("image-{sn}-{id}.png");add(&mut z,&format!("ppt/media/{name}"),image_bytes(im)?)?;relationships+=&rel(&format!("image{id}"),"image",&format!("../media/{name}"));shapes+=&format!(r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="Image {id}"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="image{id}"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#);id+=1;}
  let tree=format!(r#"<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>{shapes}"#);
  add(&mut z,&format!("ppt/slides/slide{sn}.xml"),format!(r#"<?xml version="1.0" encoding="UTF-8"?><p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree>{tree}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"#))?;
  add(&mut z,&format!("ppt/slides/_rels/slide{sn}.xml.rels"),rels(&relationships))?;overrides.push((format!("ppt/slides/slide{sn}.xml"),"presentationml.slide+xml"));ids+=&format!(r#"<p:sldId id="{}" r:id="rId{sn}"/>"#,255+sn);relations+=&rel(&format!("rId{sn}"),"slide",&format!("slides/slide{sn}.xml"));
 }
 add(&mut z,"ppt/presentation.xml",format!(r#"<?xml version="1.0" encoding="UTF-8"?><p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:sldIdLst>{ids}</p:sldIdLst><p:sldSz cx="{sw}" cy="{sh}"/><p:notesSz cx="6858000" cy="9144000"/></p:presentation>"#))?;
 add(&mut z,"ppt/_rels/presentation.xml.rels",rels(&relations))?;add(&mut z,"_rels/.rels",rels(&rel("rId1","officeDocument","ppt/presentation.xml")))?;add(&mut z,"[Content_Types].xml",types(&overrides.iter().map(|(a,b)|(a.as_str(),*b)).collect::<Vec<_>>()))?;z.finish().map_err(|e|e.to_string())?;Ok(())
}
fn col(mut c:usize)->String{let mut out=String::new();loop{out.insert(0,(b'A'+(c%26)as u8)as char);if c<26{break}c=c/26-1;}out}
fn xlsx(v:&Value,path:&Path)->Result<Vec<usize>,String>{
 let mut sheets=Vec::new();let mut omitted=Vec::new();
 for (pi,p)in list(v,"pages").iter().enumerate(){let lines=list(p,"lines");let mut columns:Vec<f64>=Vec::new();for l in lines{let x=n(&l["rect"][0]);if columns.iter().all(|a|(a-x).abs()>8.){columns.push(x);}}columns.sort_by(|a,b|a.total_cmp(b));if columns.len()<2{omitted.push(pi+1);continue}
  let mut rows:Vec<(f64,Vec<(usize,String)>)>=Vec::new();for l in lines{let x=n(&l["rect"][0]);let y=n(&l["rect"][1]);let ci=columns.iter().position(|a|(a-x).abs()<=8.).unwrap();if let Some(row)=rows.iter_mut().find(|r|(r.0-y).abs()<5.){row.1.push((ci,s(l,"text").into()));}else{rows.push((y,vec![(ci,s(l,"text").into())]));}}rows.sort_by(|a,b|a.0.total_cmp(&b.0));if rows.iter().filter(|r|r.1.len()>=2).count()<2{omitted.push(pi+1);continue}
  let mut cells=String::new();for (ri,(_,cs))in rows.iter().enumerate(){cells+=&format!("<row r=\"{}\">",ri+1);let mut merged=std::collections::BTreeMap::<usize,String>::new();for (ci,t)in cs{merged.entry(*ci).and_modify(|s|{s.push(' ');s.push_str(t)}).or_insert(t.clone());}for (ci,t)in merged{let reference=format!("{}{}",col(ci),ri+1);if let Ok(num)=t.trim().parse::<f64>(){if num.is_finite(){cells+=&format!("<c r=\"{reference}\"><v>{num}</v></c>");continue;}}cells+=&format!(r#"<c r="{reference}" t="inlineStr"><is><t xml:space="preserve">{}</t></is></c>"#,xml(&t));}cells+="</row>";}sheets.push((pi+1,cells));
 }
 if sheets.is_empty(){return Err("No reliable multi-column table detected. Select a table document or run OCR first.".into())}
 let mut z=ZipWriter::new(fs::File::create(path).map_err(|e|e.to_string())?);let mut refs=String::new();let mut relationships=String::new();let mut overrides=vec![("xl/workbook.xml".to_owned(),"spreadsheetml.sheet.main+xml")];for (i,(page,cells))in sheets.iter().enumerate(){let id=i+1;refs+=&format!(r#"<sheet name="Page {page}" sheetId="{id}" r:id="rId{id}"/>"#);relationships+=&rel(&format!("rId{id}"),"worksheet",&format!("worksheets/sheet{id}.xml"));let name=format!("xl/worksheets/sheet{id}.xml");add(&mut z,&name,format!(r#"<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>{cells}</sheetData></worksheet>"#))?;overrides.push((name,"spreadsheetml.worksheet+xml"));}
 add(&mut z,"xl/workbook.xml",format!(r#"<?xml version="1.0" encoding="UTF-8"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets>{refs}</sheets></workbook>"#))?;add(&mut z,"xl/_rels/workbook.xml.rels",rels(&relationships))?;add(&mut z,"_rels/.rels",rels(&rel("rId1","officeDocument","xl/workbook.xml")))?;add(&mut z,"[Content_Types].xml",types(&overrides.iter().map(|(a,b)|(a.as_str(),*b)).collect::<Vec<_>>()))?;z.finish().map_err(|e|e.to_string())?;Ok(omitted)
}
fn epub(v:&Value,path:&Path)->Result<(),String>{
 let mut z=ZipWriter::new(fs::File::create(path).map_err(|e|e.to_string())?);z.start_file("mimetype",SimpleFileOptions::default().compression_method(CompressionMethod::Stored)).map_err(|e|e.to_string())?;z.write_all(b"application/epub+zip").map_err(|e|e.to_string())?;add(&mut z,"META-INF/container.xml",r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#)?;
 let mut manifest=String::from(r#"<item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>"#);let mut spine=String::new();let mut nav=String::new();for (i,p)in list(v,"pages").iter().enumerate(){let id=i+1;let mut html=String::new();for l in list(p,"lines"){let tag=if n(&l["size"])>=20.{"h2"}else{"p"};html+=&format!("<{tag}>{}</{tag}>",xml(s(l,"text")));}for (j,im)in list(p,"images").iter().enumerate(){let name=format!("image-{id}-{j}.png");add(&mut z,&format!("OEBPS/{name}"),image_bytes(im)?)?;manifest+=&format!(r#"<item id="im{id}-{j}" href="{name}" media-type="image/png"/>"#);html+=&format!(r#"<p><img src="{name}" alt="Document image" style="max-width:100%"/></p>"#);}add(&mut z,&format!("OEBPS/page{id}.xhtml"),format!(r#"<?xml version="1.0" encoding="UTF-8"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>Page {id}</title></head><body>{html}</body></html>"#))?;manifest+=&format!(r#"<item id="p{id}" href="page{id}.xhtml" media-type="application/xhtml+xml"/>"#);spine+=&format!(r#"<itemref idref="p{id}"/>"#);nav+=&format!(r#"<li><a href="page{id}.xhtml">Page {id}</a></li>"#);}
 add(&mut z,"OEBPS/nav.xhtml",format!(r#"<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>Contents</title></head><body><nav epub:type="toc"><ol>{nav}</ol></nav></body></html>"#))?;add(&mut z,"OEBPS/content.opf",format!(r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:identifier id="id">urn:uuid:{}</dc:identifier><dc:title>Converted document</dc:title><dc:language>zh</dc:language><meta property="dcterms:modified">2026-11-04T00:00:00Z</meta></metadata><manifest>{manifest}</manifest><spine>{spine}</spine></package>"#,uuid::Uuid::new_v4()))?;z.finish().map_err(|e|e.to_string())?;Ok(())
}
#[cfg(test)]mod tests{use super::*;#[test]fn escaping(){assert_eq!(xml("<&\""),"&lt;&amp;&quot;");assert_eq!(col(26),"AA");}#[test]fn reject_no_table(){let d=tempfile::tempdir().unwrap();assert!(xlsx(&serde_json::json!({"pages":[{"lines":[]}]}),&d.path().join("a.xlsx")).is_err());}}

#[cfg(test)]
mod layout_tests {
 use super::*;use serde_json::json;
 fn line(text:&str,x:f64,y:f64,width:f64)->Value{json!({"text":text,"rect":[x,y,x+width,y+12.],"dir":[1,0],"block":text,"size":12,"font":"Example-BoldItalic","color":[0.2,0.4,0.6]})}
 #[test]
 fn two_columns_with_a_spanning_heading_read_column_by_column(){
  let layout=json!({"pages":[{"lines":[line("Heading",40.,20.,500.),line("Left one",40.,80.,210.),line("Right one",330.,80.,210.),line("Left two",40.,98.,210.),line("Right two",330.,98.,210.)]}]});
  let ordered=reading_order(&layout);let texts:Vec<_>=list(&ordered["pages"][0],"lines").iter().map(|l|s(l,"text")).collect();
  assert_eq!(texts,vec!["Heading","Left one","Left two","Right one","Right two"]);
  assert_eq!(texts.len(),5);
 }
 #[test]
 fn short_table_cells_keep_row_order(){
  let layout=json!({"pages":[{"lines":[line("A",40.,50.,30.),line("B",200.,50.,30.),line("1",40.,70.,30.),line("2",200.,70.,30.)]}]});
  let ordered=reading_order(&layout);let texts:Vec<_>=list(&ordered["pages"][0],"lines").iter().map(|l|s(l,"text")).collect();assert_eq!(texts,vec!["A","B","1","2"]);
 }
 #[test]
 fn editable_exports_keep_text_style_and_report_skipped_table_pages(){
  use std::io::Read;
  let dir=tempfile::tempdir().unwrap();
  let layout=json!({"pages":[{"bounds":[0,0,595,842],"lines":[line("A",40.,50.,30.),line("B",200.,50.,30.),line("1",40.,70.,30.),line("2",200.,70.,30.)]},{"bounds":[0,0,595,842],"lines":[line("No table here",40.,50.,250.)]}]});
  for (kind,member)in [("to_docx","word/document.xml"),("to_pptx","ppt/slides/slide1.xml")]{
   let path=dir.path().join(kind);export(&layout,kind,&path).unwrap();let mut zip=zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();let mut xml=String::new();zip.by_name(member).unwrap().read_to_string(&mut xml).unwrap();assert!(xml.contains("336699"));assert!(xml.contains("Example-BoldItalic"));assert!(xml.contains(if kind=="to_docx"{"<w:i/>"}else{"i=\"1\""}));
  }
  let warnings=export(&layout,"to_xlsx",&dir.path().join("table.xlsx")).unwrap();assert!(warnings.iter().any(|w|w.contains("pages 2")&&w.contains("not exported")));
 }
}
