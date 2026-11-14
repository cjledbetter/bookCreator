//! Print-ready PDF exporter supporting custom trim sizes, print bleed,
//! corner crop marks, printer registration marks, vector shapes (rectangles,
//! rounded rectangles, ellipses, divider lines), styled image frames, master
//! page running headers/folios, and positioned typography with standard Type1 fonts.

use book_core::{
    BookDocument, ColorRgba, Frame, FrameKind, ImageFit, MasterPage, ShapeKind, TextAlignment,
};
use book_layout::{ComputedBookLayout, ComputedPageLayout, PositionedTextLine};
use pdf_writer::{Content, Finish, Name, Pdf, Rect as PdfRect, Ref, Str};
use std::fs;
use std::path::Path;
use thiserror::Error;

/// Standard cubic Bézier circle approximation constant (`4 * (sqrt(2) - 1) / 3`).
const BEZIER_KAPPA: f32 = 0.552_284_8;

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

/// Standard 14 Type1 font slots registered on every page.
struct StandardFontRefs {
    helvetica: Ref,
    helvetica_bold: Ref,
    helvetica_oblique: Ref,
    helvetica_bold_oblique: Ref,
    times_roman: Ref,
    times_bold: Ref,
    times_italic: Ref,
    times_bold_italic: Ref,
    courier: Ref,
    courier_bold: Ref,
    courier_oblique: Ref,
    courier_bold_oblique: Ref,
}

impl StandardFontRefs {
    fn register(pdf: &mut Pdf, next_id: &mut i32) -> Self {
        let mut alloc = || {
            let r = Ref::new(*next_id);
            *next_id += 1;
            r
        };
        let refs = Self {
            helvetica: alloc(),
            helvetica_bold: alloc(),
            helvetica_oblique: alloc(),
            helvetica_bold_oblique: alloc(),
            times_roman: alloc(),
            times_bold: alloc(),
            times_italic: alloc(),
            times_bold_italic: alloc(),
            courier: alloc(),
            courier_bold: alloc(),
            courier_oblique: alloc(),
            courier_bold_oblique: alloc(),
        };

        pdf.type1_font(refs.helvetica)
            .base_font(Name(b"Helvetica"))
            .encoding_predefined(Name(b"WinAnsiEncoding"));
        pdf.type1_font(refs.helvetica_bold)
            .base_font(Name(b"Helvetica-Bold"))
            .encoding_predefined(Name(b"WinAnsiEncoding"));
        pdf.type1_font(refs.helvetica_oblique)
            .base_font(Name(b"Helvetica-Oblique"))
            .encoding_predefined(Name(b"WinAnsiEncoding"));
        pdf.type1_font(refs.helvetica_bold_oblique)
            .base_font(Name(b"Helvetica-BoldOblique"))
            .encoding_predefined(Name(b"WinAnsiEncoding"));

        pdf.type1_font(refs.times_roman)
            .base_font(Name(b"Times-Roman"))
            .encoding_predefined(Name(b"WinAnsiEncoding"));
        pdf.type1_font(refs.times_bold)
            .base_font(Name(b"Times-Bold"))
            .encoding_predefined(Name(b"WinAnsiEncoding"));
        pdf.type1_font(refs.times_italic)
            .base_font(Name(b"Times-Italic"))
            .encoding_predefined(Name(b"WinAnsiEncoding"));
        pdf.type1_font(refs.times_bold_italic)
            .base_font(Name(b"Times-BoldItalic"))
            .encoding_predefined(Name(b"WinAnsiEncoding"));

        pdf.type1_font(refs.courier)
            .base_font(Name(b"Courier"))
            .encoding_predefined(Name(b"WinAnsiEncoding"));
        pdf.type1_font(refs.courier_bold)
            .base_font(Name(b"Courier-Bold"))
            .encoding_predefined(Name(b"WinAnsiEncoding"));
        pdf.type1_font(refs.courier_oblique)
            .base_font(Name(b"Courier-Oblique"))
            .encoding_predefined(Name(b"WinAnsiEncoding"));
        pdf.type1_font(refs.courier_bold_oblique)
            .base_font(Name(b"Courier-BoldOblique"))
            .encoding_predefined(Name(b"WinAnsiEncoding"));

        refs
    }

    fn bind_to_page_resources(&self, fonts: &mut pdf_writer::Dict<'_>) {
        fonts.pair(Name(b"F1"), self.helvetica);
        fonts.pair(Name(b"F2"), self.helvetica_bold);
        fonts.pair(Name(b"F3"), self.helvetica_oblique);
        fonts.pair(Name(b"F4"), self.helvetica_bold_oblique);
        fonts.pair(Name(b"F5"), self.times_roman);
        fonts.pair(Name(b"F6"), self.times_bold);
        fonts.pair(Name(b"F7"), self.times_italic);
        fonts.pair(Name(b"F8"), self.times_bold_italic);
        fonts.pair(Name(b"F9"), self.courier);
        fonts.pair(Name(b"F10"), self.courier_bold);
        fonts.pair(Name(b"F11"), self.courier_oblique);
        fonts.pair(Name(b"F12"), self.courier_bold_oblique);
    }
}

/// Map a document font family name and bold/italic flags to one of the 12 registered Type1 font resource names.
fn select_pdf_font_resource(font_family: &str, bold: bool, italic: bool) -> Name<'static> {
    let lower = font_family.to_ascii_lowercase();
    let is_mono = lower.contains("mono")
        || lower.contains("courier")
        || lower.contains("jetbrains")
        || lower.contains("code")
        || lower.contains("consolas");
    let is_serif = lower.contains("times")
        || lower.contains("literata")
        || lower.contains("georgia")
        || lower.contains("garamond")
        || (lower.contains("serif") && !lower.contains("sans"));

    if is_mono {
        match (bold, italic) {
            (false, false) => Name(b"F9"),
            (true, false) => Name(b"F10"),
            (false, true) => Name(b"F11"),
            (true, true) => Name(b"F12"),
        }
    } else if is_serif {
        match (bold, italic) {
            (false, false) => Name(b"F5"),
            (true, false) => Name(b"F6"),
            (false, true) => Name(b"F7"),
            (true, true) => Name(b"F8"),
        }
    } else {
        match (bold, italic) {
            (false, false) => Name(b"F1"),
            (true, false) => Name(b"F2"),
            (false, true) => Name(b"F3"),
            (true, true) => Name(b"F4"),
        }
    }
}

#[inline]
fn clamp_pdf_coord(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(-10_000.0, 10_000.0)
    } else {
        0.0
    }
}

#[inline]
fn clamp_pdf_dim(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 10_000.0)
    } else {
        0.0
    }
}

#[inline]
fn clamp_pdf_font_size(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(4.0, 400.0)
    } else {
        12.0
    }
}

#[inline]
fn clamp_pdf_stroke_width(v: f32) -> f32 {
    if v.is_finite() {
        v.clamp(0.0, 200.0)
    } else {
        0.0
    }
}

pub fn export_pdf_bytes(
    doc: &BookDocument,
    layout: &ComputedBookLayout,
    options: PdfExportOptions,
) -> Result<Vec<u8>, PdfExportError> {
    if !doc.setup.width_pt.is_finite()
        || !doc.setup.height_pt.is_finite()
        || doc.setup.width_pt <= 0.0
        || doc.setup.height_pt <= 0.0
    {
        return Err(PdfExportError::Generation(
            "Page width and height must be finite and positive".to_string(),
        ));
    }

    let page_w = doc.setup.width_pt.clamp(1.0, 5_000.0);
    let page_h = doc.setup.height_pt.clamp(1.0, 5_000.0);

    let mut pdf = Pdf::new();
    let catalog_id = Ref::new(1);
    let pages_id = Ref::new(2);
    let mut next_id = 3;

    pdf.catalog(catalog_id).pages(pages_id);
    let font_refs = StandardFontRefs::register(&mut pdf, &mut next_id);

    let bleed = if options.include_bleed && doc.setup.bleed_pt.is_finite() {
        doc.setup.bleed_pt.clamp(0.0, 72.0)
    } else {
        0.0
    };
    let crop_margin = if options.include_crop_marks { 18.0 } else { 0.0 };
    let total_offset = bleed + crop_margin;
    let media_w = page_w + total_offset * 2.0;
    let media_h = page_h + total_offset * 2.0;

    let mut page_ids = Vec::with_capacity(doc.pages.len());

    for (page_idx, page) in doc.pages.iter().enumerate() {
        let page_id = Ref::new(next_id);
        let content_id = Ref::new(next_id + 1);
        next_id += 2;
        page_ids.push(page_id);

        let master: Option<&MasterPage> = page
            .master_id
            .and_then(|mid| doc.master_pages.iter().find(|m| m.id == mid));

        let mut content = Content::new();

        // Fill page background (extending into bleed box when bleed is enabled)
        let bg = page.background_color;
        content.save_state();
        content.set_fill_rgb(
            bg.r as f32 / 255.0,
            bg.g as f32 / 255.0,
            bg.b as f32 / 255.0,
        );
        content.rect(
            crop_margin,
            crop_margin,
            page_w + bleed * 2.0,
            page_h + bleed * 2.0,
        );
        content.fill_nonzero();
        content.restore_state();

        // Optional crop marks & printer registration marks around the trim box
        if options.include_crop_marks {
            draw_crop_and_registration_marks(
                &mut content,
                total_offset,
                page_w,
                page_h,
                bleed,
            );
        }

        let page_layout = layout.pages.get(page_idx);

        // Render MasterPage frames first (underneath page frames)
        if let Some(master_page) = master {
            let mut master_frames: Vec<&Frame> = master_page.frames.iter().collect();
            master_frames.sort_by_key(|f| f.z_index);
            for frame in master_frames {
                render_frame(&mut content, doc, page_layout, frame, total_offset);
            }
        }

        // Draw page frames in z-order
        for frame in page.frames_sorted_by_z() {
            render_frame(&mut content, doc, page_layout, frame, total_offset);
        }

        // Render MasterPage running header and folio (page number)
        if let Some(master_page) = master {
            render_master_chrome(&mut content, doc, master_page, page_idx, total_offset);
        }

        let stream_bytes = content.finish();
        pdf.stream(content_id, &stream_bytes);

        let mut page_writer = pdf.page(page_id);
        page_writer.parent(pages_id);
        page_writer.media_box(PdfRect::new(0.0, 0.0, media_w, media_h));
        page_writer.bleed_box(PdfRect::new(
            crop_margin,
            crop_margin,
            crop_margin + page_w + bleed * 2.0,
            crop_margin + page_h + bleed * 2.0,
        ));
        page_writer.trim_box(PdfRect::new(
            total_offset,
            total_offset,
            total_offset + page_w,
            total_offset + page_h,
        ));
        page_writer.contents(content_id);

        let mut resources = page_writer.resources();
        let mut fonts = resources.fonts();
        font_refs.bind_to_page_resources(&mut fonts);
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

fn draw_crop_and_registration_marks(
    content: &mut Content,
    total_offset: f32,
    trim_w: f32,
    trim_h: f32,
    bleed: f32,
) {
    content.save_state();
    content.set_stroke_rgb(0.08, 0.08, 0.08);
    content.set_line_width(0.5);

    let trim_l = total_offset;
    let trim_r = total_offset + trim_w;
    let trim_b = total_offset;
    let trim_t = total_offset + trim_h;
    let gap = (bleed + 3.0).min(8.0);
    let mark_len = 15.0;

    // Bottom-left corner crop marks
    content.move_to(trim_l - mark_len, trim_b);
    content.line_to(trim_l - gap, trim_b);
    content.move_to(trim_l, trim_b - mark_len);
    content.line_to(trim_l, trim_b - gap);

    // Bottom-right corner crop marks
    content.move_to(trim_r + gap, trim_b);
    content.line_to(trim_r + mark_len, trim_b);
    content.move_to(trim_r, trim_b - mark_len);
    content.line_to(trim_r, trim_b - gap);

    // Top-left corner crop marks
    content.move_to(trim_l - mark_len, trim_t);
    content.line_to(trim_l - gap, trim_t);
    content.move_to(trim_l, trim_t + gap);
    content.line_to(trim_l, trim_t + mark_len);

    // Top-right corner crop marks
    content.move_to(trim_r + gap, trim_t);
    content.line_to(trim_r + mark_len, trim_t);
    content.move_to(trim_r, trim_t + gap);
    content.line_to(trim_r, trim_t + mark_len);

    // Midpoint printer registration crosshairs on all 4 sides
    let mid_x = (trim_l + trim_r) * 0.5;
    let mid_y = (trim_b + trim_t) * 0.5;
    let reg_offset = gap + 5.0;
    draw_registration_target(content, mid_x, trim_t + reg_offset, 3.0);
    draw_registration_target(content, mid_x, trim_b - reg_offset, 3.0);
    draw_registration_target(content, trim_l - reg_offset, mid_y, 3.0);
    draw_registration_target(content, trim_r + reg_offset, mid_y, 3.0);

    content.stroke();
    content.restore_state();
}

fn draw_registration_target(content: &mut Content, cx: f32, cy: f32, radius: f32) {
    // Crosshair lines
    let arm = radius + 2.0;
    content.move_to(cx - arm, cy);
    content.line_to(cx + arm, cy);
    content.move_to(cx, cy - arm);
    content.line_to(cx, cy + arm);
    // Registration circle via 4-quadrant cubic Bézier
    append_ellipse_path(content, cx - radius, cy - radius, radius * 2.0, radius * 2.0);
}

fn render_master_chrome(
    content: &mut Content,
    doc: &BookDocument,
    master: &MasterPage,
    page_idx: usize,
    total_offset: f32,
) {
    let safe = doc.setup.safe_margin_rect(page_idx);

    if !master.header_text.trim().is_empty() {
        let header_font_size = 8.5_f32;
        let header_y_top = (doc.setup.margins.top * 0.55).max(12.0);
        let pdf_y = total_offset + (doc.setup.height_pt - header_y_top - header_font_size);
        let est_w = estimate_text_width(&master.header_text, header_font_size, false);
        let pdf_x = total_offset + safe.x + ((safe.width - est_w) * 0.5).max(0.0);

        content.save_state();
        content.begin_text();
        content.set_font(Name(b"F5"), header_font_size);
        content.set_fill_rgb(0.38, 0.41, 0.46);
        content.next_line(pdf_x, pdf_y);
        let encoded = encode_pdf_text(&master.header_text);
        content.show(Str(&encoded));
        content.end_text();
        content.restore_state();
    }

    if master.show_page_numbers {
        let folio_str = format!("{}", page_idx + 1);
        let folio_font_size = 9.0_f32;
        let footer_y_from_bottom = (doc.setup.margins.bottom * 0.55).max(14.0);
        let pdf_y = total_offset + footer_y_from_bottom;
        let est_w = estimate_text_width(&folio_str, folio_font_size, false);
        let pdf_x = total_offset + (doc.setup.width_pt - est_w) * 0.5;

        content.save_state();
        content.begin_text();
        content.set_font(Name(b"F5"), folio_font_size);
        content.set_fill_rgb(0.32, 0.35, 0.40);
        content.next_line(pdf_x, pdf_y);
        let encoded = encode_pdf_text(&folio_str);
        content.show(Str(&encoded));
        content.end_text();
        content.restore_state();
    }
}

fn render_frame(
    content: &mut Content,
    doc: &BookDocument,
    page_layout: Option<&ComputedPageLayout>,
    frame: &Frame,
    total_offset: f32,
) {
    let fx = clamp_pdf_coord(frame.rect.x);
    let fy = clamp_pdf_coord(frame.rect.y);
    let w = clamp_pdf_dim(frame.rect.width);
    let h = clamp_pdf_dim(frame.rect.height);
    let page_h = doc.setup.height_pt.clamp(1.0, 5_000.0);
    let pdf_x = clamp_pdf_coord(total_offset + fx);
    let pdf_y = clamp_pdf_coord(total_offset + (page_h - fy - h));

    match &frame.kind {
        FrameKind::Shape {
            kind,
            fill,
            stroke_color,
            stroke_width_pt,
            corner_radius_pt,
        } => {
            render_shape_frame(
                content,
                *kind,
                pdf_x,
                pdf_y,
                w,
                h,
                *fill,
                *stroke_color,
                clamp_pdf_stroke_width(*stroke_width_pt),
                clamp_pdf_dim(*corner_radius_pt),
            );
        }
        FrameKind::Image {
            asset_id,
            fit,
            caption,
            corner_radius_pt,
        } => {
            render_image_frame(
                content,
                doc,
                pdf_x,
                pdf_y,
                w,
                h,
                asset_id.as_deref(),
                *fit,
                caption,
                clamp_pdf_dim(*corner_radius_pt),
            );
        }
        FrameKind::Text {
            padding_pt,
            background,
            ..
        } => {
            render_text_frame(
                content,
                doc,
                page_layout,
                frame,
                pdf_x,
                pdf_y,
                w,
                h,
                clamp_pdf_dim(*padding_pt),
                *background,
                total_offset,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_shape_frame(
    content: &mut Content,
    kind: ShapeKind,
    pdf_x: f32,
    pdf_y: f32,
    w: f32,
    h: f32,
    fill: ColorRgba,
    stroke_color: ColorRgba,
    stroke_width_pt: f32,
    corner_radius_pt: f32,
) {
    content.save_state();
    let has_fill = fill.a > 0;
    let has_stroke = stroke_width_pt > 0.0 && stroke_color.a > 0;

    if has_fill {
        content.set_fill_rgb(
            fill.r as f32 / 255.0,
            fill.g as f32 / 255.0,
            fill.b as f32 / 255.0,
        );
    }
    if has_stroke {
        content.set_stroke_rgb(
            stroke_color.r as f32 / 255.0,
            stroke_color.g as f32 / 255.0,
            stroke_color.b as f32 / 255.0,
        );
        content.set_line_width(stroke_width_pt);
    }

    match kind {
        ShapeKind::Rectangle => {
            content.rect(pdf_x, pdf_y, w, h);
            finish_fill_and_stroke(content, has_fill, has_stroke);
        }
        ShapeKind::RoundedRectangle => {
            append_rounded_rect_path(content, pdf_x, pdf_y, w, h, corner_radius_pt);
            finish_fill_and_stroke(content, has_fill, has_stroke);
        }
        ShapeKind::Ellipse => {
            append_ellipse_path(content, pdf_x, pdf_y, w, h);
            finish_fill_and_stroke(content, has_fill, has_stroke);
        }
        ShapeKind::DividerLine => {
            let line_color = if stroke_color.a > 0 {
                stroke_color
            } else {
                fill
            };
            content.set_stroke_rgb(
                line_color.r as f32 / 255.0,
                line_color.g as f32 / 255.0,
                line_color.b as f32 / 255.0,
            );
            content.set_line_width(stroke_width_pt.max(1.0));
            let mid_y = pdf_y + h * 0.5;
            content.move_to(pdf_x, mid_y);
            content.line_to(pdf_x + w, mid_y);
            content.stroke();
        }
    }

    content.restore_state();
}

fn finish_fill_and_stroke(content: &mut Content, has_fill: bool, has_stroke: bool) {
    match (has_fill, has_stroke) {
        (true, true) => {
            content.fill_nonzero_and_stroke();
        }
        (true, false) => {
            content.fill_nonzero();
        }
        (false, true) => {
            content.stroke();
        }
        (false, false) => {
            content.end_path();
        }
    }
}

/// Append a closed rounded-rectangle path using 4 cubic Bézier corner arcs.
fn append_rounded_rect_path(
    content: &mut Content,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radius: f32,
) {
    let r = radius.clamp(0.0, (w * 0.5).min(h * 0.5));
    if r <= 0.1 {
        content.rect(x, y, w, h);
        return;
    }
    let k = r * BEZIER_KAPPA;

    content.move_to(x + r, y);
    content.line_to(x + w - r, y);
    content.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    content.line_to(x + w, y + h - r);
    content.cubic_to(
        x + w,
        y + h - r + k,
        x + w - r + k,
        y + h,
        x + w - r,
        y + h,
    );
    content.line_to(x + r, y + h);
    content.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    content.line_to(x, y + r);
    content.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    content.close_path();
}

/// Append a closed 4-quadrant cubic Bézier ellipse inscribed in `(x, y, w, h)`.
fn append_ellipse_path(content: &mut Content, x: f32, y: f32, w: f32, h: f32) {
    let rx = w * 0.5;
    let ry = h * 0.5;
    let cx = x + rx;
    let cy = y + ry;
    let kx = rx * BEZIER_KAPPA;
    let ky = ry * BEZIER_KAPPA;

    content.move_to(cx + rx, cy);
    content.cubic_to(cx + rx, cy + ky, cx + kx, cy + ry, cx, cy + ry);
    content.cubic_to(cx - kx, cy + ry, cx - rx, cy + ky, cx - rx, cy);
    content.cubic_to(cx - rx, cy - ky, cx - kx, cy - ry, cx, cy - ry);
    content.cubic_to(cx + kx, cy - ry, cx + rx, cy - ky, cx + rx, cy);
    content.close_path();
}

#[allow(clippy::too_many_arguments)]
fn render_image_frame(
    content: &mut Content,
    doc: &BookDocument,
    pdf_x: f32,
    pdf_y: f32,
    w: f32,
    h: f32,
    asset_id: Option<&str>,
    fit: ImageFit,
    caption: &str,
    corner_radius_pt: f32,
) {
    content.save_state();

    // Outer frame surface & border
    content.set_fill_rgb(0.93, 0.95, 0.98);
    content.set_stroke_rgb(0.52, 0.60, 0.72);
    content.set_line_width(1.0);
    append_rounded_rect_path(content, pdf_x, pdf_y, w, h, corner_radius_pt);
    content.fill_nonzero_and_stroke();

    // Inner aspect-fit viewport guide box
    let inset = match fit {
        ImageFit::Contain => (w.min(h) * 0.08).clamp(4.0, 14.0),
        ImageFit::Cover => 3.0,
        ImageFit::Stretch => 5.0,
    };
    if w > inset * 2.5 && h > inset * 2.5 {
        let ix = pdf_x + inset;
        let iy = pdf_y + inset;
        let iw = w - inset * 2.0;
        let ih = h - inset * 2.0;

        content.set_stroke_rgb(0.75, 0.80, 0.88);
        content.set_line_width(0.6);
        content.rect(ix, iy, iw, ih);
        content.stroke();

        // Subtle diagonal frame lines
        content.set_stroke_rgb(0.82, 0.86, 0.92);
        content.set_line_width(0.4);
        content.move_to(ix, iy);
        content.line_to(ix + iw, iy + ih);
        content.move_to(ix, iy + ih);
        content.line_to(ix + iw, iy);
        content.stroke();
    }

    // Determine label / caption to show inside the image box
    let asset_label = asset_id
        .and_then(|id| doc.assets.get(id))
        .map(|entry| entry.original_filename.as_str());

    let display_text = if !caption.trim().is_empty() {
        Some(caption.trim().to_string())
    } else {
        asset_label.map(|name| format!("[Image: {name}]"))
    };

    if let Some(label) = display_text {
        let band_h = 18.0_f32.min(h * 0.35);
        if band_h >= 10.0 && w >= 30.0 {
            content.set_fill_rgb(0.18, 0.22, 0.29);
            content.rect(pdf_x + 1.0, pdf_y + 1.0, (w - 2.0).max(0.0), band_h);
            content.fill_nonzero();

            let font_size = (band_h * 0.52).clamp(7.0, 10.0);
            let text_w = estimate_text_width(&label, font_size, false);
            let text_x = pdf_x + ((w - text_w) * 0.5).max(4.0);
            let text_y = pdf_y + (band_h - font_size) * 0.5 + 1.5;

            content.begin_text();
            content.set_font(Name(b"F1"), font_size);
            content.set_fill_rgb(0.96, 0.97, 0.99);
            content.next_line(text_x, text_y);
            let encoded = encode_pdf_text(&label);
            content.show(Str(&encoded));
            content.end_text();
        }
    }

    content.restore_state();
}

#[allow(clippy::too_many_arguments)]
fn render_text_frame(
    content: &mut Content,
    doc: &BookDocument,
    page_layout: Option<&ComputedPageLayout>,
    frame: &Frame,
    pdf_x: f32,
    pdf_y: f32,
    w: f32,
    h: f32,
    padding_pt: f32,
    background: Option<ColorRgba>,
    total_offset: f32,
) {
    if let Some(bg_col) = background {
        if bg_col.a > 0 {
            content.save_state();
            content.set_fill_rgb(
                bg_col.r as f32 / 255.0,
                bg_col.g as f32 / 255.0,
                bg_col.b as f32 / 255.0,
            );
            content.rect(pdf_x, pdf_y, w, h);
            content.fill_nonzero();
            content.restore_state();
        }
    }

    let Some(computed_tf) = page_layout.and_then(|pl| pl.text_frames.get(&frame.id)) else {
        return;
    };

    let inner_left = frame.rect.x + padding_pt;

    for (line_idx, line) in computed_tf.lines.iter().enumerate() {
        if line.text.is_empty() {
            continue;
        }
        let is_last_line = line_idx + 1 == computed_tf.lines.len();
        render_positioned_text_line(
            content,
            line,
            inner_left,
            doc.setup.height_pt,
            total_offset,
            is_last_line,
        );
    }
}

fn render_positioned_text_line(
    content: &mut Content,
    line: &PositionedTextLine,
    frame_inner_left: f32,
    page_height_pt: f32,
    total_offset: f32,
    is_last_line: bool,
) {
    let safe_font_size = clamp_pdf_font_size(line.font_size_pt);
    let safe_line_x = clamp_pdf_coord(line.x_pt);
    let safe_line_y = clamp_pdf_coord(line.y_pt);
    let safe_line_w = clamp_pdf_dim(line.width_pt);
    let safe_inner_left = clamp_pdf_coord(frame_inner_left);

    let est_w = estimate_text_width(&line.text, safe_font_size, line.bold);
    let slack = (safe_line_w - est_w).max(0.0);

    // Check if the layout engine already shifted `line.x_pt` for alignment, or if `line.x_pt`
    // is at the left edge of the line segment.
    let already_shifted = (safe_line_x - safe_inner_left).abs() > 1.5;

    let aligned_x = match line.alignment {
        TextAlignment::Left | TextAlignment::Justified => safe_line_x,
        TextAlignment::Center => {
            if already_shifted {
                safe_line_x
            } else {
                safe_line_x + slack * 0.5
            }
        }
        TextAlignment::Right => {
            if already_shifted {
                safe_line_x
            } else {
                safe_line_x + slack
            }
        }
    };

    let line_pdf_x = clamp_pdf_coord(total_offset + aligned_x);
    let line_pdf_y = clamp_pdf_coord(total_offset + (page_height_pt - safe_line_y - safe_font_size));
    let font_res = select_pdf_font_resource(&line.font_family, line.bold, line.italic);

    content.save_state();
    content.begin_text();
    content.set_font(font_res, safe_font_size);
    content.set_fill_rgb(
        line.color.r as f32 / 255.0,
        line.color.g as f32 / 255.0,
        line.color.b as f32 / 255.0,
    );

    if line.alignment == TextAlignment::Justified && !is_last_line {
        let space_count = line.text.chars().filter(|&c| c == ' ').count();
        if space_count > 0 && slack > 0.0 && slack < safe_line_w * 0.35 {
            content.set_word_spacing(slack / space_count as f32);
        }
    }

    content.next_line(line_pdf_x, line_pdf_y);
    let encoded = encode_pdf_text(&line.text);
    content.show(Str(&encoded));
    content.end_text();
    content.restore_state();
}

/// Estimate proportional width of a text string in typographic points for standard Type1 fonts.
fn estimate_text_width(text: &str, font_size_pt: f32, bold: bool) -> f32 {
    let weight_factor = if bold { 1.06 } else { 1.0 };
    let units: f32 = text
        .chars()
        .map(|c| match c {
            ' ' | '\t' => 0.28,
            'i' | 'l' | 'I' | '!' | '|' | '.' | ',' | ':' | ';' | '\'' | '`' => 0.28,
            'f' | 'j' | 'r' | 't' | '(' | ')' | '[' | ']' | '"' | '-' => 0.36,
            'm' | 'w' | 'M' | 'W' | '@' | '%' | '—' => 0.78,
            c if c.is_ascii_uppercase() => 0.64,
            c if c.is_ascii_digit() => 0.54,
            _ => 0.50,
        })
        .sum();
    units * font_size_pt * weight_factor
}

/// Convert UTF-8 text into printable ASCII bytes suitable for PDF literal strings `(...)`
/// on standard Type1 fonts, normalizing smart punctuation to clean ASCII equivalents.
/// Note: `pdf_writer::Str` escapes unbalanced `(` / `)` and `\` when writing the literal string.
pub fn encode_pdf_text(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + 8);
    for ch in text.chars() {
        match ch {
            '\u{0020}'..='\u{007E}' => out.push(ch as u8),
            '—' => out.extend_from_slice(b"--"),
            '–' => out.push(b'-'),
            '‘' | '’' => out.push(b'\''),
            '“' | '”' => out.push(b'"'),
            '•' => out.push(b'*'),
            '…' => out.extend_from_slice(b"..."),
            '\t' => out.push(b' '),
            '\n' | '\r' => continue,
            _ => out.push(b'?'),
        }
    }
    out
}

/// Escape raw PDF literal string characters `(`, `)`, and `\` for direct PDF syntax inspection.
pub fn escape_pdf_literal_string(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len() + 8);
    for ch in text.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '(' => escaped.push_str("\\("),
            ')' => escaped.push_str("\\)"),
            '\r' => escaped.push_str("\\r"),
            '\n' => escaped.push_str("\\n"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use book_core::{
        AssetKind, BookDocument, ColorRgba, Frame, ImageFit, Page, PagePreset, PageSetup, Rect,
        ShapeKind, Story, TextAlignment, TextStyle,
    };
    use book_layout::ComputedBookLayout;

    fn build_sample_multi_page_book() -> BookDocument {
        let mut doc = BookDocument::default();
        doc.metadata.title = "The Rust Book Creator Manual".to_string();
        doc.metadata.author = "Studio Press".to_string();
        doc.setup = PageSetup::from_preset(PagePreset::UsTrade6x9);
        doc.setup.bleed_pt = 9.0;

        if let Some(master) = doc.master_pages.first_mut() {
            master.show_page_numbers = true;
            master.header_text = "THE RUST BOOK CREATOR".to_string();
            master.frames.push(Frame::new_shape(
                Rect::new(36.0, 28.0, 360.0, 1.5),
                ShapeKind::DividerLine,
                ColorRgba::rgb(180, 185, 195),
            ));
        }
        let master_id = doc.master_pages.first().map(|m| m.id);

        // Story 1: Serif Justified
        let serif_story = Story::new(
            "Chapter 1",
            "Welcome to Book Creator! (Unbalanced paren and 1\\2 backslash — This paragraph tests parentheses, backslashes C:\\books\\rust, and justified line spacing across a full column width so every line renders crisply in PDF.",
            TextStyle {
                font_family: "Literata".to_string(),
                font_size_pt: 12.0,
                line_height_factor: 1.35,
                bold: false,
                italic: false,
                color: ColorRgba::BLACK,
                alignment: TextAlignment::Justified,
            },
        );
        let story1_id = doc.insert_story(serif_story);

        // Story 2: Sans Bold Centered
        let title_story = Story::new(
            "Heading",
            "Chapter One: Print Prepress & Typography",
            TextStyle {
                font_family: "Inter".to_string(),
                font_size_pt: 18.0,
                line_height_factor: 1.2,
                bold: true,
                italic: false,
                color: ColorRgba::ACCENT_BLUE,
                alignment: TextAlignment::Center,
            },
        );
        let story2_id = doc.insert_story(title_story);

        // Story 3: Monospace Italic Right-aligned
        let mono_story = Story::new(
            "Code Note",
            "cargo run --release (x86_64 & arm64)",
            TextStyle {
                font_family: "JetBrains Mono".to_string(),
                font_size_pt: 10.0,
                line_height_factor: 1.3,
                bold: false,
                italic: true,
                color: ColorRgba::rgb(60, 70, 90),
                alignment: TextAlignment::Right,
            },
        );
        let story3_id = doc.insert_story(mono_story);

        // Register an image asset
        let asset_id = doc.add_asset_bytes(
            "hero_diagram",
            AssetKind::Image,
            "diagram.png",
            "image/png",
            vec![137, 80, 78, 71, 13, 10, 26, 10],
            Some((1200, 800)),
        );

        // Page 1: All shape kinds + heading + body + image frame
        doc.pages[0].title = "Page 1: Shapes & Typography".to_string();
        doc.pages[0].frames.push(Frame::new_shape(
            Rect::new(36.0, 48.0, 360.0, 42.0),
            ShapeKind::RoundedRectangle,
            ColorRgba::rgb(235, 242, 252),
        ));
        doc.pages[0].frames.push(Frame::new_text(
            Rect::new(42.0, 54.0, 348.0, 34.0),
            story2_id,
            0,
        ));
        doc.pages[0].frames.push(Frame::new_shape(
            Rect::new(36.0, 100.0, 360.0, 2.0),
            ShapeKind::DividerLine,
            ColorRgba::ACCENT_BLUE,
        ));
        doc.pages[0].frames.push(Frame::new_shape(
            Rect::new(310.0, 120.0, 72.0, 72.0),
            ShapeKind::Ellipse,
            ColorRgba::WARM_GOLD,
        ));
        doc.pages[0].frames.push(Frame::new_text(
            Rect::new(36.0, 120.0, 260.0, 220.0),
            story1_id,
            0,
        ));

        let mut img_frame = Frame::new_image(
            Rect::new(36.0, 360.0, 360.0, 180.0),
            Some(asset_id),
        );
        if let FrameKind::Image {
            ref mut caption,
            ref mut fit,
            ..
        } = img_frame.kind
        {
            *caption = "Figure 1.1: Press Sheet Trim & Bleed Geometry".to_string();
            *fit = ImageFit::Contain;
        }
        doc.pages[0].frames.push(img_frame);

        // Page 2: Rectangle + monospace right-aligned text
        let mut page2 = Page::new("Page 2: Code & Layout", master_id);
        page2.frames.push(Frame::new_shape(
            Rect::new(36.0, 60.0, 360.0, 80.0),
            ShapeKind::Rectangle,
            ColorRgba::rgb(245, 247, 250),
        ));
        let mut code_tf = Frame::new_text(Rect::new(42.0, 70.0, 348.0, 60.0), story3_id, 0);
        if let FrameKind::Text {
            ref mut background, ..
        } = code_tf.kind
        {
            *background = Some(ColorRgba::rgb(238, 241, 246));
        }
        page2.frames.push(code_tf);
        doc.pages.push(page2);

        doc
    }

    #[test]
    fn pdf_export_generates_valid_header_eof_and_multipage_structure() {
        let doc = build_sample_multi_page_book();
        let layout = ComputedBookLayout::compute(&doc);
        let bytes = export_pdf_bytes(&doc, &layout, PdfExportOptions::default())
            .expect("PDF export should succeed");

        assert!(bytes.starts_with(b"%PDF-1."), "PDF must start with %PDF-1.x header");
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.trim_end().ends_with("%%EOF"), "PDF must end with %%EOF trailer");
        assert!(text.contains("/Count 2"), "Pages tree must declare 2 pages");
        assert!(text.contains("/BaseFont /Times-Roman"), "Must register Times-Roman");
        assert!(text.contains("/BaseFont /Helvetica-Bold"), "Must register Helvetica-Bold");
        assert!(text.contains("/BaseFont /Courier-Oblique"), "Must register Courier-Oblique");
    }

    #[test]
    fn pdf_export_includes_bleed_box_trim_box_and_crop_marks() {
        let doc = build_sample_multi_page_book();
        let layout = ComputedBookLayout::compute(&doc);
        let bytes = export_pdf_bytes(
            &doc,
            &layout,
            PdfExportOptions {
                include_bleed: true,
                include_crop_marks: true,
            },
        )
        .expect("PDF export with bleed and crop marks should succeed");

        let text = String::from_utf8_lossy(&bytes);
        // US Trade 6x9 is 432 x 648 pt.
        // bleed = 9 pt, crop_margin = 18 pt => total_offset = 27 pt.
        // MediaBox = [0 0 486 702], BleedBox = [18 18 468 684], TrimBox = [27 27 459 675].
        assert!(text.contains("/MediaBox [0 0 486 702]"), "MediaBox must include bleed + crop margin");
        assert!(text.contains("/BleedBox [18 18 468 684]"), "BleedBox must expand trim by 9pt bleed");
        assert!(text.contains("/TrimBox [27 27 459 675]"), "TrimBox must match 432x648 trim at offset 27");
    }

    #[test]
    fn pdf_export_escapes_parentheses_backslashes_and_renders_shapes_and_captions() {
        assert_eq!(
            escape_pdf_literal_string("(Edition 1\\2)"),
            "\\(Edition 1\\\\2\\)"
        );

        let doc = build_sample_multi_page_book();
        let layout = ComputedBookLayout::compute(&doc);
        let bytes = export_pdf_bytes(&doc, &layout, PdfExportOptions::default())
            .expect("PDF export should succeed");
        let text = String::from_utf8_lossy(&bytes);

        // Verify escaped parentheses and backslashes in content stream
        assert!(
            text.contains("\\(Unbalanced") && text.contains("1\\\\2"),
            "Parentheses and backslashes must be properly escaped in PDF strings"
        );
        // Verify cubic Bezier operator `c` is emitted for RoundedRectangle and Ellipse shapes
        assert!(
            text.contains(" c\n"),
            "RoundedRectangle and Ellipse must emit cubic Bezier curve operators"
        );
        // Verify image caption and master page header are rendered
        assert!(
            text.contains("Figure 1.1: Press Sheet Trim & Bleed Geometry"),
            "Image frame caption must appear in PDF stream"
        );
        assert!(
            text.contains("THE RUST BOOK CREATOR"),
            "MasterPage header text must appear in PDF stream"
        );
    }

    #[test]
    fn pdf_export_to_file_roundtrip() {
        let doc = build_sample_multi_page_book();
        let layout = ComputedBookLayout::compute(&doc);
        let tmp_path =
            std::env::temp_dir().join(format!("book_test_{}.pdf", std::process::id()));
        export_pdf_to_file(
            &doc,
            &layout,
            PdfExportOptions {
                include_bleed: true,
                include_crop_marks: false,
            },
            &tmp_path,
        )
        .expect("export_pdf_to_file should succeed");

        let disk_bytes = std::fs::read(&tmp_path).expect("PDF file should exist on disk");
        let _ = std::fs::remove_file(&tmp_path);
        assert!(disk_bytes.starts_with(b"%PDF-1."));
    }

    #[test]
    fn pdf_export_clamps_non_finite_geometry_and_neutralizes_control_chars() {
        let mut bad_doc = BookDocument::default();
        bad_doc.setup.width_pt = f32::NAN;
        let empty_layout = ComputedBookLayout::default();
        assert!(
            export_pdf_bytes(&bad_doc, &empty_layout, PdfExportOptions::default()).is_err(),
            "PDF exporter must reject non-finite page dimensions"
        );

        let mut doc = BookDocument::default();
        doc.setup.bleed_pt = f32::INFINITY;
        let story = Story::new(
            "Control Char Story",
            "Hello\x00\x07PDF((World))\\Test",
            TextStyle {
                font_family: "Literata".to_string(),
                font_size_pt: f32::NAN,
                line_height_factor: f32::INFINITY,
                bold: false,
                italic: false,
                color: ColorRgba::BLACK,
                alignment: TextAlignment::Left,
            },
        );
        let sid = doc.insert_story(story);
        doc.pages[0]
            .frames
            .push(Frame::new_text(Rect::new(f32::NAN, f32::INFINITY, -50.0, 200.0), sid, 0));

        let mut shape = Frame::new_shape(
            Rect::new(f32::NEG_INFINITY, 20.0, f32::NAN, 80.0),
            ShapeKind::RoundedRectangle,
            ColorRgba::ACCENT_BLUE,
        );
        if let FrameKind::Shape {
            ref mut stroke_width_pt,
            ref mut corner_radius_pt,
            ..
        } = shape.kind
        {
            *stroke_width_pt = f32::INFINITY;
            *corner_radius_pt = f32::NAN;
        }
        doc.pages[0].frames.push(shape);

        let layout = ComputedBookLayout::compute(&doc);
        let bytes = export_pdf_bytes(
            &doc,
            &layout,
            PdfExportOptions {
                include_bleed: true,
                include_crop_marks: true,
            },
        )
        .expect("PDF export with clamped non-finite frame values should succeed");

        let pdf_str = String::from_utf8_lossy(&bytes);
        assert!(!pdf_str.contains("NaN"), "PDF stream must never contain NaN");
        assert!(!pdf_str.contains("inf"), "PDF stream must never contain inf");
        assert!(!bytes.contains(&0x07), "Control char 0x07 must be stripped/replaced");
    }
}
