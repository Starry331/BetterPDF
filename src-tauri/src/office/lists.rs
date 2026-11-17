//! Native Word list definitions; inserting after a list item continues its numbering.
use super::slides::{append_child, attribute};
use super::*;

pub(super) fn edit(
    mut z: ZipArchive<File>,
    output: &Path,
    document: &str,
    item: &Item,
    mode: &str,
) -> Result<(), String> {
    if !["bullet", "decimal", "none"].contains(&mode) {
        return Err("Choose bullets, a new numbered list, or no list".into());
    }
    if mode == "none" {
        let changed = paragraphs::change(
            document,
            item,
            "paragraph_numbering",
            &serde_json::json!({"numId":0}),
        )?;
        return write_package(
            z,
            output,
            [(item.part.clone(), changed)].into_iter().collect(),
        );
    }
    let names = z
        .file_names()
        .map(str::to_string)
        .collect::<std::collections::HashSet<_>>();
    let relpath = "word/_rels/document.xml.rels";
    let mut rels = if names.contains(relpath) {
        xml(&mut z, relpath)?
    } else {
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#.into()
    };
    let mut found = false;
    let mut ids = std::collections::HashSet::new();
    for (a, b, _) in spans(&rels, "Relationship")? {
        let f = &rels[a..b];
        ids.insert(attribute(f, "Id")?);
        if attribute(f, "Type")?.ends_with("/numbering") {
            if found
                || attribute(f, "TargetMode")? == "External"
                || !["numbering.xml", "/word/numbering.xml"]
                    .contains(&attribute(f, "Target")?.as_str())
            {
                return Err("Unsupported Word numbering relationship".into());
            }
            found = true;
        }
    }
    if !found {
        let rid = (1..=20001)
            .map(|n| format!("bpdfList{n}"))
            .find(|n| !ids.contains(n))
            .ok_or("Too many Word relationships")?;
        append_child(
            &mut rels,
            &format!(
                r#"<Relationship Id="{rid}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/>"#
            ),
        )?;
    }
    let mut numbering = if names.contains("word/numbering.xml") {
        xml(&mut z, "word/numbering.xml")?
    } else {
        r#"<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"></w:numbering>"#.into()
    };
    if !numbering.contains("<w:numbering") {
        return Err("Unsupported numbering namespace".into());
    }
    let next = |s: &str, tag: &str, key: &str| -> Result<u32, String> {
        spans(s, tag)?
            .iter()
            .map(|(a, b, _)| attribute(&s[*a..*b], key)?.parse::<u32>().map_err(err))
            .collect::<Result<Vec<_>, String>>()?
            .into_iter()
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("List identifier limit reached".into())
    };
    let abstract_id = next(&numbering, "abstractNum", "w:abstractNumId")?;
    let num_id = next(&numbering, "num", "w:numId")?;
    let mark = if mode == "bullet" { "•" } else { "%1." };
    let definition = format!(
        r#"<w:abstractNum w:abstractNumId="{abstract_id}"><w:multiLevelType w:val="singleLevel"/><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="{mode}"/><w:lvlText w:val="{mark}"/><w:lvlJc w:val="left"/><w:pPr><w:tabs><w:tab w:val="num" w:pos="720"/></w:tabs><w:ind w:left="720" w:hanging="360"/></w:pPr></w:lvl></w:abstractNum>"#
    );
    // Definitions precede concrete list instances in WordprocessingML.
    let before = ["num", "numIdMacAtCleanup"]
        .iter()
        .filter_map(|tag| spans(&numbering, tag).ok()?.first().map(|v| v.0))
        .min();
    if let Some(at) = before {
        numbering.insert_str(at, &definition);
    } else {
        append_child(&mut numbering, &definition)?;
    }
    let instance =
        format!(r#"<w:num w:numId="{num_id}"><w:abstractNumId w:val="{abstract_id}"/></w:num>"#);
    if let Some((a, _, _)) = spans(&numbering, "numIdMacAtCleanup")?.first() {
        numbering.insert_str(*a, &instance);
    } else {
        append_child(&mut numbering, &instance)?;
    }
    let mut types = xml(&mut z, "[Content_Types].xml")?;
    if !spans(&types, "Override")?.iter().any(|(a, b, _)| {
        attribute(&types[*a..*b], "PartName").ok().as_deref() == Some("/word/numbering.xml")
    }) {
        append_child(
            &mut types,
            r#"<Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/>"#,
        )?;
    }
    let changed = paragraphs::change(
        document,
        item,
        "paragraph_numbering",
        &serde_json::json!({"numId":num_id}),
    )?;
    write_package(
        z,
        output,
        [
            (item.part.clone(), changed),
            ("word/numbering.xml".into(), numbering),
            (relpath.into(), rels),
            ("[Content_Types].xml".into(), types),
        ]
        .into_iter()
        .collect(),
    )
}
