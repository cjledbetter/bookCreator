//! Equal-priority EPUB 3.3 exporter supporting both Fixed-Layout (`pre-paginated`)
//! for layout-heavy illustrated books and Reflowable EPUB 3 modes.
//!
//! Produces standards-compliant EPUB 3.3 archives where `mimetype` is the first
//! uncompressed entry, followed by `META-INF/container.xml`, `OEBPS/content.opf`,
//! `OEBPS/nav.xhtml`, `OEBPS/styles.css`, per-page XHTML documents, and embedded
//! image and font assets under `OEBPS/assets/...`.

use book_core::{
    AssetEntry, AssetKind, BookDocument, ColorRgba, Frame, FrameKind, ImageFit, MasterPage,
    ShapeKind, TextAlignment,
};
use book_layout::{ComputedBookLayout, ComputedPageLayout, PositionedTextLine};
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

        // 1. EPUB 3.3 specification: `mimetype` MUST be the very first ZIP entry and Stored uncompressed.
        zip.start_file("mimetype", stored)?;
        zip.write_all(b"application/epub+zip")?;

        // 2. OCF container descriptor
        zip.start_file("META-INF/container.xml", deflated)?;
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#,
        )?;

        // 3. Shared stylesheet (`OEBPS/styles.css`) including `@font-face` rules for embedded fonts
        let css = build_stylesheet(doc);
        zip.start_file("OEBPS/styles.css", deflated)?;
        zip.write_all(css.as_bytes())?;

        let is_fixed = mode == EpubLayoutMode::FixedLayout;
        let rendition_meta = if is_fixed {
            "    <meta property=\"rendition:layout\">pre-paginated</meta>\n    <meta property=\"rendition:orientation\">auto</meta>\n    <meta property=\"rendition:spread\">auto</meta>"
        } else {
            "    <meta property=\"rendition:layout\">reflowable</meta>"
        };

        let mut manifest_items = String::from(
            "    <item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>\n    <item id=\"styles\" href=\"styles.css\" media-type=\"text/css\"/>\n",
        );
        let mut spine_items = String::new();
        let mut nav_links = String::new();

        // 4. Write embedded image & font assets into `OEBPS/assets/...` and declare in `<manifest>`
        for (asset_idx, (asset_id, entry)) in doc.assets.iter().enumerate() {
            let Ok(safe_rel_buf) = book_core::validate_safe_relative_bundle_path(&entry.relative_path) else {
                continue;
            };
            let rel_path = normalize_portable_rel_path(&safe_rel_buf.to_string_lossy());
            let manifest_id = format!("asset_{}_{}", asset_idx + 1, sanitize_xml_id(asset_id));
            let media_type = if entry.mime_type.trim().is_empty() {
                infer_mime_type(&rel_path)
            } else {
                entry.mime_type.as_str()
            };

            manifest_items.push_str(&format!(
                "    <item id=\"{}\" href=\"{}\" media-type=\"{}\"/>\n",
                escape_xml(&manifest_id),
                escape_xml(&rel_path),
                escape_xml(media_type),
            ));

            let bytes = doc
                .asset_bytes
                .get(asset_id)
                .map(|v| v.as_slice())
                .unwrap_or(&[]);
            zip.start_file(format!("OEBPS/{rel_path}"), deflated)?;
            zip.write_all(bytes)?;
        }

        // 5. Render each page as XHTML (either Fixed-Layout or Reflowable)
        for (idx, page) in doc.pages.iter().enumerate() {
            let page_num = idx + 1;
            let filename = format!("page_{page_num:03}.xhtml");
            let item_props = if is_fixed && page_contains_svg(doc, idx) {
                " properties=\"svg\""
            } else {
                ""
            };
            manifest_items.push_str(&format!(
                "    <item id=\"page_{page_num:03}\" href=\"{filename}\" media-type=\"application/xhtml+xml\"{item_props}/>\n"
            ));
            spine_items.push_str(&format!("    <itemref idref=\"page_{page_num:03}\"/>\n"));
            let nav_title = if page.title.trim().is_empty() {
                format!("Page {page_num}")
            } else {
                page.title.clone()
            };
            nav_links.push_str(&format!(
                "        <li><a href=\"{filename}\">{}</a></li>\n",
                escape_xml(&nav_title)
            ));

            let page_xhtml = if is_fixed {
                render_fixed_page_xhtml(doc, layout, idx)
            } else {
                render_reflowable_page_xhtml(doc, layout, idx)
            };
            zip.start_file(format!("OEBPS/{filename}"), deflated)?;
            zip.write_all(page_xhtml.as_bytes())?;
        }

        // 6. Write `OEBPS/content.opf`
        let subtitle_meta = if doc.metadata.subtitle.trim().is_empty() {
            String::new()
        } else {
            format!(
                "\n    <dc:description>{}</dc:description>",
                escape_xml(&doc.metadata.subtitle)
            )
        };
        let publisher_meta = if doc.metadata.publisher.trim().is_empty() {
            String::new()
        } else {
            format!(
                "\n    <dc:publisher>{}</dc:publisher>",
                escape_xml(&doc.metadata.publisher)
            )
        };
        let language = if doc.metadata.language.trim().is_empty() {
            "en"
        } else {
            doc.metadata.language.trim()
        };
        let identifier = if doc.metadata.isbn.trim().is_empty() {
            "urn:uuid:00000000-0000-0000-0000-000000000001".to_string()
        } else {
            escape_xml(doc.metadata.isbn.trim())
        };

        let opf = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="bookid" xml:lang="{lang}">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="bookid">{identifier}</dc:identifier>
    <dc:title>{title}</dc:title>{subtitle_meta}
    <dc:creator>{author}</dc:creator>{publisher_meta}
    <dc:language>{lang}</dc:language>
    <meta property="dcterms:modified">2026-11-14T00:00:00Z</meta>
{rendition_meta}
  </metadata>
  <manifest>
{manifest_items}  </manifest>
  <spine>
{spine_items}  </spine>
</package>"#,
            lang = escape_xml(language),
            identifier = identifier,
            title = escape_xml(&doc.metadata.title),
            subtitle_meta = subtitle_meta,
            author = escape_xml(&doc.metadata.author),
            publisher_meta = publisher_meta,
            rendition_meta = rendition_meta,
            manifest_items = manifest_items,
            spine_items = spine_items,
        );
        zip.start_file("OEBPS/content.opf", deflated)?;
        zip.write_all(opf.as_bytes())?;

        // 7. Write `OEBPS/nav.xhtml`
        let nav = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/1999/xhtml" xml:lang="{lang}">
<head>
  <meta charset="utf-8"/>
  <title>{title} — Table of Contents</title>
  <link rel="stylesheet" type="text/css" href="styles.css"/>
</head>
<body class="nav-body">
  <nav xmlns:epub="http://www.idpf.org/2007/ops" epub:type="toc" id="toc">
    <h1>{title}</h1>
    <ol>
{nav_links}    </ol>
  </nav>
</body>
</html>"#,
            lang = escape_xml(language),
            title = escape_xml(&doc.metadata.title),
            nav_links = nav_links,
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

fn build_stylesheet(doc: &BookDocument) -> String {
    let mut css = String::from(
        r#"/* Generated by Book Creator EPUB 3.3 Exporter */
* {
  box-sizing: border-box;
}
body.fixed-page {
  margin: 0;
  padding: 0;
  position: relative;
  overflow: hidden;
  -webkit-text-size-adjust: none;
}
.fixed-frame {
  position: absolute;
}
.fixed-text-line {
  position: absolute;
  white-space: pre;
  line-height: 1.0;
  margin: 0;
  padding: 0;
}
.master-header {
  position: absolute;
  font-family: "Literata", "Georgia", serif;
  font-size: 8.5px;
  color: #5c6470;
  text-align: center;
  letter-spacing: 0.04em;
}
.master-folio {
  position: absolute;
  font-family: "Literata", "Georgia", serif;
  font-size: 9px;
  color: #4f5661;
  text-align: center;
}
body.reflowable-page {
  margin: 0 auto;
  padding: 2.2rem 1.6rem;
  max-width: 42rem;
  font-family: "Literata", "Georgia", "Times New Roman", serif;
  line-height: 1.55;
  color: #14161c;
}
article.chapter {
  margin: 0;
}
h2.chapter-title {
  font-size: 1.35rem;
  margin: 0 0 1.1rem 0;
  font-weight: 700;
}
p.story-para {
  margin: 0 0 0.85rem 0;
}
.align-left { text-align: left; }
.align-center { text-align: center; }
.align-right { text-align: right; }
.align-justify { text-align: justify; }
figure.epub-figure {
  margin: 1.25rem 0;
  text-align: center;
}
figure.epub-figure img {
  max-width: 100%;
  height: auto;
}
figure.epub-figure figcaption {
  margin-top: 0.45rem;
  font-size: 0.88rem;
  color: #4a5260;
  font-style: italic;
}
hr.divider {
  border: none;
  border-top: 1px solid #9aa4b4;
  margin: 1.4rem 0;
}
"#,
    );

    // Emit `@font-face` declarations for any embedded font assets
    for entry in doc.assets.values() {
        if entry.kind == AssetKind::Font {
            let Ok(safe_rel_buf) =
                book_core::validate_safe_relative_bundle_path(&entry.relative_path)
            else {
                continue;
            };
            let rel_path = normalize_portable_rel_path(&safe_rel_buf.to_string_lossy());
            let family_name = Path::new(&entry.original_filename)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&entry.id);
            css.push_str(&format!(
                "\n@font-face {{\n  font-family: \"{}\";\n  src: url(\"{}\");\n}}\n",
                sanitize_css_identifier(family_name),
                escape_css_string(&rel_path)
            ));
        }
    }

    css
}

fn page_contains_svg(doc: &BookDocument, page_idx: usize) -> bool {
    let Some(page) = doc.pages.get(page_idx) else {
        return false;
    };
    let master_has_shape = page
        .master_id
        .and_then(|mid| doc.master_pages.iter().find(|m| m.id == mid))
        .map(|m| m.frames.iter().any(|f| matches!(f.kind, FrameKind::Shape { .. })))
        .unwrap_or(false);
    master_has_shape
        || page
            .frames
            .iter()
            .any(|f| matches!(f.kind, FrameKind::Shape { .. }))
}

fn render_fixed_page_xhtml(
    doc: &BookDocument,
    layout: &ComputedBookLayout,
    page_idx: usize,
) -> String {
    let page = &doc.pages[page_idx];
    let w = doc.setup.width_pt.round().max(1.0) as i32;
    let h = doc.setup.height_pt.round().max(1.0) as i32;
    let bg = page.background_color;
    let page_layout = layout.pages.get(page_idx);

    let master: Option<&MasterPage> = page
        .master_id
        .and_then(|mid| doc.master_pages.iter().find(|m| m.id == mid));

    let mut body = String::new();

    // 1. Render MasterPage frames underneath page frames
    if let Some(master_page) = master {
        let mut master_frames: Vec<&Frame> = master_page.frames.iter().collect();
        master_frames.sort_by_key(|f| f.z_index);
        for frame in master_frames {
            render_fixed_frame(&mut body, doc, page_layout, frame);
        }
    }

    // 2. Render page frames in z-index order
    for frame in page.frames_sorted_by_z() {
        render_fixed_frame(&mut body, doc, page_layout, frame);
    }

    // 3. Render MasterPage running header and page number folio
    if let Some(master_page) = master {
        let safe = doc.setup.safe_margin_rect(page_idx);
        if !master_page.header_text.trim().is_empty() {
            let header_top = (doc.setup.margins.top * 0.45).max(10.0);
            body.push_str(&format!(
                "  <div class=\"master-header\" style=\"left:{:.1}px;top:{:.1}px;width:{:.1}px;\">{}</div>\n",
                safe.x,
                header_top,
                safe.width,
                escape_xml(&master_page.header_text)
            ));
        }
        if master_page.show_page_numbers {
            let footer_top = (doc.setup.height_pt - doc.setup.margins.bottom * 0.65).max(10.0);
            body.push_str(&format!(
                "  <div class=\"master-folio\" style=\"left:0px;top:{:.1}px;width:{w}px;\">{}</div>\n",
                footer_top,
                page_idx + 1
            ));
        }
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<head>
  <meta charset="utf-8"/>
  <title>{}</title>
  <meta name="viewport" content="width={w}, height={h}"/>
  <link rel="stylesheet" type="text/css" href="styles.css"/>
</head>
<body class="fixed-page" style="margin:0;padding:0;width:{w}px;height:{h}px;position:relative;background:rgb({},{},{});overflow:hidden;">
{body}</body>
</html>"#,
        escape_xml(&page.title),
        bg.r,
        bg.g,
        bg.b
    )
}

fn render_fixed_frame(
    body: &mut String,
    doc: &BookDocument,
    page_layout: Option<&ComputedPageLayout>,
    frame: &Frame,
) {
    let r = frame.rect;
    let rot_style = if frame.rotation_deg.abs() > 0.01 {
        format!("transform:rotate({:.1}deg);", frame.rotation_deg)
    } else {
        String::new()
    };

    match &frame.kind {
        FrameKind::Shape {
            kind,
            fill,
            stroke_color,
            stroke_width_pt,
            corner_radius_pt,
        } => {
            render_fixed_shape(
                body,
                *kind,
                r,
                *fill,
                *stroke_color,
                *stroke_width_pt,
                *corner_radius_pt,
                &rot_style,
            );
        }
        FrameKind::Image {
            asset_id,
            fit,
            caption,
            corner_radius_pt,
        } => {
            render_fixed_image(
                body,
                doc,
                r,
                asset_id.as_deref(),
                *fit,
                caption,
                *corner_radius_pt,
                &rot_style,
            );
        }
        FrameKind::Text { background, .. } => {
            if let Some(bg_col) = background {
                if bg_col.a > 0 {
                    body.push_str(&format!(
                        "  <div class=\"fixed-frame\" style=\"left:{:.1}px;top:{:.1}px;width:{:.1}px;height:{:.1}px;background:rgba({},{},{},{:.2});{rot_style}\"></div>\n",
                        r.x,
                        r.y,
                        r.width,
                        r.height,
                        bg_col.r,
                        bg_col.g,
                        bg_col.b,
                        bg_col.a as f32 / 255.0,
                    ));
                }
            }
            if let Some(computed) = page_layout.and_then(|pl| pl.text_frames.get(&frame.id)) {
                for (line_idx, line) in computed.lines.iter().enumerate() {
                    let is_last = line_idx + 1 == computed.lines.len();
                    render_fixed_text_line(body, line, is_last);
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_fixed_shape(
    body: &mut String,
    kind: ShapeKind,
    r: book_core::Rect,
    fill: ColorRgba,
    stroke_color: ColorRgba,
    stroke_width_pt: f32,
    corner_radius_pt: f32,
    rot_style: &str,
) {
    let fill_css = format!(
        "rgba({},{},{},{:.2})",
        fill.r,
        fill.g,
        fill.b,
        fill.a as f32 / 255.0
    );
    let stroke_css = format!(
        "rgba({},{},{},{:.2})",
        stroke_color.r,
        stroke_color.g,
        stroke_color.b,
        stroke_color.a as f32 / 255.0
    );
    let sw = stroke_width_pt.max(0.0);

    match kind {
        ShapeKind::Rectangle | ShapeKind::RoundedRectangle => {
            let radius = if kind == ShapeKind::RoundedRectangle {
                corner_radius_pt.max(0.0)
            } else {
                0.0
            };
            body.push_str(&format!(
                "  <div class=\"fixed-frame shape-rect\" style=\"left:{:.1}px;top:{:.1}px;width:{:.1}px;height:{:.1}px;background:{fill_css};border:{:.1}px solid rgb({},{},{});border-radius:{:.1}px;{rot_style}\"></div>\n",
                r.x, r.y, r.width, r.height, sw, stroke_color.r, stroke_color.g, stroke_color.b, radius
            ));
        }
        ShapeKind::Ellipse => {
            let cx = r.width * 0.5;
            let cy = r.height * 0.5;
            let rx = (r.width * 0.5 - sw * 0.5).max(0.5);
            let ry = (r.height * 0.5 - sw * 0.5).max(0.5);
            body.push_str(&format!(
                "  <svg xmlns=\"http://www.w3.org/2000/svg\" class=\"fixed-frame shape-ellipse\" style=\"left:{:.1}px;top:{:.1}px;width:{:.1}px;height:{:.1}px;border-radius:50%;{rot_style}\" viewBox=\"0 0 {:.1} {:.1}\"><ellipse cx=\"{:.1}\" cy=\"{:.1}\" rx=\"{:.1}\" ry=\"{:.1}\" fill=\"{fill_css}\" stroke=\"{stroke_css}\" stroke-width=\"{:.1}\"/></svg>\n",
                r.x, r.y, r.width, r.height, r.width.max(1.0), r.height.max(1.0), cx, cy, rx, ry, sw
            ));
        }
        ShapeKind::DividerLine => {
            let line_col = if stroke_color.a > 0 {
                stroke_css
            } else {
                fill_css
            };
            let line_w = sw.max(1.0);
            let mid_y = (r.height * 0.5).max(0.5);
            body.push_str(&format!(
                "  <svg xmlns=\"http://www.w3.org/2000/svg\" class=\"fixed-frame shape-divider\" style=\"left:{:.1}px;top:{:.1}px;width:{:.1}px;height:{:.1}px;{rot_style}\" viewBox=\"0 0 {:.1} {:.1}\"><line x1=\"0\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"{line_col}\" stroke-width=\"{:.1}\"/></svg>\n",
                r.x, r.y, r.width, r.height.max(2.0), r.width.max(1.0), r.height.max(2.0), mid_y, r.width.max(1.0), mid_y, line_w
            ));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_fixed_image(
    body: &mut String,
    doc: &BookDocument,
    r: book_core::Rect,
    asset_id: Option<&str>,
    fit: ImageFit,
    caption: &str,
    corner_radius_pt: f32,
    rot_style: &str,
) {
    let object_fit = match fit {
        ImageFit::Cover => "cover",
        ImageFit::Contain => "contain",
        ImageFit::Stretch => "fill",
    };

    let resolved_entry: Option<(&AssetEntry, String)> = asset_id
        .and_then(|id| doc.assets.get(id))
        .and_then(|entry| {
            book_core::validate_safe_relative_bundle_path(&entry.relative_path)
                .ok()
                .map(|p| (entry, normalize_portable_rel_path(&p.to_string_lossy())))
        });

    if let Some((entry, rel_path)) = resolved_entry {
        let alt_text = if caption.trim().is_empty() {
            &entry.original_filename
        } else {
            caption.trim()
        };
        body.push_str(&format!(
            "  <div class=\"fixed-frame image-frame\" style=\"left:{:.1}px;top:{:.1}px;width:{:.1}px;height:{:.1}px;border-radius:{:.1}px;overflow:hidden;{rot_style}\"><img src=\"{}\" alt=\"{}\" style=\"width:100%;height:100%;object-fit:{object_fit};display:block;\"/></div>\n",
            r.x,
            r.y,
            r.width,
            r.height,
            corner_radius_pt.max(0.0),
            escape_xml(&rel_path),
            escape_xml(alt_text),
        ));
    } else {
        body.push_str(&format!(
            "  <div class=\"fixed-frame image-placeholder\" style=\"left:{:.1}px;top:{:.1}px;width:{:.1}px;height:{:.1}px;background:#eef2f8;border:1px solid #8ea0b8;border-radius:{:.1}px;display:flex;align-items:center;justify-content:center;font-family:sans-serif;font-size:12px;color:#334;{rot_style}\">{}</div>\n",
            r.x,
            r.y,
            r.width,
            r.height,
            corner_radius_pt.max(0.0),
            escape_xml(caption)
        ));
    }
}

fn render_fixed_text_line(body: &mut String, line: &PositionedTextLine, is_last: bool) {
    let weight = if line.bold { "bold" } else { "normal" };
    let style = if line.italic { "italic" } else { "normal" };
    let align_css = match line.alignment {
        TextAlignment::Left => "text-align:left;",
        TextAlignment::Center => "text-align:center;",
        TextAlignment::Right => "text-align:right;",
        TextAlignment::Justified => {
            if is_last {
                "text-align:left;"
            } else {
                "text-align:justify;text-align-last:justify;"
            }
        }
    };
    let safe_font = sanitize_css_identifier(&line.font_family);

    body.push_str(&format!(
        "  <div class=\"fixed-text-line\" style=\"position:absolute;left:{:.1}px;top:{:.1}px;width:{:.1}px;font-family:'{}',serif;font-size:{:.1}px;font-weight:{weight};font-style:{style};color:rgb({},{},{});{align_css}white-space:pre;\">{}</div>\n",
        line.x_pt,
        line.y_pt,
        line.width_pt,
        escape_xml(&safe_font),
        line.font_size_pt,
        line.color.r,
        line.color.g,
        line.color.b,
        escape_xml(&line.text)
    ));
}

fn render_reflowable_page_xhtml(
    doc: &BookDocument,
    layout: &ComputedBookLayout,
    page_idx: usize,
) -> String {
    let page = &doc.pages[page_idx];
    let page_layout = layout.pages.get(page_idx);

    // Sort frames in natural reading order: top-to-bottom (`y`), then left-to-right (`x`).
    let mut reading_frames: Vec<&Frame> = page.frames.iter().collect();
    reading_frames.sort_by(|a, b| {
        a.rect
            .y
            .partial_cmp(&b.rect.y)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                a.rect
                    .x
                    .partial_cmp(&b.rect.x)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    let mut content_html = String::new();
    if !page.title.trim().is_empty() {
        content_html.push_str(&format!(
            "    <h2 class=\"chapter-title\">{}</h2>\n",
            escape_xml(&page.title)
        ));
    }

    for frame in reading_frames {
        match &frame.kind {
            FrameKind::Shape {
                kind: ShapeKind::DividerLine,
                ..
            } => {
                content_html.push_str("    <hr class=\"divider\"/>\n");
            }
            FrameKind::Shape { .. } => {}
            FrameKind::Image {
                asset_id, caption, ..
            } => {
                let resolved_entry = asset_id
                    .as_deref()
                    .and_then(|id| doc.assets.get(id))
                    .and_then(|entry| {
                        book_core::validate_safe_relative_bundle_path(&entry.relative_path)
                            .ok()
                            .map(|p| (entry, normalize_portable_rel_path(&p.to_string_lossy())))
                    });
                if resolved_entry.is_some() || !caption.trim().is_empty() {
                    content_html.push_str("    <figure class=\"epub-figure\">\n");
                    if let Some((entry, rel_path)) = resolved_entry {
                        let alt = if caption.trim().is_empty() {
                            &entry.original_filename
                        } else {
                            caption.trim()
                        };
                        content_html.push_str(&format!(
                            "      <img src=\"{}\" alt=\"{}\"/>\n",
                            escape_xml(&rel_path),
                            escape_xml(alt)
                        ));
                    }
                    if !caption.trim().is_empty() {
                        content_html.push_str(&format!(
                            "      <figcaption>{}</figcaption>\n",
                            escape_xml(caption.trim())
                        ));
                    }
                    content_html.push_str("    </figure>\n");
                }
            }
            FrameKind::Text {
                story_id,
                thread_order,
                ..
            } => {
                if let Some(computed) = page_layout.and_then(|pl| pl.text_frames.get(&frame.id)) {
                    let paragraphs = group_lines_into_paragraphs(&computed.lines);
                    let style = doc.stories.get(story_id).map(|s| &s.style);
                    let align_class = style
                        .map(|s| alignment_css_class(s.alignment))
                        .unwrap_or("align-left");

                    for para in paragraphs {
                        if !para.is_empty() {
                            content_html.push_str(&format!(
                                "    <p class=\"story-para {align_class}\">{}</p>\n",
                                escape_xml(&para)
                            ));
                        }
                    }
                } else if *thread_order == 0 {
                    // Fallback when layout was not precomputed
                    if let Some(story) = doc.stories.get(story_id) {
                        let align_class = alignment_css_class(story.style.alignment);
                        for raw_para in story.text.split('\n') {
                            let trimmed = raw_para.trim();
                            if !trimmed.is_empty() {
                                content_html.push_str(&format!(
                                    "    <p class=\"story-para {align_class}\">{}</p>\n",
                                    escape_xml(trimmed)
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
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
<head>
  <meta charset="utf-8"/>
  <title>{}</title>
  <meta name="viewport" content="width=device-width, initial-scale=1.0"/>
  <link rel="stylesheet" type="text/css" href="styles.css"/>
</head>
<body class="reflowable-page">
  <article class="chapter" epub:type="bodymatter chapter">
{content_html}  </article>
</body>
</html>"#,
        escape_xml(&page.title)
    )
}

fn group_lines_into_paragraphs(lines: &[PositionedTextLine]) -> Vec<String> {
    if lines.is_empty() {
        return Vec::new();
    }
    let mut paragraphs = Vec::new();
    let mut current = String::new();
    let mut prev_bottom: Option<f32> = None;

    for line in lines {
        let trimmed = line.text.trim();
        if trimmed.is_empty() {
            if !current.is_empty() {
                paragraphs.push(std::mem::take(&mut current));
            }
            continue;
        }
        if let Some(pb) = prev_bottom {
            // A vertical gap larger than 1.4x line height starts a new paragraph
            if line.y_pt - pb > line.height_pt * 0.45 && !current.is_empty() {
                paragraphs.push(std::mem::take(&mut current));
            }
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(trimmed);
        prev_bottom = Some(line.y_pt + line.height_pt);
    }

    if !current.is_empty() {
        paragraphs.push(current);
    }
    paragraphs
}

fn alignment_css_class(alignment: TextAlignment) -> &'static str {
    match alignment {
        TextAlignment::Left => "align-left",
        TextAlignment::Center => "align-center",
        TextAlignment::Right => "align-right",
        TextAlignment::Justified => "align-justify",
    }
}

/// Ensure bundle paths inside the EPUB never start with `/` or contain OS backslashes.
fn normalize_portable_rel_path(path: &str) -> String {
    let unified = path.replace('\\', "/");
    unified.trim_start_matches('/').to_string()
}

fn sanitize_xml_id(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "item".to_string()
    } else {
        out
    }
}

fn infer_mime_type(path: &str) -> &'static str {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".svg") {
        "image/svg+xml"
    } else if lower.ends_with(".ttf") {
        "font/ttf"
    } else if lower.ends_with(".otf") {
        "font/otf"
    } else if lower.ends_with(".woff2") {
        "font/woff2"
    } else {
        "application/octet-stream"
    }
}

fn escape_css_string(input: &str) -> String {
    input
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '"' | '\'' | ';' | '{' | '}' | '<' | '>' | '\\'))
        .collect()
}

/// Strips CSS/XHTML attribute injection characters (`['"', '\'', ';', '{', '}', '<', '>', '\\', '\n', '\r']`)
/// and control characters from a CSS font family identifier.
pub fn sanitize_css_identifier(input: &str) -> String {
    let cleaned: String = input
        .chars()
        .filter(|c| {
            !c.is_control()
                && !matches!(
                    c,
                    '"' | '\'' | ';' | '{' | '}' | '<' | '>' | '\\' | '\n' | '\r'
                )
        })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "serif".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Strips invalid XML 1.0 control characters (`\u{0000}..=\u{0008}`, `\u{000B}`, `\u{000C}`,
/// `\u{000E}..=\u{001F}`) and escapes XML special entities (`&`, `<`, `>`, `"`, `'`).
pub fn escape_xml(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if matches!(
            ch,
            '\u{0000}'..='\u{0008}' | '\u{000B}' | '\u{000C}' | '\u{000E}'..='\u{001F}'
        ) {
            continue;
        }
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use book_core::{
        AssetKind, BookDocument, ColorRgba, Frame, FrameKind, ImageFit, Page, PagePreset,
        PageSetup, Rect, ShapeKind, Story, TextAlignment, TextStyle,
    };
    use book_layout::ComputedBookLayout;
    use std::io::Read;

    fn build_sample_epub_book() -> BookDocument {
        let mut doc = BookDocument::default();
        doc.metadata.title = "Illustrated Voyages & Typography".to_string();
        doc.metadata.subtitle = "A Portable EPUB 3.3 Showcase".to_string();
        doc.metadata.author = "Ada Lovelace".to_string();
        doc.metadata.publisher = "Open Book Studio".to_string();
        doc.metadata.language = "en".to_string();
        doc.metadata.isbn = "978-1-4028-9462-6".to_string();
        doc.setup = PageSetup::from_preset(PagePreset::UsTrade6x9);

        if let Some(master) = doc.master_pages.first_mut() {
            master.header_text = "ILLUSTRATED VOYAGES".to_string();
            master.show_page_numbers = true;
        }
        let master_id = doc.master_pages.first().map(|m| m.id);

        // Register embedded image and font assets
        let img_id = doc.add_asset_bytes(
            "cover_map",
            AssetKind::Image,
            "world_map.png",
            "image/png",
            vec![137, 80, 78, 71, 13, 10, 26, 10, 1, 2, 3, 4],
            Some((1600, 1200)),
        );
        doc.add_asset_bytes(
            "body_font",
            AssetKind::Font,
            "Literata-Regular.ttf",
            "font/ttf",
            vec![0, 1, 0, 0, 0, 12],
            None,
        );

        // Create threaded story across Page 1 and Page 2
        let story = Story::new(
            "Chapter 1",
            "The first voyage began at dawn across calm seas. Navigators charted every island with precision.\n\nBy the second week, the fleet reached the southern archipelago where coral reefs shimmered beneath the hull.",
            TextStyle {
                font_family: "Literata".to_string(),
                font_size_pt: 12.0,
                line_height_factor: 1.4,
                bold: false,
                italic: false,
                color: ColorRgba::BLACK,
                alignment: TextAlignment::Justified,
            },
        );
        let story_id = doc.insert_story(story);

        // Page 1: RoundedRectangle, Ellipse, DividerLine, Image, and first threaded Text frame
        doc.pages[0].title = "Chapter 1: Departure".to_string();
        doc.pages[0].frames.push(Frame::new_shape(
            Rect::new(36.0, 44.0, 360.0, 40.0),
            ShapeKind::RoundedRectangle,
            ColorRgba::rgb(230, 238, 250),
        ));
        doc.pages[0].frames.push(Frame::new_shape(
            Rect::new(320.0, 100.0, 64.0, 64.0),
            ShapeKind::Ellipse,
            ColorRgba::WARM_GOLD,
        ));
        doc.pages[0].frames.push(Frame::new_shape(
            Rect::new(36.0, 175.0, 360.0, 2.0),
            ShapeKind::DividerLine,
            ColorRgba::ACCENT_BLUE,
        ));

        let mut img_frame = Frame::new_image(Rect::new(36.0, 190.0, 360.0, 160.0), Some(img_id));
        if let FrameKind::Image {
            ref mut caption,
            ref mut fit,
            ..
        } = img_frame.kind
        {
            *caption = "Map 1: Southern Archipelago Route".to_string();
            *fit = ImageFit::Contain;
        }
        doc.pages[0].frames.push(img_frame);

        let tf1 = Frame::new_text(Rect::new(36.0, 365.0, 360.0, 90.0), story_id, 0);
        let tf1_id = tf1.id;
        doc.pages[0].frames.push(tf1);

        // Page 2: Linked continuation text frame
        let mut page2 = Page::new("Chapter 1: The Archipelago", master_id);
        let tf2 = Frame::new_text(Rect::new(36.0, 48.0, 360.0, 280.0), story_id, 1);
        let tf2_id = tf2.id;
        page2.frames.push(tf2);
        doc.pages.push(page2);

        doc.link_text_frames(tf1_id, tf2_id);
        doc
    }

    #[test]
    fn epub_first_entry_is_uncompressed_mimetype() {
        let doc = build_sample_epub_book();
        let layout = ComputedBookLayout::compute(&doc);
        let bytes = export_epub_bytes(&doc, &layout, EpubLayoutMode::FixedLayout)
            .expect("EPUB export should succeed");

        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).expect("Valid ZIP archive");
        let mut first = archive.by_index(0).expect("First ZIP entry must exist");
        assert_eq!(first.name(), "mimetype", "First entry in EPUB must be mimetype");
        assert_eq!(
            first.compression(),
            zip::CompressionMethod::Stored,
            "mimetype entry must be stored uncompressed"
        );
        let mut content = String::new();
        first.read_to_string(&mut content).unwrap();
        assert_eq!(content, "application/epub+zip");
    }

    #[test]
    fn epub_fixed_layout_exports_viewport_shapes_and_embedded_assets() {
        let doc = build_sample_epub_book();
        let layout = ComputedBookLayout::compute(&doc);
        let bytes = export_epub_bytes(&doc, &layout, EpubLayoutMode::FixedLayout)
            .expect("Fixed-Layout EPUB export should succeed");

        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).expect("Valid ZIP archive");

        // Verify META-INF/container.xml
        {
            let mut container = archive
                .by_name("META-INF/container.xml")
                .expect("container.xml must exist");
            let mut xml = String::new();
            container.read_to_string(&mut xml).unwrap();
            assert!(xml.contains("OEBPS/content.opf"));
        }

        // Verify OEBPS/content.opf manifest and pre-paginated metadata
        {
            let mut opf_file = archive
                .by_name("OEBPS/content.opf")
                .expect("content.opf must exist");
            let mut opf = String::new();
            opf_file.read_to_string(&mut opf).unwrap();
            assert!(opf.contains("<meta property=\"rendition:layout\">pre-paginated</meta>"));
            assert!(opf.contains("href=\"styles.css\""));
            assert!(opf.contains("href=\"assets/images/cover_map_world_map.png\""));
            assert!(opf.contains("href=\"assets/fonts/body_font_Literata-Regular.ttf\""));
            assert!(opf.contains("id=\"page_001\""));
            assert!(opf.contains("id=\"page_002\""));
        }

        // Verify embedded binary asset exists in OEBPS/assets/...
        {
            let mut img_entry = archive
                .by_name("OEBPS/assets/images/cover_map_world_map.png")
                .expect("Embedded image asset must be written to OEBPS/assets/images/...");
            let mut img_bytes = Vec::new();
            img_entry.read_to_end(&mut img_bytes).unwrap();
            assert_eq!(img_bytes.len(), 12);
        }

        // Verify OEBPS/styles.css has @font-face
        {
            let mut css_file = archive
                .by_name("OEBPS/styles.css")
                .expect("styles.css must exist");
            let mut css = String::new();
            css_file.read_to_string(&mut css).unwrap();
            assert!(css.contains("@font-face"));
            assert!(css.contains("assets/fonts/body_font_Literata-Regular.ttf"));
        }

        // Verify OEBPS/page_001.xhtml has exact viewport, SVG ellipse/divider, image, and positioned text
        {
            let mut page1_file = archive
                .by_name("OEBPS/page_001.xhtml")
                .expect("page_001.xhtml must exist");
            let mut page1 = String::new();
            page1_file.read_to_string(&mut page1).unwrap();
            assert!(page1.contains("content=\"width=432, height=648\""));
            assert!(page1.contains("<ellipse"));
            assert!(page1.contains("<line"));
            assert!(page1.contains("src=\"assets/images/cover_map_world_map.png\""));
            assert!(page1.contains("ILLUSTRATED VOYAGES"));
        }
    }

    #[test]
    fn epub_reflowable_mode_exports_semantic_chapters_and_styles() {
        let doc = build_sample_epub_book();
        let layout = ComputedBookLayout::compute(&doc);
        let tmp_path =
            std::env::temp_dir().join(format!("book_test_{}.epub", std::process::id()));

        export_epub_to_file(&doc, &layout, EpubLayoutMode::Reflowable, &tmp_path)
            .expect("Reflowable EPUB export to file should succeed");

        let bytes = std::fs::read(&tmp_path).expect("EPUB file should exist on disk");
        let _ = std::fs::remove_file(&tmp_path);

        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).expect("Valid ZIP archive");

        {
            let mut opf_file = archive.by_name("OEBPS/content.opf").unwrap();
            let mut opf = String::new();
            opf_file.read_to_string(&mut opf).unwrap();
            assert!(opf.contains("<meta property=\"rendition:layout\">reflowable</meta>"));
        }

        {
            let mut page1_file = archive.by_name("OEBPS/page_001.xhtml").unwrap();
            let mut page1 = String::new();
            page1_file.read_to_string(&mut page1).unwrap();
            assert!(page1.contains("<link rel=\"stylesheet\" type=\"text/css\" href=\"styles.css\"/>"));
            assert!(page1.contains("<article class=\"chapter\""));
            assert!(page1.contains("<figure class=\"epub-figure\">"));
            assert!(page1.contains("<figcaption>Map 1: Southern Archipelago Route</figcaption>"));
            assert!(page1.contains("<hr class=\"divider\"/>"));
            assert!(page1.contains("<p class=\"story-para align-justify\">"));
        }
    }

    #[test]
    fn epub_neutralizes_xml_control_chars_and_css_html_injection_payloads() {
        assert_eq!(
            escape_xml("Hello\x00\x07<script>alert(1)</script>"),
            "Hello&lt;script&gt;alert(1)&lt;/script&gt;"
        );
        assert_eq!(
            sanitize_css_identifier("Literata';background:url(javascript:alert(1));\""),
            "Literatabackground:url(javascript:alert(1))"
        );

        let mut doc = BookDocument::default();
        doc.metadata.title = "Title\x00\x07<script>evil()</script>".to_string();
        let story = Story::new(
            "Injected Story",
            "Body\x00\x07 text with <b>tags</b>",
            TextStyle {
                font_family: "Literata\" onclick=\"alert(1);};".to_string(),
                ..TextStyle::default()
            },
        );
        let story_id = doc.insert_story(story);
        doc.pages[0]
            .frames
            .push(Frame::new_text(Rect::new(36.0, 36.0, 300.0, 120.0), story_id, 0));

        // Insert a tampered asset with path traversal in `relative_path`
        doc.assets.insert(
            "malicious_asset".to_string(),
            AssetEntry {
                id: "malicious_asset".to_string(),
                kind: AssetKind::Image,
                relative_path: "../../etc/passwd".to_string(),
                original_filename: "passwd".to_string(),
                mime_type: "image/png".to_string(),
                pixel_dimensions: None,
            },
        );
        doc.asset_bytes
            .insert("malicious_asset".to_string(), vec![1, 2, 3]);

        let layout = ComputedBookLayout::compute(&doc);
        let bytes = export_epub_bytes(&doc, &layout, EpubLayoutMode::FixedLayout)
            .expect("EPUB export should succeed safely");

        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert!(
            archive.by_name("OEBPS/../../etc/passwd").is_err(),
            "Malicious relative_path must never be written into EPUB archive"
        );

        let mut opf = String::new();
        archive
            .by_name("OEBPS/content.opf")
            .unwrap()
            .read_to_string(&mut opf)
            .unwrap();
        assert!(!opf.contains('\0') && !opf.contains('\x07'));
        assert!(!opf.contains("<script>"));
        assert!(!opf.contains("etc/passwd"));

        let mut page1 = String::new();
        archive
            .by_name("OEBPS/page_001.xhtml")
            .unwrap()
            .read_to_string(&mut page1)
            .unwrap();
        assert!(!page1.contains('\0') && !page1.contains('\x07'));
        assert!(!page1.contains("onclick=\"alert(1)"));
    }
}
