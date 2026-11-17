//! Structural slide edits preserve shared media and clone per-slide notes.
use super::*;
use std::collections::{HashMap, HashSet};
pub(super) fn attribute(fragment: &str, key: &str) -> Result<String, String> {
    let mut r = Reader::from_str(fragment);
    match r.read_event().map_err(err)? {
        Event::Start(e) | Event::Empty(e) => attr(&e, key),
        _ => Err("Invalid XML element".into()),
    }
}
pub(super) fn set_attribute(fragment: &str, key: &str, value: &str) -> Result<String, String> {
    let mut r = Reader::from_str(fragment);
    let event = r.read_event().map_err(err)?;
    let e = match &event {
        Event::Start(e) | Event::Empty(e) => e,
        _ => return Err("Invalid XML element".into()),
    };
    let mut opening = format!("<{}", e.name().as_ref());
    let mut found = false;
    for a in e.attributes() {
        let a = a.map_err(err)?;
        let name = a.key.as_ref();
        let old = quick_xml::escape::unescape(&a.value).map_err(err)?;
        let v = if name == key {
            found = true;
            value
        } else {
            &old
        };
        opening.push_str(&format!(" {name}=\"{}\"", escaped(v)));
    }
    if !found {
        opening.push_str(&format!(" {key}=\"{}\"", escaped(value)));
    }
    opening.push_str(if matches!(event, Event::Empty(_)) {
        "/>"
    } else {
        ">"
    });
    opening.push_str(&fragment[r.buffer_position() as usize..]);
    Ok(opening)
}
pub(super) fn append_child(parent: &mut String, child: &str) -> Result<(), String> {
    if parent.trim_end().ends_with("/>") {
        let mut r = Reader::from_str(parent);
        let name = loop {
            match r.read_event().map_err(err)? {
                Event::Empty(e) => break e.name().as_ref().to_string(),
                Event::Decl(_) => {}
                _ => return Err("Invalid empty XML container".into()),
            }
        };
        let at = parent.rfind("/>").ok_or("Invalid XML container")?;
        parent.replace_range(at.., &format!(">{child}</{name}>"));
        return Ok(());
    }
    let at = parent.rfind("</").ok_or("Unsupported XML container")?;
    parent.insert_str(at, child);
    Ok(())
}
fn relationship_path(part: &str) -> Result<String, String> {
    let (dir, name) = part.rsplit_once('/').ok_or("Invalid part path")?;
    Ok(format!("{dir}/_rels/{name}.rels"))
}
fn resolve_target(part: &str, target: &str) -> Result<String, String> {
    if target.contains(['\\', '?', '#', '%']) {
        return Err("Unsupported relationship target".into());
    }
    let mut pieces = if target.starts_with('/') {
        vec![]
    } else {
        part.split('/').map(str::to_string).collect::<Vec<_>>()
    };
    if !target.starts_with('/') {
        pieces.pop();
    }
    for piece in target.split('/') {
        match piece {
            "" | "." => {}
            ".." => {
                pieces.pop().ok_or("Relationship escapes package")?;
            }
            p => pieces.push(p.into()),
        }
    }
    Ok(pieces.join("/"))
}
fn free_part(names: &HashSet<String>, prefix: &str) -> Result<String, String> {
    for n in 1..=20000 {
        let p = format!("{prefix}{n}.xml");
        if !names.contains(&p) {
            return Ok(p);
        }
    }
    Err("Too many slide parts".into())
}
fn override_copy(types: &str, old: &str, new: &str) -> Result<String, String> {
    for (a, b, _) in spans(types, "Override")? {
        let f = &types[a..b];
        if attribute(f, "PartName")? == format!("/{old}") {
            return set_attribute(f, "PartName", &format!("/{new}"));
        }
    }
    Err("Missing slide content type".into())
}
pub(super) fn edit(
    input: &Path,
    output: &Path,
    kind: &str,
    style: &serde_json::Value,
) -> Result<(), String> {
    if format(input) != "pptx" {
        return Err("Slide editing requires a PPTX document".into());
    }
    let mut z = archive(input)?;
    let ordered = main_parts(&mut z, "pptx")?;
    let part = style["part"].as_str().ok_or("Select a slide")?;
    let index = ordered
        .iter()
        .position(|(p, _)| p == part)
        .ok_or("Invalid slide")?;
    let mut presentation = xml(&mut z, "ppt/presentation.xml")?;
    let ids = spans(&presentation, "sldId")?;
    if ids.len() != ordered.len() {
        return Err("This presentation uses extended slide sections; structural editing is not supported yet".into());
    }
    let (a, b, _) = ids[index];
    let selected = presentation[a..b].to_string();
    let rid = attribute(&selected, "r:id")?;
    if rid.is_empty() {
        return Err("Missing slide relationship".into());
    }
    let names = z.file_names().map(str::to_owned).collect::<HashSet<_>>();
    let relpath = relationship_path(part)?;
    let slide_rels = if names.contains(&relpath) {
        xml(&mut z, &relpath)?
    } else {
        String::new()
    };
    let mut notes = None;
    for (a, b, _) in spans(&slide_rels, "Relationship")? {
        let f = &slide_rels[a..b];
        let typ = attribute(f, "Type")?;
        if typ.ends_with("/comments") || typ.ends_with("/comment") || typ.ends_with("/tags") {
            return Err("This slide contains comments or custom tags; structural editing is not supported yet".into());
        }
        if typ.ends_with("/notesSlide") {
            if notes.is_some() {
                return Err("Duplicate notes relationship".into());
            }
            notes = Some(resolve_target(part, &attribute(f, "Target")?)?);
        }
    }
    let mut relations = xml(&mut z, "ppt/_rels/presentation.xml.rels")?;
    let mut types = xml(&mut z, "[Content_Types].xml")?;
    let mut changes = HashMap::<String, Option<String>>::new();
    match kind {
        "slide_duplicate" | "slide_blank" => {
            let new_part = free_part(&names, "ppt/slides/slide")?;
            let mut new_rels = slide_rels.clone();
            if kind == "slide_blank" {
                for (a, b, _) in spans(&new_rels, "Relationship")?.into_iter().rev() {
                    if !attribute(&new_rels[a..b], "Type")?.ends_with("/slideLayout") {
                        new_rels.replace_range(a..b, "");
                    }
                }
            }
            if let Some(note) = notes.as_ref().filter(|_| kind == "slide_duplicate") {
                let new_note = free_part(&names, "ppt/notesSlides/notesSlide")?;
                changes.insert(new_note.clone(), Some(xml(&mut z, note)?));
                let note_rels = relationship_path(note)?;
                let mut content = xml(&mut z, &note_rels)?;
                let mut backlinks = 0;
                for (a, b, _) in spans(&content, "Relationship")?.into_iter().rev() {
                    let f = &content[a..b];
                    if attribute(f, "Type")?.ends_with("/slide") {
                        backlinks += 1;
                        let new = set_attribute(f, "Target", &format!("/{new_part}"))?;
                        content.replace_range(a..b, &new);
                    }
                }
                if backlinks != 1 {
                    return Err("Unsupported notes slide backlink".into());
                }
                changes.insert(relationship_path(&new_note)?, Some(content));
                for (a, b, _) in spans(&new_rels, "Relationship")?.into_iter().rev() {
                    let f = &new_rels[a..b];
                    if attribute(f, "Type")?.ends_with("/notesSlide") {
                        let new = set_attribute(f, "Target", &format!("/{new_note}"))?;
                        new_rels.replace_range(a..b, &new);
                    }
                }
                let entry = override_copy(&types, note, &new_note)?;
                append_child(&mut types, &entry)?;
            }
            let content = if kind == "slide_blank" {
                r#"<?xml version="1.0" encoding="UTF-8"?><p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" showMasterSp="0"><p:cSld><p:bg><p:bgPr><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill><a:effectLst/></p:bgPr></p:bg><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"#.to_string()
            } else {
                xml(&mut z, part)?
            };
            changes.insert(new_part.clone(), Some(content));
            if !new_rels.is_empty() {
                changes.insert(relationship_path(&new_part)?, Some(new_rels));
            }
            let entry = override_copy(&types, part, &new_part)?;
            append_child(&mut types, &entry)?;
            let rel_ids = spans(&relations, "Relationship")?
                .iter()
                .map(|(a, b, _)| attribute(&relations[*a..*b], "Id"))
                .collect::<Result<HashSet<_>, _>>()?;
            let new_rid = (1..=20001)
                .map(|n| format!("bpdfSlide{n}"))
                .find(|r| !rel_ids.contains(r))
                .ok_or("Too many relationships")?;
            let original = spans(&relations, "Relationship")?
                .into_iter()
                .find(|(a, b, _)| attribute(&relations[*a..*b], "Id").ok().as_deref() == Some(&rid))
                .ok_or("Missing slide relationship")?;
            let rel = set_attribute(
                &set_attribute(&relations[original.0..original.1], "Id", &new_rid)?,
                "Target",
                &format!("/{new_part}"),
            )?;
            append_child(&mut relations, &rel)?;
            let max = ids
                .iter()
                .map(|(a, b, _)| {
                    attribute(&presentation[*a..*b], "id")?
                        .parse::<u32>()
                        .map_err(err)
                })
                .collect::<Result<Vec<_>, String>>()?
                .into_iter()
                .max()
                .unwrap_or(255);
            if max >= 2147483647 {
                return Err("Slide ID limit reached".into());
            }
            let new = set_attribute(
                &set_attribute(&selected, "id", &(max + 1).to_string())?,
                "r:id",
                &new_rid,
            )?;
            presentation.insert_str(b, &new);
        }
        "slide_delete" => {
            if ordered.len() <= 1 {
                return Err("Keep at least one slide in the presentation".into());
            }
            let mut removed = HashSet::from([part.to_string(), relpath.clone()]);
            if let Some(note) = &notes {
                removed.insert(note.clone());
                removed.insert(relationship_path(note)?);
            }
            // Refuse incoming slide links rather than leave broken navigation or delete
            // shared notes. Layouts/media remain shared and untouched.
            for name in names.iter().filter(|n| {
                n.ends_with(".rels")
                    && !removed.contains(*n)
                    && n.as_str() != "ppt/_rels/presentation.xml.rels"
            }) {
                let owner = if name == "_rels/.rels" {
                    String::new()
                } else {
                    let (dir, file) = name
                        .rsplit_once("/_rels/")
                        .ok_or("Unsupported relationship path")?;
                    format!("{dir}/{}", file.trim_end_matches(".rels"))
                };
                let content = xml(&mut z, name)?;
                for (a, b, _) in spans(&content, "Relationship")? {
                    let f = &content[a..b];
                    if attribute(f, "TargetMode")? == "External" {
                        continue;
                    }
                    let target = resolve_target(&owner, &attribute(f, "Target")?)?;
                    if removed.contains(&target) {
                        return Err("Another part links to this slide or its notes. Remove that link before deleting the slide.".into());
                    }
                }
            }
            presentation.replace_range(a..b, "");
            // Remove the slide from custom slide shows, retaining other entries.
            for (a, b, _) in spans(&presentation, "sld")?.into_iter().rev() {
                if attribute(&presentation[a..b], "r:id")? == rid {
                    presentation.replace_range(a..b, "");
                }
            }
            for (a, b, _) in spans(&relations, "Relationship")?.into_iter().rev() {
                if attribute(&relations[a..b], "Id")? == rid {
                    relations.replace_range(a..b, "");
                }
            }
            for (a, b, _) in spans(&types, "Override")?.into_iter().rev() {
                if removed.contains(attribute(&types[a..b], "PartName")?.trim_start_matches('/')) {
                    types.replace_range(a..b, "");
                }
            }
            for name in removed {
                changes.insert(name, None);
            }
        }
        _ => return Err("Unknown slide action".into()),
    }
    changes.insert("ppt/presentation.xml".into(), Some(presentation));
    changes.insert("ppt/_rels/presentation.xml.rels".into(), Some(relations));
    changes.insert("[Content_Types].xml".into(), Some(types));
    write_package_changes(z, output, changes)
}

/// Add an independent, editable text shape without modifying shared layout resources.
pub(super) fn add_text(
    input: &Path,
    output: &Path,
    value: &str,
    style: &serde_json::Value,
) -> Result<(), String> {
    if format(input) != "pptx" {
        return Err("Text boxes require PowerPoint compatibility mode".into());
    }
    if value.trim().is_empty() {
        return Err("Enter text for the new text box".into());
    }
    let mut z = archive(input)?;
    let part = style["part"].as_str().ok_or("Select a slide")?;
    if !main_parts(&mut z, "pptx")?.iter().any(|(p, _)| p == part) {
        return Err("Invalid slide".into());
    }
    let mut slide = xml(&mut z, part)?;
    let trees = spans(&slide, "spTree")?;
    if trees.len() != 1 {
        return Err("Unsupported slide shape tree".into());
    }
    let (a, b, _) = trees[0];
    if !slide[a..b].starts_with("<p:spTree") {
        return Err("Unsupported slide namespace".into());
    }
    let id = spans(&slide, "cNvPr")?
        .iter()
        .map(|(a, b, _)| attribute(&slide[*a..*b], "id")?.parse::<u32>().map_err(err))
        .collect::<Result<Vec<_>, String>>()?
        .into_iter()
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or("Shape ID limit reached")?;
    let presentation = xml(&mut z, "ppt/presentation.xml")?;
    let size = spans(&presentation, "sldSz")?
        .into_iter()
        .next()
        .ok_or("Missing slide size")?;
    let width = attribute(&presentation[size.0..size.1], "cx")?
        .parse::<u64>()
        .map_err(err)?;
    let height = attribute(&presentation[size.0..size.1], "cy")?
        .parse::<u64>()
        .map_err(err)?;
    if width < 12700 || height < 12700 || width > 182880000 || height > 182880000 {
        return Err("Unsupported slide dimensions".into());
    }
    let x = width / 20;
    let y = height / 20;
    let w = width - x * 2;
    let h = height - y * 2;
    let text=escaped(&value.replace("\r\n","\n").replace('\r',"\n")).split('\n').map(|line|format!("<a:p><a:r><a:rPr lang=\"en-US\" sz=\"2400\"><a:solidFill><a:srgbClr val=\"222222\"/></a:solidFill></a:rPr><a:t>{line}</a:t></a:r></a:p>")).collect::<String>();
    let shape = format!(
        r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="Text {id}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{w}" cy="{h}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/><a:ln><a:noFill/></a:ln></p:spPr><p:txBody><a:bodyPr wrap="square"><a:spAutoFit/></a:bodyPr><a:lstStyle/>{text}</p:txBody></p:sp>"#
    );
    let mut tree = slide[a..b].to_string();
    append_child(&mut tree, &shape)?;
    slide.replace_range(a..b, &tree);
    write_package(z, output, [(part.to_string(), slide)].into_iter().collect())
}
