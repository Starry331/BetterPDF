//! Paragraph operations retain surrounding OOXML and guard structural boundaries.
use super::*;
pub(super) fn children(s: &str, tag: &str) -> Result<Vec<(usize, usize)>, String> {
    let mut reader = Reader::from_str(s);
    let (mut depth, mut start) = (0usize, None);
    let mut result = Vec::new();
    loop {
        let at = reader.buffer_position() as usize;
        match reader.read_event().map_err(err)? {
            Event::Start(e) => {
                if depth == 1 && e.local_name().as_ref() == tag {
                    start = Some(at);
                }
                depth += 1;
            }
            Event::Empty(e) => {
                if depth == 1 && e.local_name().as_ref() == tag {
                    result.push((at, reader.buffer_position() as usize));
                }
            }
            Event::End(_) => {
                depth = depth.checked_sub(1).ok_or("Unbalanced paragraph XML")?;
                if depth == 1 {
                    if let Some(begin) = start.take() {
                        result.push((begin, reader.buffer_position() as usize));
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(result)
}
pub(super) fn change(
    s: &str,
    item: &Item,
    kind: &str,
    style: &serde_json::Value,
) -> Result<String, String> {
    let (a, b, _) = spans(s, "p")?
        .into_iter()
        .find(|(a, b, _)| *a <= item.start && *b >= item.end)
        .ok_or("Select text inside a paragraph")?;
    let paragraph = &s[a..b];
    let mut reader = Reader::from_str(paragraph);
    let name = match reader.read_event().map_err(err)? {
        Event::Start(e) => e.name().as_ref().to_string(),
        _ => return Err("Invalid paragraph".into()),
    };
    if name != "w:p" {
        return Err(
            "This paragraph uses an unsupported namespace or drawing text container".into(),
        );
    }
    let replacement = match kind {
        "paragraph_align" | "paragraph_numbering" => {
            let numbering = kind == "paragraph_numbering";
            let alignment = style["alignment"].as_str().unwrap_or("");
            if !numbering && !["left", "center", "right", "both"].contains(&alignment) {
                return Err("Invalid paragraph alignment".into());
            }
            let properties = children(paragraph, "pPr")?.into_iter().next();
            let mut props = properties
                .map(|(a, b)| paragraph[a..b].to_string())
                .unwrap_or_else(|| "<w:pPr/>".into());
            if props.ends_with("/>") {
                props.truncate(props.len() - 2);
                props.push_str("></w:pPr>");
            }
            let target = if numbering { "numPr" } else { "jc" };
            let jc = if numbering {
                let id = style["numId"].as_u64().ok_or("Missing list identifier")?;
                format!("<w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"{id}\"/></w:numPr>")
            } else {
                format!("<w:jc w:val=\"{alignment}\"/>")
            };
            if let Some((a, b)) = children(&props, target)?.first() {
                props.replace_range(*a..*b, &jc);
            } else {
                let mut at = props.rfind("</").ok_or("Invalid paragraph properties")?;
                if numbering {
                    for tag in [
                        "suppressLineNumbers",
                        "pBdr",
                        "shd",
                        "tabs",
                        "suppressAutoHyphens",
                        "kinsoku",
                        "wordWrap",
                        "overflowPunct",
                        "topLinePunct",
                        "autoSpaceDE",
                        "autoSpaceDN",
                        "bidi",
                        "adjustRightInd",
                        "snapToGrid",
                        "spacing",
                        "ind",
                        "contextualSpacing",
                        "mirrorIndents",
                        "suppressOverlap",
                        "jc",
                    ] {
                        if let Some((a, _)) = children(&props, tag)?.first() {
                            at = at.min(*a);
                        }
                    }
                }
                for tag in [
                    "textDirection",
                    "textAlignment",
                    "textboxTightWrap",
                    "outlineLvl",
                    "divId",
                    "cnfStyle",
                    "rPr",
                    "sectPr",
                    "pPrChange",
                ] {
                    if let Some((a, _)) = children(&props, tag)?.first() {
                        at = at.min(*a);
                    }
                }
                props.insert_str(at, &jc);
            }
            let mut result = paragraph.to_string();
            if let Some((a, b)) = properties {
                result.replace_range(a..b, &props);
            } else {
                let at = result.find('>').ok_or("Invalid paragraph")? + 1;
                result.insert_str(at, &props);
            }
            result
        }
        "paragraph_delete" => {
            // A revision wrapper may be outside the paragraph itself.
            for tag in ["ins", "del", "moveFrom", "moveTo"] {
                if spans(s, tag)?.iter().any(|(x, y, _)| *x < a && *y > b) {
                    return Err(
                        "Resolve enclosing tracked changes before deleting this paragraph.".into(),
                    );
                }
            }
            for tag in [
                "sectPr",
                "bookmarkStart",
                "bookmarkEnd",
                "commentRangeStart",
                "commentRangeEnd",
                "commentReference",
                "footnoteReference",
                "endnoteReference",
                "fldChar",
                "ins",
                "del",
                "moveFrom",
                "moveTo",
            ] {
                if !spans(paragraph, tag)?.is_empty() {
                    return Err("This paragraph contains section boundaries, references or tracked changes. Resolve them before deleting the paragraph.".into());
                }
            }
            let mut containers = Vec::new();
            for tag in ["body", "tc", "txbxContent", "sdtContent"] {
                containers.extend(
                    spans(s, tag)?
                        .into_iter()
                        .filter(|(x, y, _)| *x <= a && *y >= b),
                );
            }
            let (ca, cb, _) = containers
                .into_iter()
                .min_by_key(|(x, y, _)| y - x)
                .ok_or("Unsupported paragraph container")?;
            // A cell/body must remain editable after deleting its last paragraph.
            let container = &s[ca..cb];
            let paragraphs = children(container, "p")?;
            // A table cell must end in a paragraph, including after a nested table.
            let follows_table = paragraphs.last().is_some_and(|(x, _)| ca + x == a)
                && children(container, "tbl")?.iter().any(|(_, end)| {
                    *end <= a - ca && !paragraphs.iter().any(|(x, y)| *x >= *end && ca + y <= a)
                });
            if paragraphs.len() <= 1 || follows_table {
                "<w:p><w:r><w:t xml:space=\"preserve\"></w:t></w:r></w:p>".into()
            } else {
                String::new()
            }
        }
        _ => return Err("Unknown paragraph operation".into()),
    };
    let mut result = s.to_string();
    result.replace_range(a..b, &replacement);
    Ok(result)
}
