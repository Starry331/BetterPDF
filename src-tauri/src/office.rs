//! Bounded OOXML editing. Only targeted XML spans are changed; other ZIP members
//! are copied verbatim. Layout is previewed by the bundled isolated Office worker.
#[path="office/paragraphs.rs"]
mod paragraphs;
#[path="office/lists.rs"] mod lists;
#[path="office/slides.rs"]
mod slides;
#[path="office/cells.rs"] mod cells;
use quick_xml::{events::Event, Reader};
use serde::Serialize;
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};
const XML_LIMIT: u64 = 16 * 1024 * 1024;
const PACKAGE_LIMIT: u64 = 512 * 1024 * 1024;
const ITEM_LIMIT: usize = 100_000;
#[derive(Clone, Serialize)]
pub struct Item {
    pub id: usize,
    pub part: String,
    pub label: String,
    pub text: String,
    pub kind: String,
    pub editable: bool,
    #[serde(skip)]
    start: usize,
    #[serde(skip)]
    end: usize,
}
#[derive(Serialize)]
pub struct Page {
    pub items: Vec<Item>,
    pub total: usize,
    pub parts: Vec<String>,
    pub labels: std::collections::HashMap<String, String>,
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
pub fn format(path: &Path) -> String {
    path.extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase()
}
pub fn supported(path: &Path) -> bool {
    matches!(
        format(path).as_str(),
        "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx"
    )
}
fn archive(path: &Path) -> Result<ZipArchive<File>, String> {
    let z = ZipArchive::new(File::open(path).map_err(err)?).map_err(err)?;
    if z.len() > 20_000 {
        return Err("Office package contains too many entries".into());
    }
    let mut z = z;
    let mut total = 0u64;
    let mut names = std::collections::HashSet::new();
    for i in 0..z.len() {
        let f = z.by_index(i).map_err(err)?;
        total = total.checked_add(f.size()).ok_or("Package too large")?;
        if total > PACKAGE_LIMIT {
            return Err("Office package exceeds the 512 MB expanded limit".into());
        }
        if !names.insert(f.name().to_string()) {
            return Err("Duplicate Office package entries".into());
        }
        if f.name().starts_with("_xmlsignatures/") {
            return Err("Signed Office documents cannot be edited in compatibility mode".into());
        }
    }
    Ok(z)
}
fn xml(z: &mut ZipArchive<File>, name: &str) -> Result<String, String> {
    let f = z.by_name(name).map_err(err)?;
    if f.size() > XML_LIMIT {
        return Err("Office XML part exceeds 16 MB".into());
    }
    let mut s = String::new();
    f.take(XML_LIMIT + 1).read_to_string(&mut s).map_err(err)?;
    if s.len() as u64 > XML_LIMIT || s.contains("<!DOCTYPE") || s.contains("<!ENTITY") {
        return Err("Unsupported or oversized Office XML".into());
    }
    Ok(s)
}
fn attr(e: &quick_xml::events::BytesStart, key: &str) -> Result<String, String> {
    for a in e.attributes() {
        let a = a.map_err(err)?;
        if a.key.as_ref() == key {
            return quick_xml::escape::unescape(&a.value)
                .map(|s| s.into_owned())
                .map_err(err);
        }
    }
    Ok(String::new())
}
// XML spans keep the original namespaces, run styling, relationships and media.
fn spans(s: &str, tag: &str) -> Result<Vec<(usize, usize, String)>, String> {
    let mut r = Reader::from_str(s);
    let mut stack = Vec::new();
    let mut result = Vec::new();
    loop {
        let start = r.buffer_position() as usize;
        match r.read_event().map_err(err)? {
            Event::Start(e) => {
                if e.local_name().as_ref() == tag {
                    stack.push((start, attr(&e, "r")?));
                }
            }
            Event::Empty(e) => {
                if e.local_name().as_ref() == tag {
                    result.push((start, r.buffer_position() as usize, attr(&e, "r")?));
                }
            }
            Event::End(e) => {
                if e.local_name().as_ref() == tag {
                    let (start, label) = stack.pop().ok_or("Unbalanced XML")?;
                    result.push((start, r.buffer_position() as usize, label));
                }
            }
            Event::DocType(_) => return Err("DTD is not supported".into()),
            Event::Eof => break,
            _ => {}
        }
        if result.len() > ITEM_LIMIT {
            return Err("Office part contains too many editable items".into());
        }
    }
    Ok(result)
}
fn text(s: &str) -> Result<String, String> {
    let mut r = Reader::from_str(s);
    let mut out = String::new();
    loop {
        match r.read_event().map_err(err)? {
            Event::Text(t) => out.push_str(&t),
            Event::GeneralRef(t) => {
                let v = format!("&{};", t.as_ref());
                out.push_str(&quick_xml::escape::unescape(&v).map_err(err)?);
            }
            Event::CData(t) => out.push_str(&t),
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(out)
}
fn children_text(s: &str, tag: &str) -> Result<String, String> {
    let mut out = String::new();
    for (a, b, _) in spans(s, tag)? {
        out.push_str(&text(&s[a..b])?)
    }
    Ok(out)
}
fn main_parts(z: &mut ZipArchive<File>, fmt: &str) -> Result<Vec<(String, String)>, String> {
    let mut p: Vec<_> = z
        .file_names()
        .filter(|n| match fmt {
            "docx" => *n == "word/document.xml",
            "pptx" => {
                n.starts_with("ppt/slides/slide") && n.ends_with(".xml") && !n.contains("/_rels/")
            }
            "xlsx" => {
                n.starts_with("xl/worksheets/sheet")
                    && n.ends_with(".xml")
                    && !n.contains("/_rels/")
            }
            _ => false,
        })
        .map(str::to_string)
        .collect();
    p.sort_by_key(|s| {
        let n = s
            .chars()
            .filter(char::is_ascii_digit)
            .collect::<String>()
            .parse::<u32>()
            .unwrap_or(0);
        (n, s.clone())
    });
    let fallback = p
        .iter()
        .map(|s| {
            (
                s.clone(),
                s.rsplit('/')
                    .next()
                    .unwrap_or(s)
                    .trim_end_matches(".xml")
                    .to_string(),
            )
        })
        .collect();
    let (base, index, rels, tag) = match fmt {
        "xlsx" => (
            "xl",
            "xl/workbook.xml",
            "xl/_rels/workbook.xml.rels",
            "sheet",
        ),
        "pptx" => (
            "ppt",
            "ppt/presentation.xml",
            "ppt/_rels/presentation.xml.rels",
            "sldId",
        ),
        _ => return Ok(fallback),
    };
    if !z.file_names().any(|n| n == index) || !z.file_names().any(|n| n == rels) {
        return Ok(fallback);
    }
    let mut targets = std::collections::HashMap::new();
    let relations = xml(z, rels)?;
    let mut r = Reader::from_str(&relations);
    loop {
        match r.read_event().map_err(err)? {
            Event::Start(e) | Event::Empty(e) if e.local_name().as_ref() == "Relationship" => {
                if attr(&e, "TargetMode")? == "External" {
                    continue;
                }
                let target = attr(&e, "Target")?;
                let target = if target.starts_with('/') {
                    target.trim_start_matches('/').to_string()
                } else {
                    format!("{base}/{target}")
                };
                if p.contains(&target) {
                    targets.insert(attr(&e, "Id")?, target);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let index = xml(z, index)?;
    let mut r = Reader::from_str(&index);
    let mut ordered = Vec::new();
    loop {
        match r.read_event().map_err(err)? {
            Event::Start(e) | Event::Empty(e) if e.local_name().as_ref() == tag => {
                let id = attr(&e, "r:id")?;
                if let Some(path) = targets.get(&id) {
                    let label = if fmt == "xlsx" {
                        attr(&e, "name")?
                    } else {
                        format!("{}", ordered.len() + 1)
                    };
                    ordered.push((path.clone(), label));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if ordered.is_empty() {
        return Err("Office section relationships are unsupported".into());
    }
    Ok(ordered)
}
fn items(path: &Path) -> Result<Vec<Item>, String> {
    let mut z = archive(path)?;
    let fmt = format(path);
    let parts = main_parts(&mut z, &fmt)?;
    if parts.is_empty() {
        return Err("Unsupported Office package structure".into());
    }
    let shared = if fmt == "xlsx" && z.file_names().any(|n| n == "xl/sharedStrings.xml") {
        let s = xml(&mut z, "xl/sharedStrings.xml")?;
        spans(&s, "si")?
            .iter()
            .map(|(a, b, _)| children_text(&s[*a..*b], "t"))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        vec![]
    };
    let mut out = Vec::new();
    let mut text_bytes = 0usize;
    let word_protected = fmt == "docx" && z.file_names().any(|n| n == "word/settings.xml") && {
        let settings = xml(&mut z, "word/settings.xml")?;
        !spans(&settings, "documentProtection")?.is_empty()
    };
    for (part, _) in parts {
        let s = xml(&mut z, &part)?;
        let protected = fmt == "xlsx" && !spans(&s, "sheetProtection")?.is_empty();
        for (index, (start, end, label)) in spans(&s, if fmt == "xlsx" { "c" } else { "t" })?
            .into_iter()
            .enumerate()
        {
            let fragment = &s[start..end];
            let mut kind = "text".to_string();
            let mut editable = !protected && !word_protected;
            let value = if fmt == "xlsx" {
                let mut r = Reader::from_str(fragment);
                let typ = match r.read_event().map_err(err)? {
                    Event::Start(e) | Event::Empty(e) => attr(&e, "t")?,
                    _ => String::new(),
                };
                let f = spans(fragment, "f")?;
                if !f.is_empty() {
                    kind = "formula".into();
                    let value = children_text(fragment, "f")?;
                    let mut r = Reader::from_str(&fragment[f[0].0..f[0].1]);
                    let simple = match r.read_event().map_err(err)? {
                        Event::Start(e) | Event::Empty(e) => attr(&e, "t")?.is_empty(),
                        _ => false,
                    };
                    editable = editable && simple && formula(&value).is_ok();
                    format!("={value}")
                } else {
                    let v = children_text(fragment, "v")?;
                    match typ.as_str() {
                        "s" => shared
                            .get(
                                v.parse::<usize>()
                                    .map_err(|_| "Invalid shared string index")?,
                            )
                            .ok_or("Invalid shared string index")?
                            .clone(),
                        "inlineStr" => children_text(fragment, "t")?,
                        "" | "n" => {
                            kind = "number".into();
                            v
                        }
                        "b" => {
                            kind = "boolean".into();
                            v
                        }
                        _ => {
                            editable = false;
                            v
                        }
                    }
                }
            } else {
                text(fragment)?
            };
            text_bytes += value.len();
            if text_bytes > 16 * 1024 * 1024 {
                return Err("Office text exceeds the 16 MB compatibility editing limit".into());
            }
            out.push(Item {
                id: out.len(),
                part: part.clone(),
                label: if label.is_empty() {
                    format!("{}", index + 1)
                } else {
                    label
                },
                text: value,
                kind,
                editable,
                start,
                end,
            });
            if out.len() > ITEM_LIMIT {
                return Err("Office document exceeds 100,000 editable items".into());
            }
        }
    }
    Ok(out)
}
pub fn page(path: &Path, offset: usize, query: &str, part: &str) -> Result<Page, String> {
    let all = items(path)?;
    let mut z = archive(path)?;
    let ordered = main_parts(&mut z, &format(path))?;
    let parts = ordered.iter().map(|(p, _)| p.clone()).collect();
    let labels = ordered.into_iter().collect();
    let q = query.to_lowercase();
    let filtered: Vec<_> = all
        .into_iter()
        .filter(|i| {
            (part.is_empty() || i.part == part)
                && (q.is_empty()
                    || i.text.to_lowercase().contains(&q)
                    || i.label.to_lowercase().contains(&q))
        })
        .collect();
    let total = filtered.len();
    Ok(Page {
        items: filtered.into_iter().skip(offset).take(80).collect(),
        total,
        parts,
        labels,
    })
}
fn escaped(s: &str) -> String {
    quick_xml::escape::escape(s).into_owned()
}
pub fn edit(
    input: &Path,
    output: &Path,
    id: usize,
    value: &str,
    kind: &str,
    style: &serde_json::Value,
) -> Result<(), String> {
    if value.len() > 32767
        || value
            .chars()
            .any(|c| c < ' ' && !matches!(c, '\t' | '\n' | '\r'))
    {
        return Err("Text is too long or contains invalid control characters".into());
    }
    if id==ITEM_LIMIT+1{if matches!(kind,"slide_duplicate"|"slide_delete"|"slide_blank"){return slides::edit(input,output,kind,style)}return reorder_slide(input,output,kind,style)}
    if id == ITEM_LIMIT+2 {return slides::add_text(input,output,value,style)}
    if id == ITEM_LIMIT {
        return insert_cell(input, output, value, kind, style);
    }
    let all = items(input)?;
    let item = all.get(id).ok_or("Office item no longer exists")?;
    if !item.editable {
        return Err(
            "Complex formula, protected or unsupported cells are read-only in compatibility mode"
                .into(),
        );
    }
    let mut z = archive(input)?;
    let mut s = xml(&mut z, &item.part)?;
    if kind=="paragraph_list" {
        if format(input)!="docx"{return Err("List editing requires Word compatibility mode".into())}
        return lists::edit(z,output,&s,item,style["list"].as_str().ok_or("Choose list type")?);
    }
    if matches!(kind,"paragraph_align"|"paragraph_delete") {
        if format(input)!="docx"{return Err("Paragraph editing requires Word compatibility mode".into())}
        let changed=paragraphs::change(&s,item,kind,style)?;
        return write_package(z,output,[(item.part.clone(),changed)].into_iter().collect());
    }
    if kind=="paragraph_after" {
        if format(input)!="docx" {return Err("Paragraph insertion requires Word compatibility mode".into())}
        let (pa,pb,_)=spans(&s,"p")?.into_iter().find(|(a,b,_)|*a<=item.start&&*b>=item.end).ok_or("Select text inside a paragraph")?;
        let paragraph=&s[pa..pb];let mut reader=Reader::from_str(paragraph);
        let name=match reader.read_event().map_err(err)? {Event::Start(e)=>e.name().as_ref().to_string(),_=>return Err("Invalid paragraph".into())};
        let prefix=name.strip_suffix('p').unwrap_or("");
        let mut properties=spans(paragraph,"pPr")?.first().map(|(a,b,_)|paragraph[*a..*b].to_string()).unwrap_or_default();
        // Continue list numbering, but never duplicate section or revision boundaries.
        for tag in ["sectPr","pPrChange"] {for (a,b,_) in spans(&properties,tag)?.into_iter().rev(){properties.replace_range(a..b,"");}}
        let (ra,rb,_)=spans(&s,"r")?.into_iter().find(|(a,b,_)|*a<=item.start&&*b>=item.end).ok_or("Select an editable text run")?;
        let original_run=&s[ra..rb];let rp=spans(original_run,"rPr")?.first().map(|(a,b,_)|original_run[*a..*b].to_string()).unwrap_or_default();
        let normalized=value.replace("\r\n","\n").replace('\r',"\n");
        let text=escaped(&normalized).replace('\n',&format!("</{prefix}t><{prefix}br/><{prefix}t xml:space=\"preserve\">"))
            .replace('\t',&format!("</{prefix}t><{prefix}tab/><{prefix}t xml:space=\"preserve\">"));
        let run=style_run(&format!("<{prefix}r>{rp}<{prefix}t xml:space=\"preserve\">{text}</{prefix}t></{prefix}r>"),true,style)?;
        s.insert_str(pb,&format!("<{name}>{properties}{run}</{name}>"));
        return write_package(z,output,[(item.part.clone(),s)].into_iter().collect());
    }
    let fragment = &s[item.start..item.end];
    let replacement = if format(input) == "xlsx" {
        let mut r = Reader::from_str(fragment);
        let e = match r.read_event().map_err(err)? {
            Event::Start(e) | Event::Empty(e) => e,
            _ => return Err("Invalid cell".into()),
        };
        let name = e.name().as_ref().to_string();
        let mut attrs = String::new();
        for a in e.attributes() {
            let a = a.map_err(err)?;
            if a.key.as_ref() != "t" {
                attrs.push_str(&format!(" {}=\"{}\"", a.key.as_ref(), &a.value));
            }
        }
        // Cell metadata/extensions are intentionally not dropped by a value replacement.
        let mut extras = String::new();
        for (a, b, _) in spans(fragment, "extLst")? {
            extras.push_str(&fragment[a..b]);
        }
        let prefix = name.strip_suffix('c').unwrap_or("");
        if kind == "number" {
            if value
                .parse::<f64>()
                .ok()
                .filter(|n| n.is_finite())
                .is_none()
            {
                return Err("Enter a finite number, or choose Text".into());
            }
            format!(
                "<{name}{attrs}><{prefix}v>{}</{prefix}v>{extras}</{name}>",
                escaped(value)
            )
        } else if kind == "formula" {
            let value = value.strip_prefix('=').unwrap_or(value);
            formula(value)?;
            format!(
                "<{name}{attrs}><{prefix}f>{}</{prefix}f>{extras}</{name}>",
                escaped(value)
            )
        } else if kind == "text" {
            format!("<{name}{attrs} t=\"inlineStr\"><{prefix}is><{prefix}t xml:space=\"preserve\">{}</{prefix}t></{prefix}is>{extras}</{name}>",escaped(value))
        } else {
            return Err("Choose Text, Number or Formula".into());
        }
    } else {
        let mut r = Reader::from_str(fragment);
        let e = match r.read_event().map_err(err)? {
            Event::Start(e) | Event::Empty(e) => e,
            _ => return Err("Invalid text run".into()),
        };
        let name = e.name().as_ref().to_string();
        let normalized=value.replace("\r\n","\n").replace('\r',"\n");
        if format(input)=="docx" {
            let prefix=name.strip_suffix('t').unwrap_or("");
            let content=escaped(&normalized).replace('\n',&format!("</{name}><{prefix}br/><{name} xml:space=\"preserve\">"))
                .replace('\t',&format!("</{name}><{prefix}tab/><{name} xml:space=\"preserve\">"));
            format!("<{name} xml:space=\"preserve\">{content}</{name}>")
        } else {format!("<{name}>{}</{name}>",escaped(&normalized))}
    };
    if format(input) != "xlsx" && (style.as_object().is_some_and(|o| !o.is_empty()) || value.contains(['\n','\r','\t'])) {
        let (start, end, _) = spans(&s, "r")?
            .into_iter()
            .find(|(a, b, _)| *a <= item.start && *b >= item.end)
            .ok_or("This text does not have an editable style run")?;
        let mut run = s[start..end].to_string();
        run.replace_range(item.start - start..item.end - start, &replacement);
        run = style_run(&run, format(input) == "docx", style)?;
        if format(input)=="pptx" && value.contains(['\n','\r']) {
            let (ta,tb,_)=spans(&run,"t")?.into_iter().next().ok_or("Missing slide text")?;
            let mut reader=Reader::from_str(&run);
            let event=reader.read_event().map_err(err)?;
            let run_name=match event {Event::Start(e)=>e.name().as_ref().to_string(),_=>return Err("Invalid slide run".into())};
            let prefix=run_name.strip_suffix('r').unwrap_or("");
            let normalized=value.replace("\r\n","\n").replace('\r',"\n");
            let fragments=normalized.split('\n').map(|line|{
                let mut copy=run.clone();copy.replace_range(ta..tb,&format!("<{prefix}t>{}</{prefix}t>",escaped(line)));copy
            }).collect::<Vec<_>>();
            run=fragments.join(&format!("<{prefix}br/>"));
        }
        s.replace_range(start..end, &run);
    } else {
        s.replace_range(item.start..item.end, &replacement);
    }
    let mut changes = std::collections::HashMap::new();
    changes.insert(item.part.clone(), s);
    if format(input) == "xlsx" {
        let mut workbook = xml(&mut z, "xl/workbook.xml")?;
        let calc = spans(&workbook, "calcPr")?;
        if let Some((a, b, _)) = calc.first() {
            let fragment = &workbook[*a..*b];
            let mut r = Reader::from_str(fragment);
            let e = match r.read_event().map_err(err)? {
                Event::Start(e) | Event::Empty(e) => e,
                _ => return Err("Invalid calculation properties".into()),
            };
            let name = e.name().as_ref().to_string();
            workbook.replace_range(*a..*b,&format!("<{name} calcId=\"0\" fullCalcOnLoad=\"1\" forceFullCalc=\"1\" calcMode=\"auto\"/>"));
        } else {
            let at = workbook.rfind("</").ok_or("Invalid workbook")?;
            let end = &workbook[at + 2..];
            let prefix = end
                .split_once(':')
                .map(|(p, _)| format!("{p}:"))
                .unwrap_or_default();
            workbook.insert_str(at,&format!("<{prefix}calcPr calcId=\"0\" fullCalcOnLoad=\"1\" forceFullCalc=\"1\" calcMode=\"auto\"/>"));
        }
        changes.insert("xl/workbook.xml".into(), workbook);
        cells::apply(&mut z,&mut changes,&item.part,&item.label,style)?;
    }
    write_package(z, output, changes)
}
fn write_package(
    z: ZipArchive<File>,output:&Path,changes:std::collections::HashMap<String,String>,
)->Result<(),String>{write_package_changes(z,output,changes.into_iter().map(|(k,v)|(k,Some(v))).collect())}
fn write_package_changes(mut z:ZipArchive<File>,output:&Path,mut changes:std::collections::HashMap<String,Option<String>>)->Result<(),String>{
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(err)?;
    let mut writer = ZipWriter::new(file);
    for i in 0..z.len() {
        let f = z.by_index(i).map_err(err)?;
        if let Some(content) = changes.remove(f.name()) {
            let Some(content)=content else {continue};
            writer
                .start_file(
                    f.name(),
                    SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated),
                )
                .map_err(err)?;
            writer.write_all(content.as_bytes()).map_err(err)?;
        } else {
            writer.raw_copy_file(f).map_err(err)?;
        }
    }
    for (name,content) in changes {if let Some(content)=content{writer.start_file(name,SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated)).map_err(err)?;writer.write_all(content.as_bytes()).map_err(err)?;}}
    writer.finish().map_err(err)?.sync_all().map_err(err)?;
    items(output)?;
    Ok(())
}

fn reorder_slide(input:&Path,output:&Path,kind:&str,style:&serde_json::Value)->Result<(),String>{
    if format(input)!="pptx" {return Err("Slide ordering requires a PPTX document".into())}
    let mut z=archive(input)?;let ordered=main_parts(&mut z,"pptx")?;
    let part=style["part"].as_str().ok_or("Select a slide")?;
    let index=ordered.iter().position(|(p,_)|p==part).ok_or("Invalid slide")?;
    let other=match kind {"slide_up" if index>0=>index-1,"slide_down" if index+1<ordered.len()=>index+1,_=>return Err("Slide cannot move in that direction".into())};
    let mut xml=xml(&mut z,"ppt/presentation.xml")?;let spans=spans(&xml,"sldId")?;
    if spans.len()!=ordered.len(){return Err("Slide relationships are inconsistent".into())}
    let first=index.min(other);let second=index.max(other);let (a,b,_)=spans[first];let (c,d,_)=spans[second];
    let left=xml[a..b].to_string();let right=xml[c..d].to_string();xml.replace_range(c..d,&left);xml.replace_range(a..b,&right);
    write_package(z,output,[("ppt/presentation.xml".into(),xml)].into_iter().collect())
}

fn formula(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 8192
        || value.contains(['[', ']', '|', '{', '}', ';', '\n', '\r'])
    {
        return Err("Use a simple local formula without external references or arrays".into());
    }
    let mut depth = 0usize;
    let mut quoted = false;
    for c in value.chars() {
        if c == '"' {
            quoted = !quoted;
        }
        if !quoted {
            if c == '(' {
                depth += 1;
            }
            if c == ')' {
                if depth == 0 {
                    return Err("Unbalanced formula parentheses".into());
                }
                depth -= 1;
            }
        }
    }
    if quoted || depth != 0 {
        return Err("Unbalanced formula parentheses or quotes".into());
    }
    let functions = regex::Regex::new(r"(?i)([a-z_][a-z0-9_.]*)\s*\(").unwrap();
    for c in functions.captures_iter(value) {
        if ![
            "SUM", "AVERAGE", "MIN", "MAX", "COUNT", "COUNTA", "IF", "IFERROR", "ROUND", "ABS",
            "AND", "OR", "NOT",
        ]
        .contains(&c[1].to_ascii_uppercase().as_str())
        {
            return Err(
                "This formula function is not supported for editing in compatibility mode".into(),
            );
        }
    }
    Ok(())
}
fn style_run(run: &str, word: bool, style: &serde_json::Value) -> Result<String, String> {
    let mut r = Reader::from_str(run);
    let e = match r.read_event().map_err(err)? {
        Event::Start(e) => e,
        _ => return Err("Invalid style run".into()),
    };
    let name = e.name().as_ref().to_string();
    let prefix = name.strip_suffix('r').unwrap_or("");
    let existing = spans(run, "rPr")?.into_iter().next();
    let mut properties = existing
        .as_ref()
        .map(|(a, b, _)| run[*a..*b].to_string())
        .unwrap_or_else(|| format!("<{prefix}rPr/>"));
    let mut changes = Vec::new();
    for k in ["bold", "italic"] {
        if let Some(v) = style[k].as_bool() {
            changes.push((
                if k == "bold" { "b" } else { "i" },
                if v { "1".into() } else { "0".into() },
            ));
        }
    }
    if let Some(value) = style["underline"].as_bool() {
        changes.push(("u",if value { if word {"single"} else {"sng"} } else {"none"}.to_string()));
    }
    if let Some(size) = style["size"].as_f64() {
        if !size.is_finite() || !(4.0..=200.0).contains(&size) {
            return Err("Font size must be between 4 and 200".into());
        }
        changes.push((
            "sz",
            ((size * if word { 2.0 } else { 100.0 }).round() as u32).to_string(),
        ));
    }
    if word {
        if properties.ends_with("/>") {
            properties.truncate(properties.len() - 2);
            properties.push_str(&format!("></{prefix}rPr>"));
        }
        for (k, v) in changes {
            let replacement = format!("<{prefix}{k} {prefix}val=\"{v}\"/>");
            if let Some((a, b, _)) = spans(&properties, k)?.first() {
                properties.replace_range(*a..*b, &replacement);
            } else {
                let at = properties.rfind("</").ok_or("Invalid run properties")?;
                properties.insert_str(at, &replacement);
            }
        }
    } else {
        let mut r = Reader::from_str(&properties);
        let e = match r.read_event().map_err(err)? {
            Event::Start(e) | Event::Empty(e) => e,
            _ => return Err("Invalid run properties".into()),
        };
        let mut opening = format!("<{}", e.name().as_ref());
        for a in e.attributes() {
            let a = a.map_err(err)?;
            if !changes.iter().any(|(k, _)| *k == a.key.as_ref()) {
                opening.push_str(&format!(
                    " {}=\"{}\"",
                    a.key.as_ref(),
                    escaped(&quick_xml::escape::unescape(&a.value).map_err(err)?)
                ));
            }
        }
        for (k, v) in changes {
            opening.push_str(&format!(" {k}=\"{v}\""));
        }
        opening.push_str(
            if properties[..r.buffer_position() as usize].ends_with("/>") {
                "/>"
            } else {
                ">"
            },
        );
        properties.replace_range(..r.buffer_position() as usize, &opening);
    }
    // Preserve unrelated properties while explicitly replacing theme-derived fonts/colors.
    let mut extra=Vec::new();
    if let Some(font)=style["font"].as_str(){
        let font=font.trim();if font.is_empty()||font.len()>128||font.chars().any(char::is_control){return Err("Invalid font family".into())}
        let font=escaped(font);
        if word {extra.push(("rFonts",format!("<{prefix}rFonts {prefix}ascii=\"{font}\" {prefix}hAnsi=\"{font}\" {prefix}eastAsia=\"{font}\" {prefix}cs=\"{font}\"/>")));}
        else {for tag in ["latin","ea","cs"]{extra.push((tag,format!("<{prefix}{tag} typeface=\"{font}\"/>")));}}
    }
    if let Some(color)=style["color"].as_str(){
        let color=color.trim_start_matches('#');if color.len()!=6||!color.bytes().all(|b|b.is_ascii_hexdigit()){return Err("Color must be a six-digit RGB value".into())}
        let color=color.to_ascii_uppercase();
        if word {extra.push(("color",format!("<{prefix}color {prefix}val=\"{color}\"/>")));}
        else {
            for tag in ["noFill","solidFill","gradFill","blipFill","pattFill","grpFill"]{for (a,b,_) in spans(&properties,tag)?.into_iter().rev(){properties.replace_range(a..b,"");}}
            extra.push(("solidFill",format!("<{prefix}solidFill><{prefix}srgbClr val=\"{color}\"/></{prefix}solidFill>")));
        }
    }
    if !extra.is_empty()&&properties.ends_with("/>"){properties.truncate(properties.len()-2);properties.push_str(&format!("></{prefix}rPr>"));}
    for (tag,replacement) in extra {
        if let Some((a,b,_))=spans(&properties,tag)?.first(){properties.replace_range(*a..*b,&replacement);}
        else {
            // Insert before later schema children rather than appending after extensions.
            let order=if word {vec!["rStyle","rFonts","b","bCs","i","iCs","caps","smallCaps","strike","dstrike","outline","shadow","emboss","imprint","noProof","snapToGrid","vanish","webHidden","color","spacing","w","kern","position","sz","szCs","highlight","u","effect","bdr","shd","fitText","vertAlign","rtl","cs","em","lang","eastAsianLayout","specVanish","oMath","rPrChange"]}else{vec!["ln","noFill","solidFill","gradFill","blipFill","pattFill","grpFill","effectLst","effectDag","highlight","uLnTx","uLn","uFillTx","uFill","latin","ea","cs","sym","hlinkClick","hlinkMouseOver","rtl","extLst"]};
            let rank=order.iter().position(|t|*t==tag).unwrap();
            let mut at=properties.rfind("</").ok_or("Invalid properties")?;
            for later in &order[rank+1..]{if let Some((a,_,_))=spans(&properties,later)?.first(){at=at.min(*a);}}
            properties.insert_str(at,&replacement);
        }
    }
    let mut out = run.to_string();
    if let Some((a, b, _)) = existing {
        out.replace_range(a..b, &properties);
    } else {
        let at = out.find('>').ok_or("Invalid run")? + 1;
        out.insert_str(at, &properties);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn package(path: &Path, entries: &[(&str, &str)]) {
        let mut w = ZipWriter::new(File::create(path).unwrap());
        for (n, v) in entries {
            w.start_file(*n, SimpleFileOptions::default()).unwrap();
            w.write_all(v.as_bytes()).unwrap();
        }
        w.finish().unwrap();
    }
    fn content(path: &Path, name: &str) -> String {
        let mut z = archive(path).unwrap();
        xml(&mut z, name).unwrap()
    }
    #[test]
    fn word_changes_only_selected_run_and_keeps_styles_media() {
        let t = tempfile::tempdir().unwrap();
        let a = t.path().join("a.docx");
        let b = t.path().join("b.docx");
        let s = r#"<w:document xmlns:w="urn:word"><w:body><w:p><w:r><w:rPr><w:b/><w:color w:val="FF0000"/></w:rPr><w:t>A &amp; 中文</w:t></w:r><w:r><w:t>Untouched</w:t></w:r></w:p></w:body></w:document>"#;
        package(
            &a,
            &[
                ("word/document.xml", s),
                ("word/media/image.png", "IMAGE"),
                ("word/styles.xml", "STYLE"),
            ],
        );
        let p = page(&a, 0, "", "").unwrap();
        assert_eq!(p.items[0].text, "A & 中文");
        edit(
            &a,
            &b,
            0,
            " 新 < & 文本 ",
            "text",
            &serde_json::json!({"size":18,"italic":true}),
        )
        .unwrap();
        let p = page(&b, 0, "", "").unwrap();
        assert_eq!(p.items[0].text, " 新 < & 文本 ");
        assert_eq!(p.items[1].text, "Untouched");
        let result = content(&b, "word/document.xml");
        assert!(result.contains("<w:b/>"));
        assert!(result.contains("w:val=\"FF0000\""));
        assert!(result.contains("w:sz w:val=\"36\""));
        assert_eq!(content(&b, "word/media/image.png"), "IMAGE");
        assert_eq!(content(&a, "word/document.xml"), s);
        assert!(edit(&a, &b, 0, "overwrite", "text", &serde_json::Value::Null).is_err());
    }
    #[test]
    fn shared_strings_are_not_modified_for_other_cells() {
        let t = tempfile::tempdir().unwrap();
        let a = t.path().join("a.xlsx");
        let b = t.path().join("b.xlsx");
        package(
            &a,
            &[
                ("xl/workbook.xml", "<workbook><sheets/></workbook>"),
                ("xl/sharedStrings.xml", "<sst><si><t>Shared</t></si></sst>"),
                (
                    "xl/worksheets/sheet1.xml",
                    r#"<worksheet><sheetData><row r="1"><c r="A1" t="s" s="7"><v>0</v></c><c r="B1" t="s"><v>0</v></c><c r="C1"><f>SUM(A2:A9)</f><v>12</v></c></row></sheetData></worksheet>"#,
                ),
            ],
        );
        edit(&a, &b, 0, "Changed", "text", &serde_json::Value::Null).unwrap();
        let p = page(&b, 0, "", "").unwrap();
        assert_eq!(p.items[0].text, "Changed");
        assert_eq!(p.items[1].text, "Shared");
        assert_eq!(p.items[2].text, "=SUM(A2:A9)");
        assert!(content(&b, "xl/worksheets/sheet1.xml").contains("s=\"7\""));
        assert!(content(&b, "xl/workbook.xml").contains("fullCalcOnLoad=\"1\""));
        assert_eq!(
            content(&a, "xl/sharedStrings.xml"),
            content(&b, "xl/sharedStrings.xml")
        );
    }
    #[test]
    fn formulas_are_explicit_and_protected_cells_stay_read_only() {
        let t = tempfile::tempdir().unwrap();
        let a = t.path().join("a.xlsx");
        let b = t.path().join("b.xlsx");
        package(
            &a,
            &[
                ("xl/workbook.xml", "<workbook/>"),
                (
                    "xl/worksheets/sheet1.xml",
                    r#"<worksheet><sheetData><row><c r="A1"><v>2</v></c></row></sheetData><sheetProtection sheet="1"/></worksheet>"#,
                ),
            ],
        );
        assert!(!page(&a, 0, "", "").unwrap().items[0].editable);
        assert!(edit(&a, &b, 0, "9", "number", &serde_json::Value::Null).is_err());
        assert!(formula("SUM(A1:A9)+ROUND(B2,2)").is_ok());
        assert!(formula("WEBSERVICE(\"https://example.com\")").is_err());
        assert!(formula("'[remote.xlsx]Sheet1'!A1").is_err());
    }
    #[test]
    fn slides_keep_other_objects_and_set_run_properties() {
        let t = tempfile::tempdir().unwrap();
        let a = t.path().join("a.pptx");
        let b = t.path().join("b.pptx");
        package(
            &a,
            &[(
                "ppt/slides/slide1.xml",
                r#"<p:sld xmlns:p="urn:p" xmlns:a="urn:a"><a:p><a:r><a:rPr lang="zh-CN"><a:solidFill><a:srgbClr val="123456"/></a:solidFill></a:rPr><a:t>Title</a:t></a:r></a:p><p:pic/></p:sld>"#,
            )],
        );
        edit(
            &a,
            &b,
            0,
            "New title",
            "text",
            &serde_json::json!({"bold":true,"size":24}),
        )
        .unwrap();
        let result = content(&b, "ppt/slides/slide1.xml");
        assert!(result.contains("b=\"1\""));
        assert!(result.contains("sz=\"2400\""));
        assert!(result.contains("<p:pic/>"));
        assert!(result.contains("val=\"123456\""));
        assert_eq!(page(&b, 0, "", "").unwrap().items[0].text, "New title");
    }
    #[test]
    fn multiline_edits_keep_run_styles_and_other_content() {
        let temp=tempfile::tempdir().unwrap();
        for fmt in ["docx","pptx"] {
            let input=temp.path().join(format!("in.{fmt}"));let output=temp.path().join(format!("out.{fmt}"));
            let (part,source)=if fmt=="docx" {("word/document.xml",r#"<w:document xmlns:w="urn:w"><w:body><w:p><w:r><w:rPr><w:b/><w:rFonts w:asciiTheme="minorHAnsi"/></w:rPr><w:t>Old</w:t></w:r><w:r><w:t>Untouched</w:t></w:r></w:p></w:body></w:document>"#)}else{("ppt/slides/slide1.xml",r#"<p:sld xmlns:p="urn:p" xmlns:a="urn:a"><a:p><a:r><a:rPr b="1"><a:solidFill><a:schemeClr val="accent1"/></a:solidFill></a:rPr><a:t>Old</a:t></a:r><a:r><a:t>Untouched</a:t></a:r></a:p><p:pic/></p:sld>"#)};
            package(&input,&[(part,source),("media/preserved.bin","DATA")]);
            edit(&input,&output,0,"One\r\n中文 & two","text",&serde_json::json!({"font":"Noto Sans CJK SC","color":"#123ABC"})).unwrap();
            let result=content(&output,part);assert!(result.contains("br/>"));assert!(result.contains("123ABC"));assert!(result.contains("Noto Sans CJK SC"));assert!(!result.contains("asciiTheme"));assert!(!result.contains("schemeClr"));
            assert_eq!(page(&output,0,"","").unwrap().items.iter().map(|i|i.text.as_str()).collect::<Vec<_>>(),vec!["One","中文 & two","Untouched"]);
            assert_eq!(content(&output,"media/preserved.bin"),"DATA");assert_eq!(content(&input,part),source);
            let invalid=temp.path().join(format!("bad.{fmt}"));assert!(edit(&input,&invalid,0,"x","text",&serde_json::json!({"color":"red"})).is_err());assert!(!invalid.exists());
        }
    }
    #[test]
    fn inserted_paragraph_keeps_content_without_copying_section_breaks_or_objects(){
        let t=tempfile::tempdir().unwrap();let input=t.path().join("in.docx");let output=t.path().join("out.docx");
        let source=r#"<w:document xmlns:w="urn:w"><w:body><w:p><w:pPr><w:jc w:val="center"/><w:numPr/><w:sectPr/></w:pPr><w:bookmarkStart w:id="7"/><w:r><w:rPr><w:b/></w:rPr><w:t>Original</w:t><w:drawing/></w:r></w:p><w:p><w:r><w:t>Next</w:t></w:r></w:p></w:body></w:document>"#;
        package(&input,&[("word/document.xml",source)]);
        edit(&input,&output,0,"Added paragraph","paragraph_after",&serde_json::json!({})).unwrap();
        let xml=content(&output,"word/document.xml");assert_eq!(spans(&xml,"p").unwrap().len(),3);
        for tag in ["sectPr","drawing","bookmarkStart"]{assert_eq!(spans(&xml,tag).unwrap().len(),1);}
        assert_eq!(spans(&xml,"numPr").unwrap().len(),2);assert_eq!(spans(&xml,"b").unwrap().len(),2);assert_eq!(spans(&xml,"jc").unwrap().len(),2);
        assert_eq!(page(&output,0,"","").unwrap().items.iter().map(|i|i.text.as_str()).collect::<Vec<_>>(),vec!["Original","Added paragraph","Next"]);
    }
    #[test]
    fn slide_order_changes_only_the_presentation_index(){
        let t=tempfile::tempdir().unwrap();let input=t.path().join("in.pptx");let output=t.path().join("out.pptx");
        package(&input,&[("ppt/presentation.xml",r#"<p:presentation xmlns:p="urn:p" xmlns:r="urn:r"><p:sldIdLst><p:sldId id="256" r:id="rId1"/><p:sldId id="257" r:id="rId2"/></p:sldIdLst></p:presentation>"#),("ppt/_rels/presentation.xml.rels",r#"<Relationships><Relationship Id="rId1" Target="slides/slide1.xml"/><Relationship Id="rId2" Target="slides/slide2.xml"/></Relationships>"#),("ppt/slides/slide1.xml","<slide><t>First</t></slide>"),("ppt/slides/slide2.xml","<slide><t>Second</t></slide>"),("ppt/media/image.png","IMAGE")]);
        edit(&input,&output,ITEM_LIMIT+1,"","slide_up",&serde_json::json!({"part":"ppt/slides/slide2.xml"})).unwrap();
        assert_eq!(page(&output,0,"","").unwrap().items.iter().map(|i|i.text.as_str()).collect::<Vec<_>>(),vec!["Second","First"]);
        for part in ["ppt/slides/slide1.xml","ppt/slides/slide2.xml","ppt/_rels/presentation.xml.rels","ppt/media/image.png"]{assert_eq!(content(&input,part),content(&output,part));}
        assert!(edit(&input,&t.path().join("invalid.pptx"),ITEM_LIMIT+1,"","slide_up",&serde_json::json!({"part":"ppt/slides/slide1.xml"})).is_err());
    }
    fn slide_fixture(path:&Path,linked:bool){
        package(path,&[
         ("[Content_Types].xml",r#"<Types><Override PartName="/ppt/slides/slide1.xml" ContentType="slide"/><Override PartName="/ppt/slides/slide2.xml" ContentType="slide"/><Override PartName="/ppt/notesSlides/notesSlide1.xml" ContentType="notes"/></Types>"#),
         ("_rels/.rels",r#"<Relationships><Relationship Id="rId1" Type="x/presentation" Target="ppt/presentation.xml"/></Relationships>"#),
         ("ppt/presentation.xml",r#"<p:presentation xmlns:p="urn:p" xmlns:r="urn:r"><p:sldIdLst><p:sldId id="256" r:id="rId1"/><p:sldId id="257" r:id="rId2"/></p:sldIdLst></p:presentation>"#),
         ("ppt/_rels/presentation.xml.rels",r#"<Relationships><Relationship Id="rId1" Type="x/slide" Target="slides/slide1.xml"/><Relationship Id="rId2" Type="x/slide" Target="slides/slide2.xml"/></Relationships>"#),
         ("ppt/slides/slide1.xml",r#"<slide><t>First</t><picture id="pic"/></slide>"#),
         ("ppt/slides/slide2.xml","<slide><t>Second</t></slide>"),
         ("ppt/slides/_rels/slide1.xml.rels",r#"<Relationships><Relationship Id="img" Type="x/image" Target="../media/image.png"/><Relationship Id="notes" Type="x/notesSlide" Target="../notesSlides/notesSlide1.xml"/></Relationships>"#),
         ("ppt/slides/_rels/slide2.xml.rels",if linked {r#"<Relationships><Relationship Id="link" Type="x/slide" Target="slide1.xml"/></Relationships>"#}else{"<Relationships></Relationships>"}),
         ("ppt/notesSlides/notesSlide1.xml","<notes><t>Speaker note</t></notes>"),
         ("ppt/notesSlides/_rels/notesSlide1.xml.rels",r#"<Relationships><Relationship Id="slide" Type="x/slide" Target="../slides/slide1.xml"/></Relationships>"#),
         ("ppt/media/image.png","IMAGE")]);
    }
    #[test]
    fn duplicate_slide_copies_notes_and_edits_independently_then_deletes_cleanly(){
        let t=tempfile::tempdir().unwrap();let input=t.path().join("in.pptx");let copy=t.path().join("copy.pptx");let changed=t.path().join("changed.pptx");let removed=t.path().join("removed.pptx");slide_fixture(&input,false);
        edit(&input,&copy,ITEM_LIMIT+1,"","slide_duplicate",&serde_json::json!({"part":"ppt/slides/slide1.xml"})).unwrap();
        let p=page(&copy,0,"","").unwrap();assert_eq!(p.parts,vec!["ppt/slides/slide1.xml","ppt/slides/slide3.xml","ppt/slides/slide2.xml"]);
        assert_eq!(content(&copy,"ppt/notesSlides/notesSlide2.xml"),content(&input,"ppt/notesSlides/notesSlide1.xml"));
        assert!(content(&copy,"ppt/notesSlides/_rels/notesSlide2.xml.rels").contains("/ppt/slides/slide3.xml"));assert!(content(&copy,"ppt/slides/_rels/slide3.xml.rels").contains("/ppt/notesSlides/notesSlide2.xml"));
        edit(&copy,&changed,1,"Only copy","text",&serde_json::json!({})).unwrap();assert_eq!(page(&changed,0,"","").unwrap().items[0].text,"First");assert_eq!(page(&changed,0,"","").unwrap().items[1].text,"Only copy");
        edit(&changed,&removed,ITEM_LIMIT+1,"","slide_delete",&serde_json::json!({"part":"ppt/slides/slide3.xml"})).unwrap();
        assert_eq!(page(&removed,0,"","").unwrap().parts.len(),2);let z=archive(&removed).unwrap();for part in ["ppt/slides/slide3.xml","ppt/slides/_rels/slide3.xml.rels","ppt/notesSlides/notesSlide2.xml","ppt/notesSlides/_rels/notesSlide2.xml.rels"]{assert!(!z.file_names().any(|n|n==part));}
        assert_eq!(content(&removed,"ppt/media/image.png"),"IMAGE");assert!(!content(&removed,"[Content_Types].xml").contains("slide3.xml"));
        let one=t.path().join("one.pptx");edit(&removed,&one,ITEM_LIMIT+1,"","slide_delete",&serde_json::json!({"part":"ppt/slides/slide1.xml"})).unwrap();assert_eq!(page(&one,0,"","").unwrap().parts.len(),1);
        let empty=t.path().join("empty.pptx");assert!(edit(&one,&empty,ITEM_LIMIT+1,"","slide_delete",&serde_json::json!({"part":"ppt/slides/slide2.xml"})).unwrap_err().contains("at least one"));assert!(!empty.exists());

    }
    #[test]
    fn deletion_rejects_incoming_links_without_writing_output(){
        let t=tempfile::tempdir().unwrap();let input=t.path().join("in.pptx");let output=t.path().join("out.pptx");slide_fixture(&input,true);
        assert!(edit(&input,&output,ITEM_LIMIT+1,"","slide_delete",&serde_json::json!({"part":"ppt/slides/slide1.xml"})).unwrap_err().contains("links to this slide"));assert!(!output.exists());assert_eq!(page(&input,0,"","").unwrap().parts.len(),2);
    }
    #[test]
    fn paragraph_alignment_preserves_runs_and_nested_revision_properties(){
        let t=tempfile::tempdir().unwrap();let input=t.path().join("in.docx");let output=t.path().join("out.docx");
        package(&input,&[("word/document.xml",r#"<w:document xmlns:w="urn:w"><w:body><w:p><w:pPr><w:spacing w:after="120"/><w:pPrChange><w:pPr><w:jc w:val="left"/></w:pPr></w:pPrChange></w:pPr><w:r><w:t>Keep</w:t></w:r><w:r><w:t>Also keep</w:t></w:r></w:p></w:body></w:document>"#)]);
        edit(&input,&output,0,"","paragraph_align",&serde_json::json!({"alignment":"center"})).unwrap();let result=content(&output,"word/document.xml");
        assert!(result.contains("<w:jc w:val=\"center\"/><w:pPrChange>"));assert!(result.contains("<w:jc w:val=\"left\"/>"));assert!(result.contains("w:after=\"120\""));assert_eq!(page(&output,0,"","").unwrap().items.len(),2);
        assert!(edit(&input,&t.path().join("invalid.docx"),0,"","paragraph_align",&serde_json::json!({"alignment":"bad"})).is_err());
    }
    #[test]
    fn paragraph_deletion_preserves_last_cell_and_rejects_structural_references(){
        let t=tempfile::tempdir().unwrap();let input=t.path().join("in.docx");let output=t.path().join("out.docx");
        package(&input,&[("word/document.xml",r#"<w:document xmlns:w="urn:w"><w:body><w:p><w:r><w:t>First</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p><w:r><w:t>Last</w:t></w:r></w:p></w:body></w:document>"#)]);
        edit(&input,&output,1,"","paragraph_delete",&serde_json::json!({})).unwrap();assert_eq!(page(&output,0,"","").unwrap().items.iter().map(|i|i.text.as_str()).collect::<Vec<_>>(),vec!["First","","Last"]);
        let deleted=t.path().join("deleted.docx");edit(&input,&deleted,0,"","paragraph_delete",&serde_json::json!({})).unwrap();assert_eq!(page(&deleted,0,"","").unwrap().items.iter().map(|i|i.text.as_str()).collect::<Vec<_>>(),vec!["Cell","Last"]);
        for (index,marker) in ["<w:sectPr/>","<w:bookmarkStart/>","<w:commentRangeEnd/>","<w:fldChar/>","<w:footnoteReference/>","<w:ins/>"] .iter().enumerate(){
            let source=t.path().join(format!("guard-{index}.docx"));package(&source,&[("word/document.xml",&format!("<w:document xmlns:w=\"urn:w\"><w:body><w:p>{marker}<w:r><w:t>Guard</w:t></w:r></w:p></w:body></w:document>"))]);let target=t.path().join(format!("guard-out-{index}.docx"));assert!(edit(&source,&target,0,"","paragraph_delete",&serde_json::json!({})).is_err());assert!(!target.exists());
        }
    }
    #[test]
    fn paragraph_delete_guards_enclosing_revision_and_nested_table_tail() {
        let t=tempfile::tempdir().unwrap();let input=t.path().join("in.docx");let output=t.path().join("out.docx");
        package(&input,&[("word/document.xml",r#"<w:document xmlns:w="urn:w"><w:body><w:ins><w:p><w:r><w:t>Revision</w:t></w:r></w:p></w:ins></w:body></w:document>"#)]);
        assert!(edit(&input,&output,0,"","paragraph_delete",&serde_json::json!({})).is_err());assert!(!output.exists());
        package(&input,&[("word/document.xml",r#"<w:document xmlns:w="urn:w"><w:body><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Before</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Nested</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p><w:r><w:t>Tail</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:body></w:document>"#)]);
        edit(&input,&output,2,"","paragraph_delete",&serde_json::json!({})).unwrap();
        assert!(content(&output,"word/document.xml").contains("</w:tbl><w:p><w:r><w:t xml:space=\"preserve\"></w:t></w:r></w:p></w:tc>"));
    }
    #[test]
    fn cell_styles_are_isolated_and_reapplying_does_not_grow_resources(){
        let t=tempfile::tempdir().unwrap();let input=t.path().join("in.xlsx");let output=t.path().join("out.xlsx");
        package(&input,&[("[Content_Types].xml","<Types/>"),("xl/_rels/workbook.xml.rels",r#"<Relationships><Relationship Id="sheet" Type="x/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#),("xl/workbook.xml",r#"<workbook xmlns:r="urn:r"><sheets><sheet name="Sheet" sheetId="1" r:id="sheet"/></sheets></workbook>"#),("xl/worksheets/sheet1.xml",r#"<worksheet><sheetData><row r="1"><c r="A1"><v>1.25</v></c><c r="B1"><v>7</v></c></row></sheetData></worksheet>"#),("media/keep.bin","SHARED")]);
        let style=serde_json::json!({"bold":true,"color":"#227744","fill":"#FFF2CC","numberFormat":"percent","horizontal":"center","wrap":true});
        edit(&input,&output,0,"1.25","number",&style).unwrap();let sheet=content(&output,"xl/worksheets/sheet1.xml");assert!(sheet.contains(r#"<c r="B1"><v>7</v></c>"#));let styles=content(&output,"xl/styles.xml");let fonts=spans(&styles,"font").unwrap();assert_eq!(fonts.len(),2);assert!(!styles[fonts[0].0..fonts[0].1].contains("227744"));assert!(styles.contains("FF227744"));assert!(styles.contains("FFFFF2CC"));assert!(styles.contains(r#"numFmtId="10""#));assert_eq!(content(&output,"media/keep.bin"),"SHARED");
        let again=t.path().join("again.xlsx");edit(&output,&again,0,"1.25","number",&style).unwrap();assert_eq!(content(&again,"xl/styles.xml"),styles);
        let bad=t.path().join("bad.xlsx");assert!(edit(&input,&bad,0,"1.25","number",&serde_json::json!({"numberFormat":"unsupported"})).is_err());assert!(!bad.exists());
    }
    #[test]
    fn word_lists_have_independent_definitions_and_inserted_items_continue(){
        let t=tempfile::tempdir().unwrap();let input=t.path().join("in.docx");let first=t.path().join("first.docx");let second=t.path().join("second.docx");let inserted=t.path().join("inserted.docx");
        package(&input,&[("[Content_Types].xml","<Types/>"),("word/document.xml",r#"<w:document xmlns:w="urn:w"><w:body><w:p><w:r><w:t>One</w:t></w:r></w:p><w:p><w:r><w:t>Two</w:t></w:r></w:p></w:body></w:document>"#)]);
        edit(&input,&first,0,"","paragraph_list",&serde_json::json!({"list":"bullet"})).unwrap();edit(&first,&second,1,"","paragraph_list",&serde_json::json!({"list":"decimal"})).unwrap();
        let numbering=content(&second,"word/numbering.xml");assert_eq!(spans(&numbering,"abstractNum").unwrap().len(),2);assert_eq!(spans(&numbering,"num").unwrap().len(),2);
        edit(&second,&inserted,1,"Next","paragraph_after",&serde_json::json!({})).unwrap();let document=content(&inserted,"word/document.xml");let ids=spans(&document,"numId").unwrap().iter().map(|(a,b,_)|slides::attribute(&document[*a..*b],"w:val").unwrap()).collect::<Vec<_>>();assert_eq!(ids,vec!["1","2","2"]);
        let cleared=t.path().join("cleared.docx");edit(&inserted,&cleared,2,"","paragraph_list",&serde_json::json!({"list":"none"})).unwrap();assert!(content(&cleared,"word/document.xml").contains(r#"<w:numId w:val="0"/>"#));
    }
    #[test]
    fn invalid_xml_entities_are_rejected() {
        let t = tempfile::tempdir().unwrap();
        let a = t.path().join("a.docx");
        package(
            &a,
            &[(
                "word/document.xml",
                r#"<!DOCTYPE document [<!ENTITY secret SYSTEM "file:///etc/passwd">]><document><t>&secret;</t></document>"#,
            )],
        );
        assert!(page(&a, 0, "", "").is_err());
    }
    #[test]
    fn insert_cell_orders_rows_and_preserves_other_values() {
        let t = tempfile::tempdir().unwrap();
        let a = t.path().join("a.xlsx");
        let b = t.path().join("b.xlsx");
        package(
            &a,
            &[
                ("xl/workbook.xml", "<workbook><sheets/></workbook>"),
                (
                    "xl/worksheets/sheet1.xml",
                    r#"<worksheet><dimension ref="A1:C3"/><sheetData><row r="1"><c r="A1"><v>1</v></c><c r="C1"><v>3</v></c></row><row r="3"><c r="A3"><v>4</v></c></row></sheetData></worksheet>"#,
                ),
            ],
        );
        edit(
            &a,
            &b,
            ITEM_LIMIT,
            "=SUM(A1,C1)",
            "formula",
            &serde_json::json!({"part":"xl/worksheets/sheet1.xml","address":"B1"}),
        )
        .unwrap();
        let p = page(&b, 0, "", "").unwrap();
        assert_eq!(
            p.items.iter().map(|i| i.label.as_str()).collect::<Vec<_>>(),
            vec!["A1", "B1", "C1", "A3"]
        );
        assert_eq!(p.items[1].text, "=SUM(A1,C1)");
        let c = t.path().join("c.xlsx");
        edit(
            &b,
            &c,
            ITEM_LIMIT,
            "20",
            "number",
            &serde_json::json!({"part":"xl/worksheets/sheet1.xml","address":"D2"}),
        )
        .unwrap();
        assert_eq!(page(&c, 0, "D2", "").unwrap().items[0].text, "20");
        assert!(coordinate("XFE1").is_err());
        assert!(coordinate("A1048577").is_err());
        assert!(coordinate("A0").is_err());
    }
    #[test]
    fn pagination_is_bounded_and_search_keeps_stable_ids() {
        let t = tempfile::tempdir().unwrap();
        let a = t.path().join("a.docx");
        let s = format!(
            "<document>{}</document>",
            (0..200)
                .map(|n| format!("<t>Item{n}</t>"))
                .collect::<String>()
        );
        package(&a, &[("word/document.xml", &s)]);
        assert_eq!(page(&a, 0, "", "").unwrap().items.len(), 80);
        assert_eq!(page(&a, 80, "", "").unwrap().items[0].id, 80);
        let p = page(&a, 0, "Item199", "").unwrap();
        assert_eq!(p.total, 1);
        assert_eq!(p.items[0].id, 199);
    }
}

fn coordinate(value: &str) -> Result<(u32, u32), String> {
    let split = value
        .find(|c: char| c.is_ascii_digit())
        .ok_or("Enter a cell address such as A1")?;
    let (col, row) = value.split_at(split);
    if col.is_empty()
        || col.len() > 3
        || !col.chars().all(|c| c.is_ascii_uppercase())
        || row.starts_with('0')
    {
        return Err("Invalid cell address".into());
    }
    let column = col
        .bytes()
        .fold(0u32, |n, c| n * 26 + u32::from(c - b'A' + 1));
    let row = row.parse::<u32>().map_err(|_| "Invalid cell row")?;
    if column > 16384 || row == 0 || row > 1048576 {
        return Err("Cell address exceeds Excel limits".into());
    }
    Ok((row, column))
}
fn insert_cell(
    input: &Path,
    output: &Path,
    value: &str,
    kind: &str,
    style: &serde_json::Value,
) -> Result<(), String> {
    if format(input) != "xlsx" {
        return Err("Adding cells requires an XLSX document".into());
    }
    let part = style["part"].as_str().ok_or("Select a worksheet")?;
    let address = style["address"]
        .as_str()
        .ok_or("Enter a cell address")?
        .trim()
        .to_ascii_uppercase();
    let (row, column) = coordinate(&address)?;
    let mut z = archive(input)?;
    if !main_parts(&mut z, "xlsx")?.iter().any(|(p, _)| p == part) {
        return Err("Invalid worksheet".into());
    }
    let mut s = xml(&mut z, part)?;
    if !spans(&s, "sheetProtection")?.is_empty() {
        return Err("Protected worksheets are read-only".into());
    }
    for (a, b, _) in spans(&s, "mergeCell")? {
        let mut r = Reader::from_str(&s[a..b]);
        if let Event::Empty(e) | Event::Start(e) = r.read_event().map_err(err)? {
            let range = attr(&e, "ref")?;
            if let Some((first, last)) = range.split_once(':') {
                let (r1, c1) = coordinate(first)?;
                let (r2, c2) = coordinate(last)?;
                if row >= r1 && row <= r2 && column >= c1 && column <= c2 && address != first {
                    return Err("Use the top-left cell of a merged range".into());
                }
            }
        }
    }
    let cells = spans(&s, "c")?;
    if cells.iter().any(|(_, _, label)| *label == address) {
        return Err("This cell already exists; find its address to edit it".into());
    }
    let (a, b, _) = spans(&s, "sheetData")?
        .into_iter()
        .next()
        .ok_or("Worksheet has no editable cell data")?;
    let mut data = s[a..b].to_string();
    let mut r = Reader::from_str(&data);
    let e = match r.read_event().map_err(err)? {
        Event::Start(e) | Event::Empty(e) => e,
        _ => return Err("Invalid sheet data".into()),
    };
    let name = e.name().as_ref().to_string();
    let prefix = name.strip_suffix("sheetData").unwrap_or("");
    if data.ends_with("/>") {
        data.truncate(data.len() - 2);
        data.push_str(&format!("></{name}>"));
    }
    let cell = format!("<{prefix}c r=\"{address}\"/>");
    let rows = spans(&data, "row")?;
    if let Some((ra, rb, _)) = rows
        .iter()
        .find(|(_, _, v)| v.parse::<u32>().ok() == Some(row))
    {
        let mut content = data[*ra..*rb].to_string();
        if content.ends_with("/>") {
            content.truncate(content.len() - 2);
            content.push_str(&format!("></{prefix}row>"));
        }
        let at = spans(&content, "c")?
            .iter()
            .find_map(|(a, _, v)| coordinate(v).ok().filter(|(_, c)| *c > column).map(|_| *a))
            .unwrap_or(content.rfind("</").ok_or("Invalid row")?);
        content.insert_str(at, &cell);
        data.replace_range(*ra..*rb, &content);
    } else {
        let at = rows
            .iter()
            .find_map(|(a, _, v)| v.parse::<u32>().ok().filter(|r| *r > row).map(|_| *a))
            .unwrap_or(data.rfind("</").ok_or("Invalid sheet data")?);
        data.insert_str(
            at,
            &format!("<{prefix}row r=\"{row}\">{cell}</{prefix}row>"),
        );
    }
    s.replace_range(a..b, &data);
    if let Some((a, b, _)) = spans(&s, "dimension")?.first() {
        s.replace_range(*a..*b, "");
    }
    let temp = tempfile::tempdir_in(output.parent().ok_or("Invalid output")?).map_err(err)?;
    let intermediate = temp.path().join("insert.xlsx");
    write_package(
        z,
        &intermediate,
        [(part.to_string(), s)].into_iter().collect(),
    )?;
    let id = items(&intermediate)?
        .into_iter()
        .find(|i| i.part == part && i.label == address)
        .ok_or("New cell could not be validated")?
        .id;
    edit(
        &intermediate,
        output,
        id,
        value,
        kind,
        &serde_json::Value::Null,
    )
}
