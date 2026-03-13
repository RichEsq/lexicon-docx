use std::io::{Cursor, Read, Write};

use zip::read::ZipArchive;
use zip::write::FileOptions;
use zip::ZipWriter;

use crate::error::{LexiconError, Result};

/// VML shapetype definition for WordArt text (type #136).
const VML_SHAPETYPE: &str = r##"<v:shapetype id="_x0000_t136" coordsize="21600,21600" o:spt="136" adj="10800" path="m@7,l@8,m@5,21600l@6,21600e"><v:formulas><v:f eqn="sum #0 0 10800"/><v:f eqn="prod #0 2 1"/><v:f eqn="sum 21600 0 @1"/><v:f eqn="sum 0 0 @2"/><v:f eqn="sum 21600 0 @3"/><v:f eqn="if @0 @3 0"/><v:f eqn="if @0 21600 @1"/><v:f eqn="if @0 0 @2"/><v:f eqn="if @0 @4 21600"/><v:f eqn="mid @5 @6"/><v:f eqn="mid @8 @5"/><v:f eqn="mid @7 @8"/><v:f eqn="mid @6 @7"/><v:f eqn="sum @6 0 @5"/></v:formulas><v:path textpathok="t" o:connecttype="custom" o:connectlocs="@9,0;@10,10800;@11,21600;@12,10800" o:connectangles="270,180,90,0"/><v:textpath on="t" fitshape="t"/><v:handles><v:h position="#0,bottomRight" xrange="6629,14971"/></v:handles><o:lock v:ext="edit" text="t" shapetype="t"/></v:shapetype>"##;

/// Build the VML watermark run for a header paragraph.
fn watermark_run(text: &str, shape_id: &str, spid: &str, z_index: i32) -> String {
    // Escape XML entities in text
    let escaped = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;");

    format!(
        r##"<w:r><w:rPr><w:noProof/></w:rPr><w:pict>{VML_SHAPETYPE}<v:shape id="{shape_id}" o:spid="{spid}" type="#_x0000_t136" alt="" style="position:absolute;margin-left:0;margin-top:0;width:442.55pt;height:193.6pt;rotation:315;z-index:{z_index};mso-wrap-edited:f;mso-position-horizontal:center;mso-position-horizontal-relative:margin;mso-position-vertical:center;mso-position-vertical-relative:margin" o:allowincell="f" fillcolor="silver" stroked="f"><v:textpath style="font-family:&quot;Calibri&quot;;font-size:1pt" string="{escaped}"/></v:shape></w:pict></w:r>"##
    )
}

/// Build a complete header XML document containing the watermark.
fn watermark_header_xml(text: &str, shape_id: &str, spid: &str, z_index: i32) -> String {
    let run = watermark_run(text, shape_id, spid, z_index);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:hdr xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:w10="urn:schemas-microsoft-com:office:word" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" xmlns:wpg="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:wp14="http://schemas.microsoft.com/office/word/2010/wordprocessingDrawing" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml" mc:Ignorable="w14 wp14"><w:p><w:pPr><w:pStyle w:val="Header"/></w:pPr>{run}</w:p></w:hdr>"#
    )
}

/// Inject the watermark VML into an existing header XML string.
/// Inserts the watermark run into the first paragraph found.
fn inject_into_existing_header(
    header_xml: &str,
    text: &str,
    shape_id: &str,
    spid: &str,
    z_index: i32,
) -> String {
    let run = watermark_run(text, shape_id, spid, z_index);

    // Find <w:p> or <w:p ...> and insert the run after the opening tag + any pPr
    // Strategy: insert before </w:p> (the closing tag of the first paragraph)
    if let Some(pos) = header_xml.find("</w:p>") {
        let mut result = String::with_capacity(header_xml.len() + run.len());
        result.push_str(&header_xml[..pos]);
        result.push_str(&run);
        result.push_str(&header_xml[pos..]);
        result
    } else {
        // No paragraph found — inject before closing </w:hdr>
        if let Some(pos) = header_xml.find("</w:hdr>") {
            let mut result = String::with_capacity(header_xml.len() + run.len() + 50);
            result.push_str(&header_xml[..pos]);
            result.push_str("<w:p><w:pPr><w:pStyle w:val=\"Header\"/></w:pPr>");
            result.push_str(&run);
            result.push_str("</w:p>");
            result.push_str(&header_xml[pos..]);
            result
        } else {
            // Self-closing header tag like <w:hdr ... />
            // Replace the self-closing with open + content + close
            if let Some(pos) = header_xml.rfind("/>") {
                let mut result = String::with_capacity(header_xml.len() + run.len() + 100);
                result.push_str(&header_xml[..pos]);
                result.push_str("><w:p><w:pPr><w:pStyle w:val=\"Header\"/></w:pPr>");
                result.push_str(&run);
                result.push_str("</w:p></w:hdr>");
                result
            } else {
                header_xml.to_string()
            }
        }
    }
}

/// Inject a "DRAFT" watermark into a .docx file when status is draft.
///
/// Post-processes the ZIP to add VML watermark shapes into header XML parts.
pub fn inject_watermark(docx_bytes: Vec<u8>, text: &str) -> Result<Vec<u8>> {
    let reader = Cursor::new(&docx_bytes);
    let mut archive = ZipArchive::new(reader)
        .map_err(|e| LexiconError::Render(format!("Failed to read docx ZIP: {}", e)))?;

    // Read all files from the archive into memory
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| LexiconError::Render(format!("Failed to read ZIP entry: {}", e)))?;
        let name = file.name().to_string();
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)
            .map_err(|e| LexiconError::Render(format!("Failed to read ZIP entry data: {}", e)))?;
        files.push((name, contents));
    }

    // Check which header files exist
    let has_header1 = files.iter().any(|(n, _)| n == "word/header1.xml");
    let has_header2 = files.iter().any(|(n, _)| n == "word/header2.xml");

    // We need to create header2.xml (default pages) if it doesn't exist
    let need_header2 = !has_header2;

    // Inject watermark into existing headers and prepare new ones
    let mut modified_files: Vec<(String, Vec<u8>)> = Vec::new();

    for (name, contents) in &files {
        match name.as_str() {
            "word/header1.xml" => {
                // First-page header — inject watermark
                let xml = String::from_utf8_lossy(contents);
                let modified = inject_into_existing_header(
                    &xml,
                    text,
                    "PowerPlusWaterMarkObject1",
                    "_x0000_s1025",
                    -251655168,
                );
                modified_files.push((name.clone(), modified.into_bytes()));
            }
            "word/document.xml" if need_header2 => {
                // Add headerReference for the new default header
                let xml = String::from_utf8_lossy(contents);
                let modified = add_header_reference(&xml, "rIdWatermarkHeader", "default");
                modified_files.push((name.clone(), modified.into_bytes()));
            }
            "word/_rels/document.xml.rels" if need_header2 => {
                // Add relationship for header2.xml
                let xml = String::from_utf8_lossy(contents);
                let modified = add_relationship(
                    &xml,
                    "rIdWatermarkHeader",
                    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header",
                    "header2.xml",
                );
                modified_files.push((name.clone(), modified.into_bytes()));
            }
            "[Content_Types].xml" if need_header2 => {
                // Ensure header content type override exists
                let xml = String::from_utf8_lossy(contents);
                let modified = ensure_header_content_type(&xml, "/word/header2.xml");
                modified_files.push((name.clone(), modified.into_bytes()));
            }
            _ => {
                modified_files.push((name.clone(), contents.clone()));
            }
        }
    }

    // If header1 didn't exist (shouldn't happen with lexicon output, but be safe)
    if !has_header1 {
        let xml = watermark_header_xml(
            text,
            "PowerPlusWaterMarkObject1",
            "_x0000_s1025",
            -251655168,
        );
        modified_files.push(("word/header1.xml".to_string(), xml.into_bytes()));
    }

    // Create header2.xml for default pages
    if need_header2 {
        let xml = watermark_header_xml(
            text,
            "PowerPlusWaterMarkObject2",
            "_x0000_s1026",
            -251651072,
        );
        modified_files.push(("word/header2.xml".to_string(), xml.into_bytes()));
    }

    // If header2 already existed, we need to inject into it
    if has_header2 {
        // Already handled in the loop above — but we didn't! Let's handle it.
        // Actually we need to re-check: if header2 existed, we passed it through unmodified.
        // Let's fix this by modifying in place.
        for (name, contents) in &mut modified_files {
            if name == "word/header2.xml" && has_header2 {
                let xml = String::from_utf8_lossy(contents);
                let modified = inject_into_existing_header(
                    &xml,
                    text,
                    "PowerPlusWaterMarkObject2",
                    "_x0000_s1026",
                    -251651072,
                );
                *contents = modified.into_bytes();
                break;
            }
        }
    }

    // Write the new ZIP
    let buf = Vec::new();
    let cursor = Cursor::new(buf);
    let mut writer = ZipWriter::new(cursor);
    let options: FileOptions<'_, ()> =
        FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    for (name, contents) in &modified_files {
        writer
            .start_file(name, options.clone())
            .map_err(|e| LexiconError::Render(format!("Failed to write ZIP entry: {}", e)))?;
        writer
            .write_all(contents)
            .map_err(|e| LexiconError::Render(format!("Failed to write ZIP data: {}", e)))?;
    }

    let cursor = writer
        .finish()
        .map_err(|e| LexiconError::Render(format!("Failed to finalize ZIP: {}", e)))?;

    Ok(cursor.into_inner())
}

/// Add a `<w:headerReference>` to the `<w:sectPr>` in document.xml.
fn add_header_reference(xml: &str, r_id: &str, header_type: &str) -> String {
    // Insert before the closing </w:sectPr> or before <w:titlePg/> if present
    let reference = format!(
        r#"<w:headerReference w:type="{header_type}" r:id="{r_id}"/>"#
    );

    // Try to insert before </w:sectPr>
    if let Some(pos) = xml.find("</w:sectPr>") {
        let mut result = String::with_capacity(xml.len() + reference.len());
        result.push_str(&xml[..pos]);
        result.push_str(&reference);
        result.push_str(&xml[pos..]);
        result
    } else {
        xml.to_string()
    }
}

/// Add a `<Relationship>` to document.xml.rels.
fn add_relationship(xml: &str, id: &str, rel_type: &str, target: &str) -> String {
    let relationship = format!(
        r#"<Relationship Id="{id}" Type="{rel_type}" Target="{target}"/>"#
    );

    if let Some(pos) = xml.find("</Relationships>") {
        let mut result = String::with_capacity(xml.len() + relationship.len());
        result.push_str(&xml[..pos]);
        result.push_str(&relationship);
        result.push_str(&xml[pos..]);
        result
    } else {
        xml.to_string()
    }
}

/// Ensure a content type Override exists for a header part.
fn ensure_header_content_type(xml: &str, part_name: &str) -> String {
    // Check if an override already exists for this part
    if xml.contains(part_name) {
        return xml.to_string();
    }

    let override_elem = format!(
        r#"<Override PartName="{part_name}" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/>"#
    );

    if let Some(pos) = xml.find("</Types>") {
        let mut result = String::with_capacity(xml.len() + override_elem.len());
        result.push_str(&xml[..pos]);
        result.push_str(&override_elem);
        result.push_str(&xml[pos..]);
        result
    } else {
        xml.to_string()
    }
}
