//! Equal-priority EPUB 3.3 exporter supporting both Fixed-Layout (`pre-paginated`)
//! for layout-heavy books and Reflowable EPUB 3 modes.

use book_core::BookDocument;
use book_layout::ComputedBookLayout;
use std::fs::File;
use std::io::{Cursor, Write};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EpubExportError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("ZIP archive error: {0}")]
    Zip(#[from] zip::result::ZipError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpubLayoutMode {
    /// Fixed-Layout EPUB 3 (`rendition:layout="pre-paginated"`), matching canvas coordinates 1:1.
    FixedLayout,
    /// Reflowable EPUB 3 ordered by threaded stories and page reading order.
    Reflowable,
}

pub fn export_epub_bytes(
    doc: &BookDocument,
    layout: &ComputedBookLayout,
    mode: EpubLayoutMode,
) -> Result<Vec<u8>, EpubExportError> {
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut cursor);
        let stored = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        let deflated = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        // EPUB 3 spec: mimetype MUST be the first entry and uncompressed (Stored)
        zip.start_file("mimetype", stored)?;
        zip.write_all(b"application/epub+zip")?;

        zip.start_file("META-INF/container.xml", deflated)?;
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#,
        )?;

        let is_fixed = mode == EpubLayoutMode::FixedLayout;
        let rendition_meta = if is_fixed {
            "<meta property=\"rendition:layout\">pre-paginated</meta>\n    <meta property=\"rendition:spread\">auto</meta>"
        } else {
            "<meta property=\"rendition:layout\">reflowable</meta>"
        };

        let mut manifest_items = String::from(
            "    <item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>\n",
        );
        let mut spine_items = String::new();
        let mut nav_links = String::new();

        for (idx, page) in doc.pages.iter().enumerate() {
            let page_num = idx + 1;
            let filename = format!("page_{page_num:03}.xhtml");
            manifest_items.push_str(&format!(
                "    <item id=\"page_{page_num:03}\" href=\"{filename}\" media-type=\"application/xhtml+xml\"/>\n"
            ));
            spine_items.push_str(&format!("    <itemref idref=\"page_{page_num:03}\"/>\n"));
            nav_links.push_str(&format!(
                "        <li><a href=\"{filename}\">{}</a></li>\n",
                escape_xml(&page.title)
            ));

            let page_xhtml = render_page_xhtml(doc, layout, idx, is_fixed);
            zip.start_file(format!("OEBPS/{filename}"), deflated)?;
            zip.write_all(page_xhtml.as_bytes())?;
        }

        let opf = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="bookid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="bookid">urn:uuid:{}</dc:identifier>
    <dc:title>{}</dc:title>
    <dc:creator>{}</dc:creator>
    <dc:language>{}</dc:language>
    <meta property="dcterms:modified">2026-11-14T00:00:00Z</meta>
    {rendition_meta}
  </metadata>
  <manifest>
{manifest_items}  </manifest>
  <spine>
{spine_items}  </spine>
</package>"#,
            if doc.metadata.isbn.is_empty() {
                "00000000-0000-0000-0000-000000000001"
            } else {
                &doc.metadata.isbn
            },
            escape_xml(&doc.metadata.title),
            escape_xml(&doc.metadata.author),
            escape_xml(&doc.metadata.language),
        );
        zip.start_file("OEBPS/content.opf", deflated)?;
        zip.write_all(opf.as_bytes())?;

        let nav = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<head><meta charset="utf-8"/><title>Table of Contents</title></head>
<body>
  <nav epub:type="toc" id="toc">
    <h1>{}</h1>
    <ol>
{nav_links}    </ol>
  </nav>
</body>
</html>"#,
            escape_xml(&doc.metadata.title)
        );
        zip.start_file("OEBPS/nav.xhtml", deflated)?;
        zip.write_all(nav.as_bytes())?;

        zip.finish()?;
    }
    Ok(cursor.into_inner())
}

pub fn export_epub_to_file(
    doc: &BookDocument,
    layout: &ComputedBookLayout,
    mode: EpubLayoutMode,
    path: impl AsRef<Path>,
) -> Result<(), EpubExportError> {
    let bytes = export_epub_bytes(doc, layout, mode)?;
    let mut file = File::create(path)?;
    file.write_all(&bytes)?;
    Ok(())
}

fn render_page_xhtml(
    doc: &BookDocument,
    layout: &ComputedBookLayout,
    page_idx: usize,
    fixed_layout: bool,
) -> String {
    let page = &doc.pages[page_idx];
    let w = doc.setup.width_pt.round() as i32;
    let h = doc.setup.height_pt.round() as i32;
    let bg = page.background_color;

    let viewport = if fixed_layout {
        format!("<meta name=\"viewport\" content=\"width={w}, height={h}\"/>")
    } else {
        "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\"/>".to_string()
    };

    let mut body = String::new();
    for frame in page.frames_sorted_by_z() {
        let r = frame.rect;
        match &frame.kind {
            book_core::FrameKind::Shape {
                fill,
                stroke_color,
                stroke_width_pt,
                corner_radius_pt,
                ..
            } => {
                if fixed_layout {
                    body.push_str(&format!(
                        "    <div style=\"position:absolute;left:{:.1}px;top:{:.1}px;width:{:.1}px;height:{:.1}px;background:rgba({},{},{},{:.2});border:{:.1}px solid rgb({},{},{});border-radius:{:.1}px;\"></div>\n",
                        r.x, r.y, r.width, r.height,
                        fill.r, fill.g, fill.b, fill.a as f32 / 255.0,
                        stroke_width_pt, stroke_color.r, stroke_color.g, stroke_color.b,
                        corner_radius_pt
                    ));
                }
            }
            book_core::FrameKind::Image { caption, .. } => {
                if fixed_layout {
                    body.push_str(&format!(
                        "    <div style=\"position:absolute;left:{:.1}px;top:{:.1}px;width:{:.1}px;height:{:.1}px;background:#eef2f8;border:1px solid #8ea0b8;display:flex;align-items:center;justify-content:center;font-family:sans-serif;font-size:12px;color:#334;\">{}</div>\n",
                        r.x, r.y, r.width, r.height, escape_xml(caption)
                    ));
                } else if !caption.is_empty() {
                    body.push_str(&format!(
                        "    <figure><figcaption>{}</figcaption></figure>\n",
                        escape_xml(caption)
                    ));
                }
            }
            book_core::FrameKind::Text { .. } => {
                if let Some(page_layout) = layout.pages.get(page_idx) {
                    if let Some(computed) = page_layout.text_frames.get(&frame.id) {
                        if fixed_layout {
                            for line in &computed.lines {
                                let weight = if line.bold { "bold" } else { "normal" };
                                let style = if line.italic { "italic" } else { "normal" };
                                body.push_str(&format!(
                                    "    <div style=\"position:absolute;left:{:.1}px;top:{:.1}px;width:{:.1}px;font-family:'{}',serif;font-size:{:.1}px;font-weight:{weight};font-style:{style};color:rgb({},{},{});white-space:pre;\">{}</div>\n",
                                    line.x_pt,
                                    line.y_pt,
                                    line.width_pt,
                                    escape_xml(&line.font_family),
                                    line.font_size_pt,
                                    line.color.r,
                                    line.color.g,
                                    line.color.b,
                                    escape_xml(&line.text)
                                ));
                            }
                        } else {
                            let joined: Vec<&str> =
                                computed.lines.iter().map(|l| l.text.as_str()).collect();
                            if !joined.is_empty() {
                                body.push_str(&format!(
                                    "    <p>{}</p>\n",
                                    escape_xml(&joined.join(" "))
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml">
<head>
  <meta charset="utf-8"/>
  <title>{}</title>
  {viewport}
</head>
<body style="margin:0;padding:0;width:{w}px;height:{h}px;position:relative;background:rgb({},{},{});overflow:hidden;">
{body}</body>
</html>"#,
        escape_xml(&page.title),
        bg.r,
        bg.g,
        bg.b
    )
}

fn escape_xml(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
