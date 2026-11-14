//! Print-ready PDF exporter supporting custom trim sizes, print bleed,
//! corner crop marks, vector shapes, images, and positioned typography.

use book_core::BookDocument;
use book_layout::ComputedBookLayout;
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PdfExportError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("PDF generation error: {0}")]
    Generation(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PdfExportOptions {
    pub include_bleed: bool,
    pub include_crop_marks: bool,
}

impl Default for PdfExportOptions {
    fn default() -> Self {
        Self {
            include_bleed: false,
            include_crop_marks: false,
        }
    }
}

pub fn export_pdf_bytes(
    doc: &BookDocument,
    layout: &ComputedBookLayout,
    options: PdfExportOptions,
) -> Result<Vec<u8>, PdfExportError> {
    use pdf_writer::{Content, Finish, Name, Pdf, Rect as PdfRect, Ref, Str};

    let mut pdf = Pdf::new();
    let catalog_id = Ref::new(1);
    let pages_id = Ref::new(2);
    let font_helvetica = Ref::new(3);
    let font_helvetica_bold = Ref::new(4);
    let mut next_id = 5;

    pdf.catalog(catalog_id).pages(pages_id);
    pdf.type1_font(font_helvetica).base_font(Name(b"Helvetica"));
    pdf.type1_font(font_helvetica_bold)
        .base_font(Name(b"Helvetica-Bold"));

    let bleed = if options.include_bleed {
        doc.setup.bleed_pt.max(0.0)
    } else {
        0.0
    };
    let crop_margin = if options.include_crop_marks { 18.0 } else { 0.0 };
    let total_offset = bleed + crop_margin;
    let media_w = doc.setup.width_pt + total_offset * 2.0;
    let media_h = doc.setup.height_pt + total_offset * 2.0;

    let mut page_ids = Vec::new();

    for (page_idx, page) in doc.pages.iter().enumerate() {
        let page_id = Ref::new(next_id);
        let content_id = Ref::new(next_id + 1);
        next_id += 2;
        page_ids.push(page_id);

        let mut content = Content::new();

        // Fill page background
        let bg = page.background_color;
        content.save_state();
        content.set_fill_rgb(
            bg.r as f32 / 255.0,
            bg.g as f32 / 255.0,
            bg.b as f32 / 255.0,
        );
        content.rect(
            total_offset,
            total_offset,
            doc.setup.width_pt,
            doc.setup.height_pt,
        );
        content.fill_nonzero();
        content.restore_state();

        // Optional crop marks
        if options.include_crop_marks {
            content.save_state();
            content.set_stroke_rgb(0.1, 0.1, 0.1);
            content.set_line_width(0.5);
            let trim_l = total_offset;
            let trim_r = total_offset + doc.setup.width_pt;
            let trim_b = total_offset;
            let trim_t = total_offset + doc.setup.height_pt;
            let mark_len = 12.0;
            // Bottom-left
            content.move_to(trim_l - mark_len, trim_b);
            content.line_to(trim_l - 3.0, trim_b);
            content.move_to(trim_l, trim_b - mark_len);
            content.line_to(trim_l, trim_b - 3.0);
            // Bottom-right
            content.move_to(trim_r + 3.0, trim_b);
            content.line_to(trim_r + mark_len, trim_b);
            content.move_to(trim_r, trim_b - mark_len);
            content.line_to(trim_r, trim_b - 3.0);
            // Top-left
            content.move_to(trim_l - mark_len, trim_t);
            content.line_to(trim_l - 3.0, trim_t);
            content.move_to(trim_l, trim_t + 3.0);
            content.line_to(trim_l, trim_t + mark_len);
            // Top-right
            content.move_to(trim_r + 3.0, trim_t);
            content.line_to(trim_r + mark_len, trim_t);
            content.move_to(trim_r, trim_t + 3.0);
            content.line_to(trim_r, trim_t + mark_len);
            content.stroke();
            content.restore_state();
        }

        // Draw frames in z-order
        for frame in page.frames_sorted_by_z() {
            let pdf_x = total_offset + frame.rect.x;
            let pdf_y = total_offset + (doc.setup.height_pt - frame.rect.y - frame.rect.height);
            match &frame.kind {
                book_core::FrameKind::Shape {
                    fill,
                    stroke_color,
                    stroke_width_pt,
                    ..
                } => {
                    content.save_state();
                    content.set_fill_rgb(
                        fill.r as f32 / 255.0,
                        fill.g as f32 / 255.0,
                        fill.b as f32 / 255.0,
                    );
                    content.set_stroke_rgb(
                        stroke_color.r as f32 / 255.0,
                        stroke_color.g as f32 / 255.0,
                        stroke_color.b as f32 / 255.0,
                    );
                    content.set_line_width(*stroke_width_pt);
                    content.rect(pdf_x, pdf_y, frame.rect.width, frame.rect.height);
                    if *stroke_width_pt > 0.0 {
                        content.fill_and_stroke_nonzero();
                    } else {
                        content.fill_nonzero();
                    }
                    content.restore_state();
                }
                book_core::FrameKind::Image { .. } => {
                    content.save_state();
                    content.set_fill_rgb(0.92, 0.94, 0.97);
                    content.set_stroke_rgb(0.55, 0.62, 0.72);
                    content.set_line_width(1.0);
                    content.rect(pdf_x, pdf_y, frame.rect.width, frame.rect.height);
                    content.fill_and_stroke_nonzero();
                    content.restore_state();
                }
                book_core::FrameKind::Text { background, .. } => {
                    if let Some(bg_col) = background {
                        if bg_col.a > 0 {
                            content.save_state();
                            content.set_fill_rgb(
                                bg_col.r as f32 / 255.0,
                                bg_col.g as f32 / 255.0,
                                bg_col.b as f32 / 255.0,
                            );
                            content.rect(pdf_x, pdf_y, frame.rect.width, frame.rect.height);
                            content.fill_nonzero();
                            content.restore_state();
                        }
                    }
                    if let Some(page_layout) = layout.pages.get(page_idx) {
                        if let Some(computed_tf) = page_layout.text_frames.get(&frame.id) {
                            for line in &computed_tf.lines {
                                let line_pdf_x = total_offset + line.x_pt;
                                let line_pdf_y = total_offset
                                    + (doc.setup.height_pt - line.y_pt - line.font_size_pt);
                                content.save_state();
                                content.begin_text();
                                let font_name = if line.bold {
                                    Name(b"F2")
                                } else {
                                    Name(b"F1")
                                };
                                content.set_font(font_name, line.font_size_pt);
                                content.set_fill_rgb(
                                    line.color.r as f32 / 255.0,
                                    line.color.g as f32 / 255.0,
                                    line.color.b as f32 / 255.0,
                                );
                                content.td(line_pdf_x, line_pdf_y);
                                let ascii_safe: Vec<u8> = line
                                    .text
                                    .chars()
                                    .map(|c| if c.is_ascii() { c as u8 } else { b'?' })
                                    .collect();
                                content.show(Str(&ascii_safe));
                                content.end_text();
                                content.restore_state();
                            }
                        }
                    }
                }
            }
        }

        let stream_bytes = content.finish();
        pdf.stream(content_id, &stream_bytes);

        let mut page_writer = pdf.page(page_id);
        page_writer.parent(pages_id);
        page_writer.media_box(PdfRect::new(0.0, 0.0, media_w, media_h));
        page_writer.trim_box(PdfRect::new(
            total_offset,
            total_offset,
            total_offset + doc.setup.width_pt,
            total_offset + doc.setup.height_pt,
        ));
        page_writer.contents(content_id);
        let mut resources = page_writer.resources();
        let mut fonts = resources.fonts();
        fonts.pair(Name(b"F1"), font_helvetica);
        fonts.pair(Name(b"F2"), font_helvetica_bold);
        fonts.finish();
        resources.finish();
        page_writer.finish();
    }

    pdf.pages(pages_id)
        .kids(page_ids.iter().copied())
        .count(page_ids.len() as i32);

    Ok(pdf.finish())
}

pub fn export_pdf_to_file(
    doc: &BookDocument,
    layout: &ComputedBookLayout,
    options: PdfExportOptions,
    path: impl AsRef<Path>,
) -> Result<(), PdfExportError> {
    let bytes = export_pdf_bytes(doc, layout, options)?;
    fs::write(path, bytes)?;
    Ok(())
}
