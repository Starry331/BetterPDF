//! Copy-on-write cell formatting: shared styles and other cells remain untouched.
use super::slides::{append_child, attribute, set_attribute};
use super::*;
use std::collections::HashMap;

fn dimensions(sheet: &mut String, address: &str, style: &serde_json::Value) -> Result<(), String> {
    if let Some(value) = style.get("rowHeight") {
        let height = value.as_f64().ok_or("Invalid row height")?;
        if !(4.0..=409.0).contains(&height) {
            return Err("Row height must be between 4 and 409 points".into());
        }
        let row = address.trim_start_matches(|c: char| c.is_ascii_alphabetic());
        let (a, b, _) = spans(sheet, "row")?
            .into_iter()
            .find(|(_, _, r)| r == row)
            .ok_or("Missing cell row")?;
        let updated = set_attribute(
            &set_attribute(&sheet[a..b], "ht", &height.to_string())?,
            "customHeight",
            "1",
        )?;
        sheet.replace_range(a..b, &updated);
    }
    if let Some(value) = style.get("columnWidth") {
        if !sheet.contains("<worksheet") {
            return Err("Unsupported worksheet namespace for column resizing".into());
        }
        let width = value.as_f64().ok_or("Invalid column width")?;
        if !(1.0..=255.0).contains(&width) {
            return Err("Column width must be between 1 and 255 characters".into());
        }
        let mut column = 0u32;
        for c in address.bytes().take_while(u8::is_ascii_alphabetic) {
            column = column
                .checked_mul(26)
                .and_then(|n| n.checked_add((c.to_ascii_uppercase() - b'A' + 1) as u32))
                .ok_or("Invalid column address")?;
        }
        if column == 0 || column > 16384 {
            return Err("Invalid column address".into());
        }
        let existing = spans(sheet, "cols")?.into_iter().next();
        let mut cols = existing
            .as_ref()
            .map(|(a, b, _)| sheet[*a..*b].to_string())
            .unwrap_or_else(|| "<cols></cols>".into());
        let mut matches = Vec::new();
        for (a, b, _) in spans(&cols, "col")? {
            let min = attribute(&cols[a..b], "min")?.parse::<u32>().map_err(err)?;
            let max = attribute(&cols[a..b], "max")?.parse::<u32>().map_err(err)?;
            if min <= column && column <= max {
                matches.push((a, b, min, max));
            }
        }
        if matches.len() > 1 {
            return Err("Overlapping column definitions cannot be resized safely".into());
        }
        if let Some((a, b, min, max)) = matches.first().copied() {
            let original = &cols[a..b];
            let mut replacement = String::new();
            if min < column {
                replacement.push_str(&set_attribute(original, "max", &(column - 1).to_string())?);
            }
            let mut selected = set_attribute(
                &set_attribute(original, "min", &column.to_string())?,
                "max",
                &column.to_string(),
            )?;
            selected = set_attribute(
                &set_attribute(
                    &set_attribute(&selected, "width", &width.to_string())?,
                    "customWidth",
                    "1",
                )?,
                "bestFit",
                "0",
            )?;
            replacement.push_str(&selected);
            if column < max {
                replacement.push_str(&set_attribute(original, "min", &(column + 1).to_string())?);
            }
            cols.replace_range(a..b, &replacement);
        } else {
            let value = format!(
                "<col min=\"{column}\" max=\"{column}\" width=\"{width}\" customWidth=\"1\"/>"
            );
            let next = spans(&cols, "col")?.into_iter().find(|(a, b, _)| {
                attribute(&cols[*a..*b], "min")
                    .ok()
                    .and_then(|v| v.parse::<u32>().ok())
                    .is_some_and(|n| n > column)
            });
            if let Some((a, _, _)) = next {
                cols.insert_str(a, &value);
            } else {
                append_child(&mut cols, &value)?;
            }
        }
        if let Some((a, b, _)) = existing {
            sheet.replace_range(a..b, &cols);
        } else {
            let at = spans(sheet, "sheetData")?
                .first()
                .ok_or("Missing worksheet data")?
                .0;
            sheet.insert_str(at, &cols);
        }
    }
    Ok(())
}

fn replace_child(xml: &mut String, tag: &str, replacement: &str) -> Result<(), String> {
    if let Some((a, b, _)) = spans(xml, tag)?.first() {
        xml.replace_range(*a..*b, replacement);
    } else {
        append_child(xml, replacement)?;
    }
    Ok(())
}
fn entry(styles: &str, container: &str, tag: &str, index: usize) -> Result<String, String> {
    let (a, b, _) = spans(styles, container)?
        .into_iter()
        .next()
        .ok_or("Missing spreadsheet style collection")?;
    let inside = &styles[a..b];
    let entries = spans(inside, tag)?;
    let (x, y, _) = entries
        .get(index)
        .ok_or("Invalid spreadsheet style reference")?;
    Ok(inside[*x..*y].into())
}
fn append_entry(
    styles: &mut String,
    container: &str,
    tag: &str,
    value: &str,
) -> Result<usize, String> {
    let (a, b, _) = spans(styles, container)?
        .into_iter()
        .next()
        .ok_or("Missing spreadsheet style collection")?;
    let mut inside = styles[a..b].to_string();
    let entries = spans(&inside, tag)?;
    if let Some(index) = entries
        .iter()
        .position(|(x, y, _)| &inside[*x..*y] == value)
    {
        return Ok(index);
    }
    let index = entries.len();
    if index >= 64000 {
        return Err("Spreadsheet style limit reached".into());
    }
    append_child(&mut inside, value)?;
    inside = set_attribute(&inside, "count", &(index + 1).to_string())?;
    styles.replace_range(a..b, &inside);
    Ok(index)
}
fn rgb(value: &str) -> Result<String, String> {
    let value = value.trim_start_matches('#');
    if value.len() != 6 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Color must be a six-digit RGB value".into());
    }
    Ok(format!("FF{}", value.to_ascii_uppercase()))
}

pub(super) fn apply(
    z: &mut ZipArchive<File>,
    changes: &mut HashMap<String, String>,
    part: &str,
    address: &str,
    style: &serde_json::Value,
) -> Result<(), String> {
    if ![
        "bold",
        "italic",
        "underline",
        "size",
        "font",
        "color",
        "fill",
        "numberFormat",
        "horizontal",
        "wrap",
    ]
    .iter()
    .any(|key| style.get(*key).is_some())
    {
        return dimensions(
            changes.get_mut(part).ok_or("Missing worksheet")?,
            address,
            style,
        );
    }
    let sheet = changes.get(part).ok_or("Missing worksheet")?;
    let (ca, cb, _) = spans(sheet, "c")?
        .into_iter()
        .find(|(_, _, r)| r == address)
        .ok_or("Cell no longer exists")?;
    let mut cell = sheet[ca..cb].to_string();
    let names = z
        .file_names()
        .map(str::to_string)
        .collect::<std::collections::HashSet<_>>();
    let mut styles = if names.contains("xl/styles.xml") {
        xml(z, "xl/styles.xml")?
    } else {
        r#"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="1"><font><sz val="11"/><name val="Liberation Sans"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border/></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>"#.into()
    };
    if !styles.contains("<styleSheet") {
        return Err("Unsupported spreadsheet style namespace".into());
    }
    let old = attribute(&cell, "s")?;
    let index = if old.is_empty() {
        0
    } else {
        old.parse::<usize>().map_err(err)?
    };
    let mut xf = entry(&styles, "cellXfs", "xf", index)?;
    if ["bold", "italic", "underline", "size", "font", "color"]
        .iter()
        .any(|key| style.get(*key).is_some())
    {
        let id = attribute(&xf, "fontId")?;
        let index = if id.is_empty() {
            0
        } else {
            id.parse::<usize>().map_err(err)?
        };
        let mut font = entry(&styles, "fonts", "font", index)?;
        for (key, tag) in [("bold", "b"), ("italic", "i"), ("underline", "u")] {
            if let Some(value) = style.get(key) {
                let enabled = value.as_bool().ok_or("Invalid font option")?;
                let val = if key == "underline" {
                    if enabled {
                        "single"
                    } else {
                        "none"
                    }
                } else {
                    if enabled {
                        "1"
                    } else {
                        "0"
                    }
                };
                replace_child(&mut font, tag, &format!("<{tag} val=\"{val}\"/>"))?;
            }
        }
        if let Some(value) = style.get("size") {
            let size = value.as_f64().ok_or("Invalid font size")?;
            if !(4.0..=200.0).contains(&size) {
                return Err("Font size must be between 4 and 200".into());
            }
            replace_child(&mut font, "sz", &format!("<sz val=\"{size}\"/>"))?;
        }
        if let Some(value) = style.get("font") {
            let name = value.as_str().ok_or("Invalid font family")?.trim();
            if name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
                return Err("Invalid font family".into());
            }
            replace_child(
                &mut font,
                "name",
                &format!("<name val=\"{}\"/>", escaped(name)),
            )?;
            for (a, b, _) in spans(&font, "scheme")?.into_iter().rev() {
                font.replace_range(a..b, "");
            }
        }
        if let Some(value) = style.get("color") {
            replace_child(
                &mut font,
                "color",
                &format!(
                    "<color rgb=\"{}\"/>",
                    rgb(value.as_str().ok_or("Invalid font color")?)?
                ),
            )?;
        }
        let id = append_entry(&mut styles, "fonts", "font", &font)?;
        xf = set_attribute(&xf, "fontId", &id.to_string())?;
        xf = set_attribute(&xf, "applyFont", "1")?;
    }
    if let Some(value) = style.get("fill") {
        let fill=format!("<fill><patternFill patternType=\"solid\"><fgColor rgb=\"{}\"/><bgColor indexed=\"64\"/></patternFill></fill>",rgb(value.as_str().ok_or("Invalid cell fill")?)?);
        let id = append_entry(&mut styles, "fills", "fill", &fill)?;
        xf = set_attribute(&xf, "fillId", &id.to_string())?;
        xf = set_attribute(&xf, "applyFill", "1")?;
    }
    if let Some(value) = style.get("numberFormat") {
        let id = match value.as_str() {
            Some("general") => 0,
            Some("decimal") => 2,
            Some("thousands") => 4,
            Some("percent") => 10,
            Some("date") => 14,
            _ => return Err("Unsupported number format".into()),
        };
        xf = set_attribute(&xf, "numFmtId", &id.to_string())?;
        xf = set_attribute(&xf, "applyNumberFormat", "1")?;
    }
    if style.get("horizontal").is_some() || style.get("wrap").is_some() {
        let mut alignment = spans(&xf, "alignment")?
            .first()
            .map(|(a, b, _)| xf[*a..*b].to_string())
            .unwrap_or_else(|| "<alignment/>".into());
        if let Some(value) = style.get("horizontal") {
            let value = value.as_str().ok_or("Invalid alignment")?;
            if !["general", "left", "center", "right", "justify"].contains(&value) {
                return Err("Invalid alignment".into());
            }
            alignment = set_attribute(&alignment, "horizontal", value)?;
        }
        if let Some(value) = style.get("wrap") {
            alignment = set_attribute(
                &alignment,
                "wrapText",
                if value.as_bool().ok_or("Invalid wrap option")? {
                    "1"
                } else {
                    "0"
                },
            )?;
        }
        if spans(&xf, "alignment")?.is_empty() {
            let at = ["protection", "extLst"]
                .iter()
                .filter_map(|tag| spans(&xf, tag).ok()?.first().map(|v| v.0))
                .min();
            if let Some(at) = at {
                xf.insert_str(at, &alignment);
            } else {
                append_child(&mut xf, &alignment)?;
            }
        } else {
            replace_child(&mut xf, "alignment", &alignment)?;
        }
        xf = set_attribute(&xf, "applyAlignment", "1")?;
    }
    let index = append_entry(&mut styles, "cellXfs", "xf", &xf)?;
    cell = set_attribute(&cell, "s", &index.to_string())?;
    changes.get_mut(part).unwrap().replace_range(ca..cb, &cell);
    let mut rels = xml(z, "xl/_rels/workbook.xml.rels")?;
    let mut found = false;
    let mut ids = std::collections::HashSet::new();
    for (a, b, _) in spans(&rels, "Relationship")? {
        let f = &rels[a..b];
        ids.insert(attribute(f, "Id")?);
        if attribute(f, "Type")?.ends_with("/styles") {
            if found
                || attribute(f, "TargetMode")? == "External"
                || !["styles.xml", "/xl/styles.xml"].contains(&attribute(f, "Target")?.as_str())
            {
                return Err("Unsupported spreadsheet style relationship".into());
            }
            found = true;
        }
    }
    if !found {
        let id = (1..=20001)
            .map(|n| format!("bpdfStyle{n}"))
            .find(|id| !ids.contains(id))
            .ok_or("Too many style relationships")?;
        append_child(&mut rels,&format!("<Relationship Id=\"{id}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/>"))?;
    }
    let mut types = xml(z, "[Content_Types].xml")?;
    if !spans(&types, "Override")?.iter().any(|(a, b, _)| {
        attribute(&types[*a..*b], "PartName").ok().as_deref() == Some("/xl/styles.xml")
    }) {
        append_child(&mut types,"<Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/>")?;
    }
    dimensions(changes.get_mut(part).unwrap(), address, style)?;
    changes.insert("xl/styles.xml".into(), styles);
    changes.insert("xl/_rels/workbook.xml.rels".into(), rels);
    changes.insert("[Content_Types].xml".into(), types);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resizing_splits_shared_column_ranges_without_changing_neighbors() {
        let mut sheet=r#"<worksheet><cols><col min="1" max="3" width="8" style="5"/></cols><sheetData><row r="1"><c r="B1"><v>9</v></c></row><row r="2"><c r="B2"><v>8</v></c></row></sheetData></worksheet>"#.to_string();
        dimensions(
            &mut sheet,
            "B1",
            &serde_json::json!({"columnWidth":24,"rowHeight":30}),
        )
        .unwrap();
        let columns = spans(&sheet, "col").unwrap();
        assert_eq!(columns.len(), 3);
        let widths = columns
            .iter()
            .map(|(a, b, _)| attribute(&sheet[*a..*b], "width").unwrap())
            .collect::<Vec<_>>();
        assert_eq!(widths, vec!["8", "24", "8"]);
        for (a, b, _) in &columns {
            assert_eq!(attribute(&sheet[*a..*b], "style").unwrap(), "5");
        }
        assert!(sheet.contains(r#"<row r="1" ht="30" customHeight="1">"#));
        assert!(sheet.contains(r#"<row r="2"><c r="B2"><v>8</v></c></row>"#));
        assert!(dimensions(&mut sheet, "B1", &serde_json::json!({"columnWidth":1000})).is_err());
    }
}
