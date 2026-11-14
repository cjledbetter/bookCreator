//! Interactive WYSIWYG spread canvas: page rendering, drop shadows, bleed & safe margin
//! guides, smart alignment snap guides, 8-handle frame resizing, rubber-band frame
//! creation, visual text-threading arrows, `[+]` overflow ports, and on-canvas text editing.

use book_core::{
    BookDocument, ColorRgba, FrameId, FrameKind, ImageFit, Page, PageSetup, Point, Rect,
    ShapeKind, TextAlignment, WrapMode,
};
use book_layout::ComputedBookLayout;
use eframe::egui;

use crate::{BookCreatorApp, Tool};

/// Which of the 8 resize handles around a selected frame is being dragged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeHandle {
    NorthWest,
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
}

impl ResizeHandle {
    pub const ALL: [ResizeHandle; 8] = [
        ResizeHandle::NorthWest,
        ResizeHandle::North,
        ResizeHandle::NorthEast,
        ResizeHandle::East,
        ResizeHandle::SouthEast,
        ResizeHandle::South,
        ResizeHandle::SouthWest,
        ResizeHandle::West,
    ];

    /// Position of this handle in page coordinates (`pt`) for a given `rect`.
    pub fn position_pt(self, rect: Rect) -> Point {
        let cx = rect.x + rect.width * 0.5;
        let cy = rect.y + rect.height * 0.5;
        let r = rect.right();
        let b = rect.bottom();
        match self {
            ResizeHandle::NorthWest => Point::new(rect.x, rect.y),
            ResizeHandle::North => Point::new(cx, rect.y),
            ResizeHandle::NorthEast => Point::new(r, rect.y),
            ResizeHandle::East => Point::new(r, cy),
            ResizeHandle::SouthEast => Point::new(r, b),
            ResizeHandle::South => Point::new(cx, b),
            ResizeHandle::SouthWest => Point::new(rect.x, b),
            ResizeHandle::West => Point::new(rect.x, cy),
        }
    }

    pub fn cursor_icon(self) -> egui::CursorIcon {
        match self {
            ResizeHandle::NorthWest | ResizeHandle::SouthEast => egui::CursorIcon::ResizeNwSe,
            ResizeHandle::NorthEast | ResizeHandle::SouthWest => egui::CursorIcon::ResizeNeSw,
            ResizeHandle::North | ResizeHandle::South => egui::CursorIcon::ResizeVertical,
            ResizeHandle::East | ResizeHandle::West => egui::CursorIcon::ResizeHorizontal,
        }
    }

    /// Apply a drag delta `(dx_pt, dy_pt)` to an initial rectangle, enforcing minimum size.
    pub fn apply_delta(self, initial: Rect, dx_pt: f32, dy_pt: f32, min_size_pt: f32) -> Rect {
        let min_s = min_size_pt.max(16.0);
        let mut left = initial.x;
        let mut top = initial.y;
        let mut right = initial.right();
        let mut bottom = initial.bottom();

        match self {
            ResizeHandle::NorthWest => {
                left = (left + dx_pt).min(right - min_s);
                top = (top + dy_pt).min(bottom - min_s);
            }
            ResizeHandle::North => {
                top = (top + dy_pt).min(bottom - min_s);
            }
            ResizeHandle::NorthEast => {
                right = (right + dx_pt).max(left + min_s);
                top = (top + dy_pt).min(bottom - min_s);
            }
            ResizeHandle::East => {
                right = (right + dx_pt).max(left + min_s);
            }
            ResizeHandle::SouthEast => {
                right = (right + dx_pt).max(left + min_s);
                bottom = (bottom + dy_pt).max(top + min_s);
            }
            ResizeHandle::South => {
                bottom = (bottom + dy_pt).max(top + min_s);
            }
            ResizeHandle::SouthWest => {
                left = (left + dx_pt).min(right - min_s);
                bottom = (bottom + dy_pt).max(top + min_s);
            }
            ResizeHandle::West => {
                left = (left + dx_pt).min(right - min_s);
            }
        }

        Rect::new(left, top, right - left, bottom - top)
    }
}

/// Smart alignment guide line rendered in magenta/cyan during frame dragging.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SmartGuideLine {
    Vertical {
        x_pt: f32,
        y_start_pt: f32,
        y_end_pt: f32,
        is_page_guide: bool,
    },
    Horizontal {
        y_pt: f32,
        x_start_pt: f32,
        x_end_pt: f32,
        is_page_guide: bool,
    },
}

/// Active pointer drag mode on the spread canvas.
#[derive(Debug, Clone, PartialEq)]
pub enum DragMode {
    None,
    MovingFrame {
        frame_id: FrameId,
        start_pointer_pt: Point,
        initial_rect: Rect,
        snapshot_recorded: bool,
    },
    ResizingFrame {
        frame_id: FrameId,
        handle: ResizeHandle,
        start_pointer_pt: Point,
        initial_rect: Rect,
        snapshot_recorded: bool,
    },
    DrawingNewFrame {
        tool: Tool,
        start_pt: Point,
        current_pt: Point,
    },
}

impl Default for DragMode {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone, Default)]
pub struct CanvasInteractionState {
    pub drag_mode: DragMode,
    pub active_guides: Vec<SmartGuideLine>,
    pub last_canvas_size: egui::Vec2,
}

/// Convert `book_core::ColorRgba` to `egui::Color32`.
pub fn to_color32(c: ColorRgba) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

/// Convert `egui::Color32` to `book_core::ColorRgba`.
pub fn from_color32(c: egui::Color32) -> ColorRgba {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    ColorRgba::rgba(r, g, b, a)
}

/// Hit-test frames on `page` in reverse `z_index` order (topmost frame first).
pub fn hit_test_frame(page: &Page, pt: Point) -> Option<FrameId> {
    let mut sorted = page.frames_sorted_by_z();
    sorted.reverse();
    for frame in sorted {
        if frame.rect.contains(pt) {
            return Some(frame.id);
        }
    }
    None
}

/// Compute smart alignment snapping against page center, safe margins, and other frames.
/// Returns `(snapped_rect, active_guide_lines)`.
pub fn compute_smart_snap(
    page: &Page,
    setup: &PageSetup,
    page_index: usize,
    moving_frame_id: FrameId,
    candidate: Rect,
    snap_threshold_pt: f32,
) -> (Rect, Vec<SmartGuideLine>) {
    let mut snapped = candidate;
    let mut guides = Vec::new();

    let margin = setup.safe_margin_rect(page_index);
    let page_center_x = setup.width_pt * 0.5;
    let page_center_y = setup.height_pt * 0.5;

    // Candidate X anchors: (x_value_in_candidate, offset_to_apply_to_candidate_x)
    // 0: left edge, 1: center X, 2: right edge
    let cand_x_anchors = [
        (candidate.x, 0.0),
        (candidate.x + candidate.width * 0.5, -candidate.width * 0.5),
        (candidate.right(), -candidate.width),
    ];

    // Target X lines: (target_x, y_min, y_max, is_page_guide)
    let mut x_targets: Vec<(f32, f32, f32, bool)> = vec![
        (page_center_x, 0.0, setup.height_pt, true),
        (margin.x, 0.0, setup.height_pt, true),
        (margin.right(), 0.0, setup.height_pt, true),
    ];

    // Candidate Y anchors: 0: top edge, 1: center Y, 2: bottom edge
    let cand_y_anchors = [
        (candidate.y, 0.0),
        (
            candidate.y + candidate.height * 0.5,
            -candidate.height * 0.5,
        ),
        (candidate.bottom(), -candidate.height),
    ];

    let mut y_targets: Vec<(f32, f32, f32, bool)> = vec![
        (page_center_y, 0.0, setup.width_pt, true),
        (margin.y, 0.0, setup.width_pt, true),
        (margin.bottom(), 0.0, setup.width_pt, true),
    ];

    for other in &page.frames {
        if other.id == moving_frame_id {
            continue;
        }
        let r = other.rect;
        let y_min = candidate.y.min(r.y);
        let y_max = candidate.bottom().max(r.bottom());
        x_targets.push((r.x, y_min, y_max, false));
        x_targets.push((r.center().x, y_min, y_max, false));
        x_targets.push((r.right(), y_min, y_max, false));

        let x_min = candidate.x.min(r.x);
        let x_max = candidate.right().max(r.right());
        y_targets.push((r.y, x_min, x_max, false));
        y_targets.push((r.center().y, x_min, x_max, false));
        y_targets.push((r.bottom(), x_min, x_max, false));
    }

    // Find closest X snap within threshold
    let mut best_x: Option<(f32, f32, SmartGuideLine)> = None;
    for &(anchor_x, offset_x) in &cand_x_anchors {
        for &(target_x, y_start, y_end, is_page) in &x_targets {
            let dist = (anchor_x - target_x).abs();
            if dist <= snap_threshold_pt {
                if best_x.map_or(true, |(best_d, _, _)| dist < best_d) {
                    best_x = Some((
                        dist,
                        target_x + offset_x,
                        SmartGuideLine::Vertical {
                            x_pt: target_x,
                            y_start_pt: y_start,
                            y_end_pt: y_end,
                            is_page_guide: is_page,
                        },
                    ));
                }
            }
        }
    }
    if let Some((_, new_x, guide)) = best_x {
        snapped.x = new_x;
        guides.push(guide);
    }

    // Find closest Y snap within threshold
    let mut best_y: Option<(f32, f32, SmartGuideLine)> = None;
    for &(anchor_y, offset_y) in &cand_y_anchors {
        for &(target_y, x_start, x_end, is_page) in &y_targets {
            let dist = (anchor_y - target_y).abs();
            if dist <= snap_threshold_pt {
                if best_y.map_or(true, |(best_d, _, _)| dist < best_d) {
                    best_y = Some((
                        dist,
                        target_y + offset_y,
                        SmartGuideLine::Horizontal {
                            y_pt: target_y,
                            x_start_pt: x_start,
                            x_end_pt: x_end,
                            is_page_guide: is_page,
                        },
                    ));
                }
            }
        }
    }
    if let Some((_, new_y, guide)) = best_y {
        snapped.y = new_y;
        guides.push(guide);
    }

    (snapped, guides)
}

/// Coordinate mapping between page points (`pt`) and screen pixels (`egui::Pos2`).
#[derive(Debug, Clone, Copy)]
pub struct PageTransform {
    pub page_origin_screen: egui::Pos2,
    pub zoom: f32,
}

impl PageTransform {
    pub fn to_screen(self, pt: Point) -> egui::Pos2 {
        egui::pos2(
            self.page_origin_screen.x + pt.x * self.zoom,
            self.page_origin_screen.y + pt.y * self.zoom,
        )
    }

    pub fn rect_to_screen(self, r: Rect) -> egui::Rect {
        egui::Rect::from_min_size(
            self.to_screen(Point::new(r.x, r.y)),
            egui::vec2(r.width * self.zoom, r.height * self.zoom),
        )
    }

    pub fn to_page_pt(self, screen_pos: egui::Pos2) -> Point {
        Point::new(
            (screen_pos.x - self.page_origin_screen.x) / self.zoom,
            (screen_pos.y - self.page_origin_screen.y) / self.zoom,
        )
    }
}

/// Render the center interactive spread canvas and process mouse/keyboard interactions.
pub fn show_spread_canvas(app: &mut BookCreatorApp, ui: &mut egui::Ui) {
    let available = ui.available_rect_before_wrap();
    app.canvas_interaction.last_canvas_size = available.size();

    let (response, painter) =
        ui.allocate_painter(available.size(), egui::Sense::click_and_drag());

    // Dark studio pasteboard background
    painter.rect_filled(available, 0.0, egui::Color32::from_rgb(30, 33, 40));

    if app.doc.pages.is_empty() {
        painter.text(
            available.center(),
            egui::Align2::CENTER_CENTER,
            "No pages in document. Click '+ Add Page' in the Left Page Rail.",
            egui::FontId::proportional(16.0),
            egui::Color32::LIGHT_GRAY,
        );
        return;
    }

    app.current_page = app.current_page.min(app.doc.pages.len() - 1);

    // Handle panning (middle-drag or right-drag or Space+drag) & Ctrl/Cmd+Wheel zoom
    if response.hovered() {
        let scroll_delta = ui.input(|i| i.smooth_scroll_delta);
        let zoom_delta = ui.input(|i| i.zoom_delta());
        if (zoom_delta - 1.0).abs() > 0.001 {
            app.zoom = (app.zoom * zoom_delta).clamp(0.25, 4.0);
        } else if scroll_delta != egui::Vec2::ZERO {
            let modifiers = ui.input(|i| i.modifiers);
            if modifiers.command || modifiers.ctrl {
                let factor = if scroll_delta.y > 0.0 { 1.08 } else { 0.92 };
                app.zoom = (app.zoom * factor).clamp(0.25, 4.0);
            } else {
                app.pan_offset += scroll_delta;
            }
        }
    }

    if response.dragged_by(egui::PointerButton::Middle)
        || (response.dragged_by(egui::PointerButton::Primary)
            && ui.input(|i| i.key_down(egui::Key::Space)))
    {
        app.pan_offset += response.drag_delta();
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    }

    let page_w_screen = app.doc.setup.width_pt * app.zoom;
    let page_h_screen = app.doc.setup.height_pt * app.zoom;
    let page_origin = egui::pos2(
        available.center().x - page_w_screen * 0.5 + app.pan_offset.x,
        available.center().y - page_h_screen * 0.5 + app.pan_offset.y,
    );
    let tf = PageTransform {
        page_origin_screen: page_origin,
        zoom: app.zoom,
    };

    let page_rect_pt = Rect::new(0.0, 0.0, app.doc.setup.width_pt, app.doc.setup.height_pt);
    let page_rect_screen = tf.rect_to_screen(page_rect_pt);

    // 1. Draw Pasteboard Drop Shadow & Optional Print Bleed Guide
    let shadow_rect = page_rect_screen.translate(egui::vec2(6.0, 8.0));
    painter.rect_filled(
        shadow_rect,
        2.0,
        egui::Color32::from_rgba_unmultiplied(0, 0, 0, 110),
    );

    if app.show_margins_and_bleed && app.doc.setup.bleed_pt > 0.0 {
        let bleed_pt = app.doc.setup.bleed_pt;
        let bleed_rect_pt = page_rect_pt.expand(bleed_pt);
        let bleed_screen = tf.rect_to_screen(bleed_rect_pt);
        draw_dashed_rect(
            &painter,
            bleed_screen,
            egui::Color32::from_rgb(235, 87, 87),
            1.2,
            6.0,
            4.0,
        );
        painter.text(
            egui::pos2(bleed_screen.left() + 4.0, bleed_screen.top() - 14.0),
            egui::Align2::LEFT_TOP,
            format!("Print Bleed ({:.1} pt)", bleed_pt),
            egui::FontId::proportional(10.0),
            egui::Color32::from_rgb(235, 110, 110),
        );
    }

    // 2. Draw Page Trim Paper Background
    let page_bg = app.doc.pages[app.current_page].background_color;
    painter.rect_filled(page_rect_screen, 0.0, to_color32(page_bg));
    painter.rect_stroke(
        page_rect_screen,
        0.0,
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(15, 18, 24)),
        egui::StrokeKind::Outside,
    );

    // 3. Draw Safe Margin Guides
    if app.show_margins_and_bleed {
        let safe_rect = app.doc.setup.safe_margin_rect(app.current_page);
        let safe_screen = tf.rect_to_screen(safe_rect);
        painter.rect_stroke(
            safe_screen,
            0.0,
            egui::Stroke::new(
                1.0_f32,
                egui::Color32::from_rgba_unmultiplied(110, 145, 240, 115),
            ),
            egui::StrokeKind::Inside,
        );
    }

    // 4. Draw Master Page Header & Page Number
    if let Some(master) = app.doc.master_pages.first() {
        let safe_rect = app.doc.setup.safe_margin_rect(app.current_page);
        if !master.header_text.is_empty() {
            let header_pos = tf.to_screen(Point::new(safe_rect.x, (safe_rect.y - 18.0).max(8.0)));
            painter.text(
                header_pos,
                egui::Align2::LEFT_TOP,
                &master.header_text,
                egui::FontId::proportional((9.5 * app.zoom).max(8.0)),
                egui::Color32::from_rgb(120, 128, 142),
            );
        }
        if master.show_page_numbers {
            let num_pos = tf.to_screen(Point::new(
                app.doc.setup.width_pt * 0.5,
                (app.doc.setup.height_pt - 22.0).max(10.0),
            ));
            painter.text(
                num_pos,
                egui::Align2::CENTER_TOP,
                format!("— {} —", app.current_page + 1),
                egui::FontId::proportional((10.0 * app.zoom).max(8.0)),
                egui::Color32::from_rgb(110, 118, 132),
            );
        }
    }

    // 5. Render All Frames in Z-Order
    let mut overflow_ports: Vec<(FrameId, egui::Rect)> = Vec::new();
    render_page_frames(
        &painter,
        &app.doc,
        &app.layout,
        app.current_page,
        tf,
        app.selected_frame_id,
        app.show_margins_and_bleed,
        &mut overflow_ports,
    );

    // 6. Render Text Threading Lines (when LinkText tool is active or Show Thread Lines is enabled)
    if app.show_thread_lines || app.active_tool == Tool::LinkText {
        render_text_thread_arrows(&painter, &app.doc, &app.layout, app.current_page, tf);
    }

    // 7. Render Active Link-Tool Rubber-Band Line
    if app.active_tool == Tool::LinkText {
        if let Some(src_id) = app.link_source_frame_id {
            if let Some(src_frame) = app.doc.pages[app.current_page]
                .frames
                .iter()
                .find(|f| f.id == src_id)
            {
                let start = tf.to_screen(src_frame.rect.center());
                if let Some(hover_pos) = response.hover_pos() {
                    painter.line_segment(
                        [start, hover_pos],
                        egui::Stroke::new(2.2_f32, egui::Color32::from_rgb(0, 210, 235)),
                    );
                    painter.circle_filled(hover_pos, 4.5, egui::Color32::from_rgb(0, 210, 235));
                }
            }
        }
    }

    // 8. Render Selection Box & 8 Resize Handles
    let mut handle_rects: Vec<(ResizeHandle, egui::Rect)> = Vec::new();
    if let Some(sel_id) = app.selected_frame_id {
        if let Some(sel_frame) = app.doc.pages[app.current_page]
            .frames
            .iter()
            .find(|f| f.id == sel_id)
        {
            let sel_screen = tf.rect_to_screen(sel_frame.rect);
            let accent = if sel_frame.locked {
                egui::Color32::from_rgb(230, 140, 60)
            } else {
                egui::Color32::from_rgb(56, 139, 253)
            };
            painter.rect_stroke(
                sel_screen,
                0.0,
                egui::Stroke::new(1.8_f32, accent),
                egui::StrokeKind::Outside,
            );

            // Show wrap padding boundary if wrap mode is active
            if sel_frame.wrap.mode != WrapMode::None {
                let wrap_rect = tf.rect_to_screen(sel_frame.rect.expand(sel_frame.wrap.padding_pt));
                draw_dashed_rect(
                    &painter,
                    wrap_rect,
                    egui::Color32::from_rgba_unmultiplied(245, 175, 45, 190),
                    1.2,
                    4.0,
                    3.0,
                );
            }

            if !sel_frame.locked {
                for handle in ResizeHandle::ALL {
                    let pos_pt = handle.position_pt(sel_frame.rect);
                    let pos_screen = tf.to_screen(pos_pt);
                    let h_rect = egui::Rect::from_center_size(pos_screen, egui::vec2(8.0, 8.0));
                    painter.rect_filled(h_rect, 1.5, egui::Color32::WHITE);
                    painter.rect_stroke(
                        h_rect,
                        1.5,
                        egui::Stroke::new(1.5_f32, accent),
                        egui::StrokeKind::Outside,
                    );
                    handle_rects.push((handle, h_rect.expand(4.0)));
                }
            }
        }
    }

    // 9. Render Active Smart Alignment Snap Guides
    for guide in &app.canvas_interaction.active_guides {
        match *guide {
            SmartGuideLine::Vertical {
                x_pt,
                y_start_pt,
                y_end_pt,
                is_page_guide,
            } => {
                let color = if is_page_guide {
                    egui::Color32::from_rgb(236, 72, 153) // Magenta for page center/margins
                } else {
                    egui::Color32::from_rgb(6, 182, 212) // Cyan for frame-to-frame alignment
                };
                let p1 = tf.to_screen(Point::new(x_pt, y_start_pt));
                let p2 = tf.to_screen(Point::new(x_pt, y_end_pt));
                painter.line_segment([p1, p2], egui::Stroke::new(1.5_f32, color));
            }
            SmartGuideLine::Horizontal {
                y_pt,
                x_start_pt,
                x_end_pt,
                is_page_guide,
            } => {
                let color = if is_page_guide {
                    egui::Color32::from_rgb(236, 72, 153)
                } else {
                    egui::Color32::from_rgb(6, 182, 212)
                };
                let p1 = tf.to_screen(Point::new(x_start_pt, y_pt));
                let p2 = tf.to_screen(Point::new(x_end_pt, y_pt));
                painter.line_segment([p1, p2], egui::Stroke::new(1.5_f32, color));
            }
        }
    }

    // 10. Render Rubber-Band Box when Drawing New Frame
    if let DragMode::DrawingNewFrame {
        start_pt,
        current_pt,
        ..
    } = app.canvas_interaction.drag_mode
    {
        let min_x = start_pt.x.min(current_pt.x);
        let min_y = start_pt.y.min(current_pt.y);
        let w = (start_pt.x - current_pt.x).abs();
        let h = (start_pt.y - current_pt.y).abs();
        let preview_screen = tf.rect_to_screen(Rect::new(min_x, min_y, w, h));
        painter.rect_filled(
            preview_screen,
            2.0,
            egui::Color32::from_rgba_unmultiplied(56, 139, 253, 38),
        );
        painter.rect_stroke(
            preview_screen,
            2.0,
            egui::Stroke::new(1.5_f32, egui::Color32::from_rgb(56, 139, 253)),
            egui::StrokeKind::Outside,
        );
    }

    // 11. Process Pointer Interactions (Click, Double-Click, Drag, Resize, Link)
    handle_canvas_pointer_interactions(
        app,
        ui,
        &response,
        tf,
        &handle_rects,
        &overflow_ports,
    );

    // 12. Direct On-Canvas Text Editing Overlay (when a text frame is double-clicked)
    show_inline_canvas_text_editor(app, ui, tf);

    // 13. Bottom-Left Canvas HUD (Page Dimensions, Active Tool Tip, Link Status)
    draw_canvas_hud(&painter, available, app);
}

fn render_page_frames(
    painter: &egui::Painter,
    doc: &BookDocument,
    layout: &ComputedBookLayout,
    page_idx: usize,
    tf: PageTransform,
    selected_frame_id: Option<FrameId>,
    show_guides: bool,
    overflow_ports: &mut Vec<(FrameId, egui::Rect)>,
) {
    let page = &doc.pages[page_idx];
    for frame in page.frames_sorted_by_z() {
        let screen_rect = tf.rect_to_screen(frame.rect);
        match &frame.kind {
            FrameKind::Shape {
                kind,
                fill,
                stroke_color,
                stroke_width_pt,
                corner_radius_pt,
            } => {
                let fill_c = to_color32(*fill);
                let stroke_c = to_color32(*stroke_color);
                let stroke_w = (*stroke_width_pt * tf.zoom).max(0.0);
                match kind {
                    ShapeKind::Rectangle => {
                        painter.rect_filled(screen_rect, 0.0, fill_c);
                        if stroke_w > 0.0 {
                            painter.rect_stroke(
                                screen_rect,
                                0.0,
                                egui::Stroke::new(stroke_w, stroke_c),
                                egui::StrokeKind::Inside,
                            );
                        }
                    }
                    ShapeKind::RoundedRectangle => {
                        let radius = (*corner_radius_pt * tf.zoom).min(screen_rect.width() * 0.5);
                        painter.rect_filled(screen_rect, radius, fill_c);
                        if stroke_w > 0.0 {
                            painter.rect_stroke(
                                screen_rect,
                                radius,
                                egui::Stroke::new(stroke_w, stroke_c),
                                egui::StrokeKind::Inside,
                            );
                        }
                    }
                    ShapeKind::Ellipse => {
                        let center = screen_rect.center();
                        let rx = screen_rect.width() * 0.5;
                        let ry = screen_rect.height() * 0.5;
                        let points: Vec<egui::Pos2> = (0..48)
                            .map(|i| {
                                let angle = (i as f32 / 48.0) * std::f32::consts::TAU;
                                egui::pos2(center.x + rx * angle.cos(), center.y + ry * angle.sin())
                            })
                            .collect();
                        painter.add(egui::Shape::convex_polygon(
                            points,
                            fill_c,
                            egui::Stroke::new(stroke_w, stroke_c),
                        ));
                    }
                    ShapeKind::DividerLine => {
                        let y = screen_rect.center().y;
                        painter.line_segment(
                            [
                                egui::pos2(screen_rect.left(), y),
                                egui::pos2(screen_rect.right(), y),
                            ],
                            egui::Stroke::new((stroke_w).max(2.0 * tf.zoom), fill_c),
                        );
                    }
                }
            }
            FrameKind::Image {
                asset_id,
                fit,
                caption,
                corner_radius_pt,
            } => {
                let radius = (*corner_radius_pt * tf.zoom).min(24.0);
                let has_asset = asset_id
                    .as_ref()
                    .map_or(false, |id| doc.assets.contains_key(id));
                let bg = if has_asset {
                    egui::Color32::from_rgb(225, 236, 248)
                } else {
                    egui::Color32::from_rgb(236, 240, 246)
                };
                painter.rect_filled(screen_rect, radius, bg);
                painter.rect_stroke(
                    screen_rect,
                    radius,
                    egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(160, 175, 198)),
                    egui::StrokeKind::Inside,
                );

                // Subtle diagonal placeholder crosslines when no asset is loaded
                if !has_asset {
                    painter.line_segment(
                        [screen_rect.left_top(), screen_rect.right_bottom()],
                        egui::Stroke::new(
                            1.0_f32,
                            egui::Color32::from_rgba_unmultiplied(160, 175, 198, 90),
                        ),
                    );
                    painter.line_segment(
                        [screen_rect.right_top(), screen_rect.left_bottom()],
                        egui::Stroke::new(
                            1.0_f32,
                            egui::Color32::from_rgba_unmultiplied(160, 175, 198, 90),
                        ),
                    );
                }

                let fit_badge = match fit {
                    ImageFit::Cover => "Cover",
                    ImageFit::Contain => "Contain",
                    ImageFit::Stretch => "Stretch",
                };
                let label_text = if let Some(ref id) = asset_id {
                    if let Some(entry) = doc.assets.get(id) {
                        let dims = entry
                            .pixel_dimensions
                            .map(|(w, h)| format!(" ({w}×{h}px)"))
                            .unwrap_or_default();
                        format!("🖼 {}{} [{}]", entry.original_filename, dims, fit_badge)
                    } else {
                        format!("⚠ Missing Asset: {id}")
                    }
                } else if !caption.is_empty() {
                    format!("🖼 {caption}")
                } else {
                    "🖼 Image Frame (Drop PNG/JPG/SVG or Import in Inspector)".to_string()
                };

                let pill_rect = egui::Rect::from_center_size(
                    screen_rect.center(),
                    egui::vec2((screen_rect.width() - 16.0).max(40.0), 34.0 * tf.zoom.min(1.2)),
                );
                painter.rect_filled(
                    pill_rect,
                    4.0,
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 220),
                );
                painter.text(
                    pill_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    label_text,
                    egui::FontId::proportional((11.0 * tf.zoom).clamp(9.0, 15.0)),
                    egui::Color32::from_rgb(40, 52, 72),
                );
            }
            FrameKind::Text { background, .. } => {
                if let Some(bg) = background {
                    if bg.a > 0 {
                        painter.rect_filled(screen_rect, 0.0, to_color32(*bg));
                    }
                }
                if show_guides && selected_frame_id != Some(frame.id) {
                    painter.rect_stroke(
                        screen_rect,
                        0.0,
                        egui::Stroke::new(
                            1.0_f32,
                            egui::Color32::from_rgba_unmultiplied(120, 145, 180, 95),
                        ),
                        egui::StrokeKind::Inside,
                    );
                }

                // Render positioned text lines from ComputedBookLayout
                if let Some(page_layout) = layout.pages.get(page_idx) {
                    if let Some(computed_tf) = page_layout.text_frames.get(&frame.id) {
                        let clip_painter = painter.with_clip_rect(screen_rect);
                        for line in &computed_tf.lines {
                            let font_px = (line.font_size_pt * tf.zoom).max(6.0);
                            let font_id = if line.font_family.to_lowercase().contains("mono") {
                                egui::FontId::monospace(font_px)
                            } else {
                                egui::FontId::proportional(font_px)
                            };
                            let text_color = to_color32(line.color);
                            let line_screen_y = tf.page_origin_screen.y + line.y_pt * tf.zoom;
                            let seg_left_x = tf.page_origin_screen.x + line.x_pt * tf.zoom;
                            let seg_w_screen = line.width_pt * tf.zoom;

                            let (pos, align) = match line.alignment {
                                TextAlignment::Left | TextAlignment::Justified => (
                                    egui::pos2(seg_left_x, line_screen_y),
                                    egui::Align2::LEFT_TOP,
                                ),
                                TextAlignment::Center => (
                                    egui::pos2(seg_left_x + seg_w_screen * 0.5, line_screen_y),
                                    egui::Align2::CENTER_TOP,
                                ),
                                TextAlignment::Right => (
                                    egui::pos2(seg_left_x + seg_w_screen, line_screen_y),
                                    egui::Align2::RIGHT_TOP,
                                ),
                            };

                            clip_painter.text(pos, align, &line.text, font_id, text_color);
                        }

                        // Draw Red `[+]` Overflow Port if story overflows this text frame
                        if computed_tf.is_overflowing {
                            let port_center = egui::pos2(
                                screen_rect.right() - 10.0,
                                screen_rect.bottom() - 10.0,
                            );
                            let port_rect =
                                egui::Rect::from_center_size(port_center, egui::vec2(18.0, 18.0));
                            painter.rect_filled(
                                port_rect,
                                3.0,
                                egui::Color32::from_rgb(220, 38, 38),
                            );
                            painter.rect_stroke(
                                port_rect,
                                3.0,
                                egui::Stroke::new(1.2_f32, egui::Color32::WHITE),
                                egui::StrokeKind::Outside,
                            );
                            painter.text(
                                port_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "+",
                                egui::FontId::monospace(13.0),
                                egui::Color32::WHITE,
                            );
                            overflow_ports.push((frame.id, port_rect.expand(4.0)));
                        }
                    }
                }
            }
        }
    }
}

fn render_text_thread_arrows(
    painter: &egui::Painter,
    doc: &BookDocument,
    layout: &ComputedBookLayout,
    page_idx: usize,
    tf: PageTransform,
) {
    let Some(page_layout) = layout.pages.get(page_idx) else {
        return;
    };
    let page = &doc.pages[page_idx];
    let thread_color = egui::Color32::from_rgb(6, 182, 212);

    for frame in &page.frames {
        if let Some(computed) = page_layout.text_frames.get(&frame.id) {
            if let Some((next_page_idx, next_frame_id, _)) = computed.next_linked_frame {
                let src_screen = tf.rect_to_screen(frame.rect);
                let src_port = egui::pos2(src_screen.right() - 8.0, src_screen.bottom() - 24.0);
                painter.circle_filled(src_port, 4.5, thread_color);

                if next_page_idx == page_idx {
                    if let Some(dst_frame) = page.frames.iter().find(|f| f.id == next_frame_id) {
                        let dst_screen = tf.rect_to_screen(dst_frame.rect);
                        let dst_port = egui::pos2(dst_screen.left() + 8.0, dst_screen.top() + 18.0);
                        painter.line_segment(
                            [src_port, dst_port],
                            egui::Stroke::new(2.0_f32, thread_color),
                        );
                        draw_arrowhead(painter, src_port, dst_port, thread_color);
                    }
                } else {
                    // Cross-page thread indicator badge
                    let badge_pos = egui::pos2(src_port.x - 42.0, src_port.y - 10.0);
                    let badge_rect =
                        egui::Rect::from_min_size(badge_pos, egui::vec2(76.0, 18.0));
                    painter.rect_filled(badge_rect, 4.0, thread_color);
                    painter.text(
                        badge_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        format!("↪ Page {}", next_page_idx + 1),
                        egui::FontId::proportional(10.5),
                        egui::Color32::from_rgb(12, 24, 36),
                    );
                }
            }
        }
    }
}

fn draw_arrowhead(
    painter: &egui::Painter,
    from: egui::Pos2,
    to: egui::Pos2,
    color: egui::Color32,
) {
    let dir = (to - from).normalized();
    if dir.length_sq() < 0.1 {
        return;
    }
    let perp = egui::vec2(-dir.y, dir.x);
    let tip = to;
    let p1 = tip - dir * 9.0 + perp * 4.5;
    let p2 = tip - dir * 9.0 - perp * 4.5;
    painter.add(egui::Shape::convex_polygon(
        vec![tip, p1, p2],
        color,
        egui::Stroke::NONE,
    ));
}

fn handle_canvas_pointer_interactions(
    app: &mut BookCreatorApp,
    ui: &egui::Ui,
    response: &egui::Response,
    tf: PageTransform,
    handle_rects: &[(ResizeHandle, egui::Rect)],
    overflow_ports: &[(FrameId, egui::Rect)],
) {
    // Update cursor icon when hovering resize handles or overflow ports
    if let Some(hover_pos) = response.hover_pos() {
        for &(handle, h_rect) in handle_rects {
            if h_rect.contains(hover_pos) {
                ui.ctx().set_cursor_icon(handle.cursor_icon());
                break;
            }
        }
        for &(_, port_rect) in overflow_ports {
            if port_rect.contains(hover_pos) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                break;
            }
        }
    }

    // Double-click on a text frame -> open Direct On-Canvas Text Editor
    if response.double_clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let pt = tf.to_page_pt(pos);
            if let Some(hit_id) = hit_test_frame(&app.doc.pages[app.current_page], pt) {
                if let Some(frame) = app.doc.pages[app.current_page]
                    .frames
                    .iter()
                    .find(|f| f.id == hit_id)
                {
                    if matches!(frame.kind, FrameKind::Text { .. }) {
                        app.selected_frame_id = Some(hit_id);
                        app.editing_text_frame_id = Some(hit_id);
                        return;
                    }
                }
            }
        }
    }

    // Single click handling (overflow `[+]` port, LinkText tool, or frame selection)
    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            // Check if user clicked a red `[+]` overflow port directly
            for &(overflow_frame_id, port_rect) in overflow_ports {
                if port_rect.contains(pos) {
                    app.active_tool = Tool::LinkText;
                    app.link_source_frame_id = Some(overflow_frame_id);
                    app.selected_frame_id = Some(overflow_frame_id);
                    app.status_message =
                        "Link Mode: Click another Text Frame (on this page or another page) to thread overflow text.".to_string();
                    return;
                }
            }

            let pt = tf.to_page_pt(pos);
            let hit = hit_test_frame(&app.doc.pages[app.current_page], pt);

            if app.active_tool == Tool::LinkText {
                if let Some(clicked_id) = hit {
                    app.handle_link_tool_click(clicked_id);
                } else {
                    app.link_source_frame_id = None;
                }
                return;
            }

            app.selected_frame_id = hit;
            if hit != app.editing_text_frame_id {
                app.editing_text_frame_id = None;
            }
        }
    }

    // Drag start
    if response.drag_started_by(egui::PointerButton::Primary)
        && !ui.input(|i| i.key_down(egui::Key::Space))
    {
        if let Some(pos) = response.interact_pointer_pos() {
            let pt = tf.to_page_pt(pos);

            match app.active_tool {
                Tool::TextFrame | Tool::ImageFrame | Tool::ShapeFrame => {
                    app.canvas_interaction.drag_mode = DragMode::DrawingNewFrame {
                        tool: app.active_tool,
                        start_pt: pt,
                        current_pt: pt,
                    };
                }
                Tool::SelectMove => {
                    // 1. Check resize handles on currently selected frame first
                    let mut started_resize = None;
                    if let Some(sel_id) = app.selected_frame_id {
                        for &(handle, h_rect) in handle_rects {
                            if h_rect.contains(pos) {
                                if let Some(frame) = app.doc.pages[app.current_page]
                                    .frames
                                    .iter()
                                    .find(|f| f.id == sel_id)
                                {
                                    started_resize = Some((sel_id, handle, frame.rect));
                                }
                                break;
                            }
                        }
                    }

                    if let Some((frame_id, handle, initial_rect)) = started_resize {
                        app.canvas_interaction.drag_mode = DragMode::ResizingFrame {
                            frame_id,
                            handle,
                            start_pointer_pt: pt,
                            initial_rect,
                            snapshot_recorded: false,
                        };
                    } else if let Some(hit_id) =
                        hit_test_frame(&app.doc.pages[app.current_page], pt)
                    {
                        app.selected_frame_id = Some(hit_id);
                        if let Some(frame) = app.doc.pages[app.current_page]
                            .frames
                            .iter()
                            .find(|f| f.id == hit_id)
                        {
                            if !frame.locked {
                                app.canvas_interaction.drag_mode = DragMode::MovingFrame {
                                    frame_id: hit_id,
                                    start_pointer_pt: pt,
                                    initial_rect: frame.rect,
                                    snapshot_recorded: false,
                                };
                            }
                        }
                    } else {
                        app.selected_frame_id = None;
                        app.editing_text_frame_id = None;
                    }
                }
                Tool::LinkText => {}
            }
        }
    }

    // Active primary drag update
    if response.dragged_by(egui::PointerButton::Primary)
        && !ui.input(|i| i.key_down(egui::Key::Space))
    {
        if let Some(pos) = response.interact_pointer_pos() {
            let pt = tf.to_page_pt(pos);
            let mut needs_layout_update = false;

            match app.canvas_interaction.drag_mode.clone() {
                DragMode::MovingFrame {
                    frame_id,
                    start_pointer_pt,
                    initial_rect,
                    mut snapshot_recorded,
                } => {
                    let dx = pt.x - start_pointer_pt.x;
                    let dy = pt.y - start_pointer_pt.y;
                    if (dx.abs() > 0.5 || dy.abs() > 0.5) && !snapshot_recorded {
                        app.record_undo_snapshot();
                        snapshot_recorded = true;
                        app.canvas_interaction.drag_mode = DragMode::MovingFrame {
                            frame_id,
                            start_pointer_pt,
                            initial_rect,
                            snapshot_recorded,
                        };
                    }

                    let candidate = Rect::new(
                        initial_rect.x + dx,
                        initial_rect.y + dy,
                        initial_rect.width,
                        initial_rect.height,
                    );
                    let (snapped_rect, guides) = compute_smart_snap(
                        &app.doc.pages[app.current_page],
                        &app.doc.setup,
                        app.current_page,
                        frame_id,
                        candidate,
                        5.0,
                    );
                    app.canvas_interaction.active_guides = guides;

                    if let Some(frame) = app.doc.pages[app.current_page]
                        .frames
                        .iter_mut()
                        .find(|f| f.id == frame_id)
                    {
                        frame.rect = snapped_rect;
                        needs_layout_update = true;
                    }
                }
                DragMode::ResizingFrame {
                    frame_id,
                    handle,
                    start_pointer_pt,
                    initial_rect,
                    mut snapshot_recorded,
                } => {
                    let dx = pt.x - start_pointer_pt.x;
                    let dy = pt.y - start_pointer_pt.y;
                    if (dx.abs() > 0.5 || dy.abs() > 0.5) && !snapshot_recorded {
                        app.record_undo_snapshot();
                        snapshot_recorded = true;
                        app.canvas_interaction.drag_mode = DragMode::ResizingFrame {
                            frame_id,
                            handle,
                            start_pointer_pt,
                            initial_rect,
                            snapshot_recorded,
                        };
                    }
                    let new_rect = handle.apply_delta(initial_rect, dx, dy, 24.0);
                    if let Some(frame) = app.doc.pages[app.current_page]
                        .frames
                        .iter_mut()
                        .find(|f| f.id == frame_id)
                    {
                        frame.rect = new_rect;
                        needs_layout_update = true;
                    }
                }
                DragMode::DrawingNewFrame {
                    tool,
                    start_pt,
                    ..
                } => {
                    app.canvas_interaction.drag_mode = DragMode::DrawingNewFrame {
                        tool,
                        start_pt,
                        current_pt: pt,
                    };
                }
                DragMode::None => {}
            }

            if needs_layout_update {
                app.recompute_layout_and_preflight();
            }
        }
    }

    // Drag release
    if response.drag_stopped_by(egui::PointerButton::Primary) {
        if let DragMode::DrawingNewFrame {
            tool,
            start_pt,
            current_pt,
        } = app.canvas_interaction.drag_mode
        {
            let min_x = start_pt.x.min(current_pt.x);
            let min_y = start_pt.y.min(current_pt.y);
            let w = (start_pt.x - current_pt.x).abs().max(90.0);
            let h = (start_pt.y - current_pt.y).abs().max(60.0);
            let new_rect = Rect::new(min_x, min_y, w, h);
            let page_idx = app.current_page;
            app.create_frame_from_drag(page_idx, tool, new_rect);
            app.active_tool = Tool::SelectMove;
        }
        app.canvas_interaction.drag_mode = DragMode::None;
        app.canvas_interaction.active_guides.clear();
    }
}

fn show_inline_canvas_text_editor(
    app: &mut BookCreatorApp,
    ui: &mut egui::Ui,
    tf: PageTransform,
) {
    let Some(edit_frame_id) = app.editing_text_frame_id else {
        return;
    };
    let Some(frame) = app
        .doc
        .pages
        .get(app.current_page)
        .and_then(|p| p.frames.iter().find(|f| f.id == edit_frame_id))
        .cloned()
    else {
        app.editing_text_frame_id = None;
        return;
    };
    let FrameKind::Text { story_id, .. } = frame.kind else {
        app.editing_text_frame_id = None;
        return;
    };

    let screen_rect = tf.rect_to_screen(frame.rect);
    let editor_pos = egui::pos2(screen_rect.left(), (screen_rect.top() - 32.0).max(44.0));

    let mut close_editor = false;
    let mut story_changed = false;

    egui::Window::new("✏ Direct On-Canvas Story Editor")
        .fixed_pos(editor_pos)
        .default_size(egui::vec2(
            screen_rect.width().clamp(280.0, 520.0),
            screen_rect.height().clamp(160.0, 360.0),
        ))
        .collapsible(false)
        .show(ui.ctx(), |ui| {
            if let Some(story) = app.doc.stories.get_mut(&story_id) {
                ui.horizontal(|ui| {
                    ui.label("Story:");
                    ui.text_edit_singleline(&mut story.title);
                    if ui.button("✓ Done (Esc)").clicked() {
                        close_editor = true;
                    }
                });
                ui.separator();
                let edit_resp = ui.add_sized(
                    egui::vec2(ui.available_width(), (screen_rect.height() - 24.0).clamp(110.0, 260.0)),
                    egui::TextEdit::multiline(&mut story.text)
                        .font(egui::TextStyle::Body)
                        .desired_width(f32::INFINITY),
                );
                if edit_resp.changed() {
                    story_changed = true;
                }
            }
        });

    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        close_editor = true;
    }
    if story_changed {
        app.recompute_layout_and_preflight();
    }
    if close_editor {
        app.editing_text_frame_id = None;
    }
}

fn draw_canvas_hud(painter: &egui::Painter, available: egui::Rect, app: &BookCreatorApp) {
    let hud_rect = egui::Rect::from_min_size(
        egui::pos2(available.left() + 12.0, available.bottom() - 34.0),
        egui::vec2((available.width() - 24.0).min(780.0), 24.0),
    );
    painter.rect_filled(
        hud_rect,
        5.0,
        egui::Color32::from_rgba_unmultiplied(18, 20, 26, 215),
    );

    let tool_hint = match app.active_tool {
        Tool::SelectMove => "Select/Move (V): Click frame to select, drag to move (with Smart Snap), double-click text to edit inline",
        Tool::TextFrame => "Text Frame (T): Click and drag on the page to create a new Story Text Frame",
        Tool::ImageFrame => "Image Frame (I): Click and drag to create an Image Frame (or drop PNG/JPG/SVG from OS)",
        Tool::ShapeFrame => "Shape Frame (S): Click and drag on the page to draw a vector shape",
        Tool::LinkText => {
            if app.link_source_frame_id.is_some() {
                "Link Text (L): Source selected! Now click destination Text Frame (on any page) to thread story"
            } else {
                "Link Text (L): Click source Text Frame first, then click target Text Frame to link"
            }
        }
    };

    let status = if app.status_message.is_empty() {
        format!(
            "Page {}/{} ({:.0}×{:.0} pt)  •  {}",
            app.current_page + 1,
            app.doc.pages.len(),
            app.doc.setup.width_pt,
            app.doc.setup.height_pt,
            tool_hint
        )
    } else {
        format!(
            "{}  |  Page {}/{} ({:.0}×{:.0} pt)",
            app.status_message,
            app.current_page + 1,
            app.doc.pages.len(),
            app.doc.setup.width_pt,
            app.doc.setup.height_pt
        )
    };

    painter.text(
        egui::pos2(hud_rect.left() + 10.0, hud_rect.center().y),
        egui::Align2::LEFT_CENTER,
        status,
        egui::FontId::proportional(11.5),
        egui::Color32::from_rgb(210, 218, 230),
    );
}

fn draw_dashed_rect(
    painter: &egui::Painter,
    rect: egui::Rect,
    color: egui::Color32,
    stroke_width: f32,
    dash_len: f32,
    gap_len: f32,
) {
    let corners = [
        (rect.left_top(), rect.right_top()),
        (rect.right_top(), rect.right_bottom()),
        (rect.right_bottom(), rect.left_bottom()),
        (rect.left_bottom(), rect.left_top()),
    ];
    let stroke = egui::Stroke::new(stroke_width, color);
    for (start, end) in corners {
        let vec = end - start;
        let total_len = vec.length();
        if total_len <= 0.1 {
            continue;
        }
        let dir = vec / total_len;
        let mut t = 0.0;
        while t < total_len {
            let seg_end = (t + dash_len).min(total_len);
            painter.line_segment([start + dir * t, start + dir * seg_end], stroke);
            t += dash_len + gap_len;
        }
    }
}
