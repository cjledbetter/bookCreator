//! Deterministic typography, multi-frame story threading, smart alignment snapping,
//! hit-testing, and live obstacle text-wrap ("runaround") layout engine shared by
//! the Canvas, PDF, and EPUB exporters.

use book_core::{
    BookDocument, ColorRgba, FrameId, Page, PageId, PageSetup, Point, Rect, TextAlignment,
    TextStyle, WrapMode,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A single positioned line segment inside a text frame after obstacle text-wrap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PositionedTextLine {
    /// Baseline or top-left X in page points (`pt`).
    pub x_pt: f32,
    /// Top Y in page points (`pt`).
    pub y_pt: f32,
    /// Available segment width in points (`pt`).
    pub width_pt: f32,
    /// Line height in points (`pt`).
    pub height_pt: f32,
    pub text: String,
    pub font_family: String,
    pub font_size_pt: f32,
    pub bold: bool,
    pub italic: bool,
    pub color: ColorRgba,
    pub alignment: TextAlignment,
}

/// Computed layout for a single text frame, including overflow state for `[+]` indicator.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ComputedTextFrame {
    pub frame_id: FrameId,
    pub lines: Vec<PositionedTextLine>,
    /// True if this is the last frame in a story's thread chain and unplaced words remain.
    pub is_overflowing: bool,
    /// Number of remaining characters that did not fit in this thread chain.
    pub overflow_char_count: usize,
    /// Next frame in the thread chain (if linked).
    pub next_linked_frame: Option<(usize, FrameId, Point)>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ComputedPageLayout {
    pub page_id: PageId,
    pub page_index: usize,
    pub text_frames: BTreeMap<FrameId, ComputedTextFrame>,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ComputedBookLayout {
    pub pages: Vec<ComputedPageLayout>,
}

/// Orientation axis of a smart alignment guide line on the canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapAxis {
    /// A vertical guide line at a fixed `x` coordinate (`start_pt..=end_pt` along Y).
    Vertical,
    /// A horizontal guide line at a fixed `y` coordinate (`start_pt..=end_pt` along X).
    Horizontal,
}

/// Classification of the page feature that triggered a smart alignment snap guide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapTargetKind {
    /// Page horizontal or vertical center line.
    PageCenter,
    /// Safe margin boundary (top, bottom, inside, outside).
    SafeMargin,
    /// Page trim edge.
    PageEdge,
    /// Another frame's left, right, top, or bottom edge.
    FrameEdge,
    /// Another frame's center X or center Y.
    FrameCenter,
}

/// Visual smart-alignment guide line produced when moving or resizing a frame on the canvas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapGuide {
    pub axis: SnapAxis,
    /// Coordinate in points (`x` for `Vertical`, `y` for `Horizontal`).
    pub position_pt: f32,
    /// Start coordinate along the perpendicular axis in points.
    pub start_pt: f32,
    /// End coordinate along the perpendicular axis in points.
    pub end_pt: f32,
    /// What target kind caused this guide to appear.
    pub kind: SnapTargetKind,
    /// Human-readable description of the alignment guide.
    pub label: String,
}

/// 8-point resize handles around a selected frame's bounding box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResizeHandle {
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    MiddleRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

/// Returns true if `font_family` refers to a fixed-width monospace family.
pub fn is_monospace_family(font_family: &str) -> bool {
    let lower = font_family.trim().to_ascii_lowercase();
    lower.contains("mono")
        || lower.contains("courier")
        || lower.contains("code")
        || lower.contains("consolas")
}

/// Returns the relative horizontal advance multiplier for a font family compared to neutral sans-serif.
pub fn font_family_width_factor(font_family: &str) -> f32 {
    if is_monospace_family(font_family) {
        return 1.0;
    }
    let lower = font_family.trim().to_ascii_lowercase();
    if lower.contains("literata")
        || lower.contains("times")
        || lower.contains("georgia")
        || lower.contains("garamond")
        || (lower.contains("serif") && !lower.contains("sans"))
    {
        0.96
    } else if lower.contains("inter")
        || lower.contains("helvetica")
        || lower.contains("arial")
        || lower.contains("sans")
    {
        1.00
    } else {
        0.98
    }
}

/// Estimates the typographic advance width in points (`pt`) of a single character
/// using proportional ASCII/Unicode character-class metrics and style multipliers.
pub fn estimate_char_width_pt(
    ch: char,
    font_size_pt: f32,
    font_family: &str,
    bold: bool,
    italic: bool,
) -> f32 {
    let size = font_size_pt.max(1.0);
    if matches!(ch, '\n' | '\r' | '\0') {
        return 0.0;
    }

    let base_em = if is_monospace_family(font_family) {
        if ch == '\t' {
            2.40
        } else {
            0.60
        }
    } else {
        let raw_em = match ch {
            ' ' => 0.27,
            '\t' => 1.08,
            'i' | 'l' | '!' | '|' | '.' | ',' | ':' | ';' | '\'' | '`' => 0.26,
            'I' | 'j' | 'f' | 't' | 'r' | '(' | ')' | '[' | ']' | '{' | '}' | '"' | '-' | '/'
            | '\\' => 0.34,
            's' | 'c' | 'z' | 'v' | 'x' | 'y' | 'k' | 'J' | 'L' | '?' | '*' | '+' | '=' | '~'
            | '^' => 0.46,
            'a' | 'b' | 'd' | 'e' | 'g' | 'h' | 'n' | 'o' | 'p' | 'q' | 'u' | '0'..='9' | '$'
            | '#' | '_' => 0.52,
            'A'..='H' | 'K' | 'N'..='V' | 'X'..='Z' => 0.66,
            'm' | 'w' => 0.76,
            'M' | 'W' | '%' | '&' | '@' | '—' | '–' | '…' => 0.86,
            c if ('\u{00A0}'..='\u{024F}').contains(&c) => 0.52,
            c if ('\u{2000}'..='\u{206F}').contains(&c) => 0.50,
            c if ('\u{2E80}'..='\u{9FFF}').contains(&c)
                || ('\u{AC00}'..='\u{D7AF}').contains(&c)
                || ('\u{1F000}'..='\u{1FAFF}').contains(&c) =>
            {
                1.00
            }
            _ => 0.56,
        };
        raw_em * font_family_width_factor(font_family)
    };

    let weight_factor = if bold { 1.06 } else { 1.0 };
    let slant_factor = if italic { 1.015 } else { 1.0 };
    size * base_em * weight_factor * slant_factor
}

/// Measures the estimated advance width in points (`pt`) of a string slice for the given `TextStyle`.
pub fn measure_text_width_pt(text: &str, style: &TextStyle) -> f32 {
    text.chars()
        .map(|ch| {
            estimate_char_width_pt(
                ch,
                style.font_size_pt,
                &style.font_family,
                style.bold,
                style.italic,
            )
        })
        .sum()
}

/// Computes the horizontal X offset in points (`pt`) within a line segment of `segment_width_pt`
/// for a rendered line of width `text_width_pt` according to `TextAlignment`.
pub fn compute_alignment_offset_pt(
    alignment: TextAlignment,
    segment_width_pt: f32,
    text_width_pt: f32,
) -> f32 {
    let slack = (segment_width_pt - text_width_pt).max(0.0);
    match alignment {
        TextAlignment::Left | TextAlignment::Justified => 0.0,
        TextAlignment::Center => slack * 0.5,
        TextAlignment::Right => slack,
    }
}

/// Computes extra inter-word spacing in points (`pt`) for `TextAlignment::Justified`
/// on non-final lines of a paragraph.
pub fn compute_justified_word_spacing_pt(
    alignment: TextAlignment,
    is_last_line_in_paragraph: bool,
    segment_width_pt: f32,
    text_width_pt: f32,
    word_count: usize,
) -> f32 {
    if alignment != TextAlignment::Justified || is_last_line_in_paragraph || word_count <= 1 {
        return 0.0;
    }
    let gaps = (word_count - 1) as f32;
    ((segment_width_pt - text_width_pt).max(0.0)) / gaps
}

impl ComputedBookLayout {
    pub fn compute(doc: &BookDocument) -> Self {
        let mut pages: Vec<ComputedPageLayout> = doc
            .pages
            .iter()
            .enumerate()
            .map(|(idx, p)| ComputedPageLayout {
                page_id: p.id,
                page_index: idx,
                text_frames: BTreeMap::new(),
            })
            .collect();

        // Collect obstacle frames per page (any frame with WrapMode != None)
        let obstacles_per_page: Vec<Vec<(FrameId, Rect, WrapMode)>> = doc
            .pages
            .iter()
            .map(|page| {
                page.frames
                    .iter()
                    .filter(|f| f.wrap.mode != WrapMode::None)
                    .map(|f| (f.id, f.rect.expand(f.wrap.padding_pt.max(0.0)), f.wrap.mode))
                    .collect()
            })
            .collect();

        for (&story_id, story) in &doc.stories {
            let chain = doc.threaded_frames_for_story(story_id);
            if chain.is_empty() {
                continue;
            }
            flow_story_into_chain(doc, story, &chain, &obstacles_per_page, &mut pages);
        }

        // Ensure any text frame referencing an unmapped story still gets a clean entry
        for (page_idx, page) in doc.pages.iter().enumerate() {
            for frame in &page.frames {
                if matches!(frame.kind, book_core::FrameKind::Text { .. }) {
                    if let Some(page_layout) = pages.get_mut(page_idx) {
                        page_layout
                            .text_frames
                            .entry(frame.id)
                            .or_insert_with(|| ComputedTextFrame {
                                frame_id: frame.id,
                                ..Default::default()
                            });
                    }
                }
            }
        }

        Self { pages }
    }
}

pub fn flow_story_into_chain(
    doc: &BookDocument,
    story: &book_core::Story,
    chain: &[(usize, FrameId, Rect)],
    obstacles_per_page: &[Vec<(FrameId, Rect, WrapMode)>],
    pages: &mut [ComputedPageLayout],
) {
    let line_height = (story.style.font_size_pt * story.style.line_height_factor).max(8.0);
    let space_w = estimate_char_width_pt(
        ' ',
        story.style.font_size_pt,
        &story.style.font_family,
        story.style.bold,
        story.style.italic,
    )
    .max(2.0);

    // Tokenize paragraphs into words while preserving explicit line breaks
    let paragraphs: Vec<Vec<&str>> = story
        .text
        .split('\n')
        .map(|para| para.split_whitespace().collect())
        .collect();

    let mut para_idx = 0;
    let mut word_idx = 0;

    for (chain_pos, &(page_idx, frame_id, frame_rect)) in chain.iter().enumerate() {
        let padding = doc
            .pages
            .get(page_idx)
            .and_then(|p| {
                p.frames
                    .iter()
                    .find(|f| f.id == frame_id)
                    .and_then(|f| match f.kind {
                        book_core::FrameKind::Text { padding_pt, .. } => Some(padding_pt),
                        _ => None,
                    })
            })
            .unwrap_or(6.0)
            .max(0.0);

        let inner = Rect::new(
            frame_rect.x + padding,
            frame_rect.y + padding,
            (frame_rect.width - padding * 2.0).max(12.0),
            (frame_rect.height - padding * 2.0).max(12.0),
        );

        let page_obstacles = obstacles_per_page
            .get(page_idx)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);

        let mut lines = Vec::new();
        let mut y = inner.y;

        while y + line_height <= inner.bottom() && para_idx < paragraphs.len() {
            let band_rect = Rect::new(inner.x, y, inner.width, line_height);
            let segments = available_line_segments(frame_id, band_rect, page_obstacles);

            if segments.is_empty() {
                // If blocked by a JumpObject obstacle, jump directly to the bottom of the obstacle band
                let jump_bottom = page_obstacles
                    .iter()
                    .filter(|&&(obs_id, obs_rect, mode)| {
                        obs_id != frame_id
                            && mode == WrapMode::JumpObject
                            && band_rect.intersects(&obs_rect)
                            && obs_rect.bottom() > y
                    })
                    .map(|&(_, obs_rect, _)| obs_rect.bottom())
                    .fold(None::<f32>, |acc, b| Some(acc.map_or(b, |prev| prev.min(b))));

                if let Some(next_y) = jump_bottom {
                    y = next_y.max(y + 1.0);
                } else {
                    y += line_height * 0.5;
                }
                continue;
            }

            for (seg_x, seg_w) in segments {
                if para_idx >= paragraphs.len() {
                    break;
                }
                let words = &paragraphs[para_idx];
                if words.is_empty() {
                    // Blank paragraph line
                    para_idx += 1;
                    word_idx = 0;
                    break;
                }

                let mut current_line = String::new();
                let mut current_width_pt = 0.0_f32;
                let mut words_on_line = 0usize;

                while word_idx < words.len() {
                    let word = words[word_idx];
                    let word_w = measure_text_width_pt(word, &story.style);
                    let candidate_w = if current_line.is_empty() {
                        word_w
                    } else {
                        current_width_pt + space_w + word_w
                    };

                    if candidate_w <= seg_w
                        || (current_line.is_empty() && seg_w >= inner.width * 0.5)
                    {
                        if !current_line.is_empty() {
                            current_line.push(' ');
                        }
                        current_line.push_str(word);
                        current_width_pt = candidate_w;
                        words_on_line += 1;
                        word_idx += 1;
                    } else {
                        break;
                    }
                }

                if !current_line.is_empty() {
                    let is_last_in_para = word_idx >= words.len();
                    let effective_text_w = if story.style.alignment == TextAlignment::Justified
                        && !is_last_in_para
                        && words_on_line > 1
                    {
                        seg_w
                    } else {
                        current_width_pt.min(seg_w)
                    };
                    let align_offset = compute_alignment_offset_pt(
                        story.style.alignment,
                        seg_w,
                        effective_text_w,
                    );

                    lines.push(PositionedTextLine {
                        x_pt: seg_x + align_offset,
                        y_pt: y,
                        width_pt: seg_w,
                        height_pt: line_height,
                        text: current_line,
                        font_family: story.style.font_family.clone(),
                        font_size_pt: story.style.font_size_pt,
                        bold: story.style.bold,
                        italic: story.style.italic,
                        color: story.style.color,
                        alignment: story.style.alignment,
                    });
                }

                if word_idx >= words.len() {
                    para_idx += 1;
                    word_idx = 0;
                }
            }

            y += line_height;
        }

        let remaining_chars: usize = paragraphs
            .iter()
            .skip(para_idx)
            .enumerate()
            .map(|(i, p)| {
                let start = if i == 0 { word_idx } else { 0 };
                p.iter().skip(start).map(|w| w.chars().count() + 1).sum::<usize>()
            })
            .sum();

        let is_last_in_chain = chain_pos + 1 == chain.len();
        let next_linked_frame = chain
            .get(chain_pos + 1)
            .map(|&(next_page, next_id, next_rect)| (next_page, next_id, next_rect.center()));

        if let Some(page_layout) = pages.get_mut(page_idx) {
            page_layout.text_frames.insert(
                frame_id,
                ComputedTextFrame {
                    frame_id,
                    lines,
                    is_overflowing: is_last_in_chain && remaining_chars > 0,
                    overflow_char_count: if is_last_in_chain {
                        remaining_chars
                    } else {
                        0
                    },
                    next_linked_frame,
                },
            );
        }
    }
}

/// Subtract obstacle rectangles from `[band.x, band.right()]` for live text wrap.
pub fn available_line_segments(
    text_frame_id: FrameId,
    band: Rect,
    obstacles: &[(FrameId, Rect, WrapMode)],
) -> Vec<(f32, f32)> {
    if band.width <= 0.0 || band.height <= 0.0 {
        return Vec::new();
    }
    let mut intervals = vec![(band.x, band.right())];
    for &(obs_id, obs_rect, mode) in obstacles {
        if mode == WrapMode::None || obs_id == text_frame_id || !band.intersects(&obs_rect) {
            continue;
        }
        if mode == WrapMode::JumpObject {
            return Vec::new();
        }
        let cut_l = obs_rect.x;
        let cut_r = obs_rect.right();
        let mut next = Vec::new();
        for (a, b) in intervals {
            if cut_r <= a || cut_l >= b {
                next.push((a, b));
            } else {
                if cut_l > a {
                    next.push((a, cut_l));
                }
                if cut_r < b {
                    next.push((cut_r, b));
                }
            }
        }
        intervals = next;
    }
    intervals
        .into_iter()
        .filter_map(|(a, b)| {
            let w = b - a;
            if w >= 36.0 {
                Some((a, w))
            } else {
                None
            }
        })
        .collect()
}

/// Hit-tests all frames on `page` in topmost-first (`z_index` descending) order,
/// returning the `FrameId` of the top frame containing `point`.
pub fn hit_test_page_frames(page: &Page, point: Point) -> Option<FrameId> {
    for frame in page.frames_sorted_by_z().into_iter().rev() {
        let local_point = if frame.rotation_deg.abs() > 1e-3 {
            let c = frame.rect.center();
            let rad = -frame.rotation_deg.to_radians();
            let (sin_r, cos_r) = rad.sin_cos();
            let dx = point.x - c.x;
            let dy = point.y - c.y;
            Point::new(c.x + dx * cos_r - dy * sin_r, c.y + dx * sin_r + dy * cos_r)
        } else {
            point
        };
        if frame.rect.contains(local_point) {
            return Some(frame.id);
        }
    }
    None
}

/// Returns the 8 resize handle center points for a frame's bounding `Rect`.
pub fn frame_resize_handle_points(rect: Rect) -> [(ResizeHandle, Point); 8] {
    let cx = rect.x + rect.width * 0.5;
    let cy = rect.y + rect.height * 0.5;
    let r = rect.right();
    let b = rect.bottom();
    [
        (ResizeHandle::TopLeft, Point::new(rect.x, rect.y)),
        (ResizeHandle::TopCenter, Point::new(cx, rect.y)),
        (ResizeHandle::TopRight, Point::new(r, rect.y)),
        (ResizeHandle::MiddleLeft, Point::new(rect.x, cy)),
        (ResizeHandle::MiddleRight, Point::new(r, cy)),
        (ResizeHandle::BottomLeft, Point::new(rect.x, b)),
        (ResizeHandle::BottomCenter, Point::new(cx, b)),
        (ResizeHandle::BottomRight, Point::new(r, b)),
    ]
}

/// Hit-tests the 8 resize handles around `rect` within `handle_radius_pt`.
pub fn hit_test_resize_handle(
    rect: Rect,
    point: Point,
    handle_radius_pt: f32,
) -> Option<ResizeHandle> {
    let r2 = handle_radius_pt.max(1.0).powi(2);
    for (handle, hp) in frame_resize_handle_points(rect) {
        let dx = point.x - hp.x;
        let dy = point.y - hp.y;
        if dx * dx + dy * dy <= r2 {
            return Some(handle);
        }
    }
    None
}

/// Hit-tests a `ComputedTextFrame` to find the `(line_index, char_index)` closest to `point`.
pub fn hit_test_text_cursor_index(
    computed_frame: &ComputedTextFrame,
    point: Point,
) -> Option<(usize, usize)> {
    if computed_frame.lines.is_empty() {
        return None;
    }
    let mut best_line_idx = 0;
    let mut best_vy = f32::MAX;
    for (idx, line) in computed_frame.lines.iter().enumerate() {
        let line_cy = line.y_pt + line.height_pt * 0.5;
        let vy = (point.y - line_cy).abs();
        if vy < best_vy {
            best_vy = vy;
            best_line_idx = idx;
        }
    }
    let line = &computed_frame.lines[best_line_idx];
    let mut accum_x = line.x_pt;
    for (char_idx, ch) in line.text.chars().enumerate() {
        let cw = estimate_char_width_pt(
            ch,
            line.font_size_pt,
            &line.font_family,
            line.bold,
            line.italic,
        );
        if point.x < accum_x + cw * 0.5 {
            return Some((best_line_idx, char_idx));
        }
        accum_x += cw;
    }
    Some((best_line_idx, line.text.chars().count()))
}

/// Computes smart alignment snap guides and returns `(snapped_rect, active_guides)`
/// when moving or positioning `candidate_rect` on `page`.
///
/// Snaps `candidate_rect`'s left, center, and right X coordinates and top, center,
/// and bottom Y coordinates against:
/// 1. Page horizontal & vertical centers
/// 2. Page safe margin edges (`setup.safe_margin_rect(page_idx)`)
/// 3. Other frames' edges and centers on the same page
pub fn compute_smart_snap_guides(
    page: &Page,
    setup: &PageSetup,
    page_idx: usize,
    active_frame_id: impl Into<Option<FrameId>>,
    candidate_rect: Rect,
    threshold_pt: f32,
) -> (Rect, Vec<SnapGuide>) {
    let active_id: Option<FrameId> = active_frame_id.into();
    let threshold = threshold_pt.max(0.0);
    let safe_rect = setup.safe_margin_rect(page_idx);

    // (target_pos, start_span, end_span, kind, label)
    let mut x_targets: Vec<(f32, f32, f32, SnapTargetKind, String)> = vec![
        (
            setup.width_pt * 0.5,
            0.0,
            setup.height_pt,
            SnapTargetKind::PageCenter,
            "Page Center X".to_string(),
        ),
        (
            safe_rect.x,
            0.0,
            setup.height_pt,
            SnapTargetKind::SafeMargin,
            "Left Safe Margin".to_string(),
        ),
        (
            safe_rect.right(),
            0.0,
            setup.height_pt,
            SnapTargetKind::SafeMargin,
            "Right Safe Margin".to_string(),
        ),
    ];

    let mut y_targets: Vec<(f32, f32, f32, SnapTargetKind, String)> = vec![
        (
            setup.height_pt * 0.5,
            0.0,
            setup.width_pt,
            SnapTargetKind::PageCenter,
            "Page Center Y".to_string(),
        ),
        (
            safe_rect.y,
            0.0,
            setup.width_pt,
            SnapTargetKind::SafeMargin,
            "Top Safe Margin".to_string(),
        ),
        (
            safe_rect.bottom(),
            0.0,
            setup.width_pt,
            SnapTargetKind::SafeMargin,
            "Bottom Safe Margin".to_string(),
        ),
    ];

    for frame in &page.frames {
        if active_id == Some(frame.id) {
            continue;
        }
        let r = frame.rect;
        let c = r.center();
        let y_min = r.y.min(candidate_rect.y);
        let y_max = r.bottom().max(candidate_rect.bottom());
        x_targets.push((
            r.x,
            y_min,
            y_max,
            SnapTargetKind::FrameEdge,
            format!("{} Left", frame.label),
        ));
        x_targets.push((
            c.x,
            y_min,
            y_max,
            SnapTargetKind::FrameCenter,
            format!("{} Center X", frame.label),
        ));
        x_targets.push((
            r.right(),
            y_min,
            y_max,
            SnapTargetKind::FrameEdge,
            format!("{} Right", frame.label),
        ));

        let x_min = r.x.min(candidate_rect.x);
        let x_max = r.right().max(candidate_rect.right());
        y_targets.push((
            r.y,
            x_min,
            x_max,
            SnapTargetKind::FrameEdge,
            format!("{} Top", frame.label),
        ));
        y_targets.push((
            c.y,
            x_min,
            x_max,
            SnapTargetKind::FrameCenter,
            format!("{} Center Y", frame.label),
        ));
        y_targets.push((
            r.bottom(),
            x_min,
            x_max,
            SnapTargetKind::FrameEdge,
            format!("{} Bottom", frame.label),
        ));
    }

    let mut snapped_rect = candidate_rect;
    let mut guides = Vec::new();

    // Find best X snap delta across candidate [left, center_x, right]
    let cand_x_anchors = [
        candidate_rect.x,
        candidate_rect.x + candidate_rect.width * 0.5,
        candidate_rect.right(),
    ];
    let mut best_x_match: Option<(f32, f32, f32, f32, SnapTargetKind, String)> = None;
    for &(target_x, s_pt, e_pt, kind, ref label) in &x_targets {
        for &anchor_x in &cand_x_anchors {
            let delta = target_x - anchor_x;
            let dist = delta.abs();
            if dist <= threshold {
                if best_x_match
                    .as_ref()
                    .map_or(true, |(best_dist, ..)| dist < *best_dist)
                {
                    best_x_match = Some((dist, delta, target_x, s_pt.min(e_pt), kind, label.clone()));
                }
            }
        }
    }

    if let Some((_, dx, ..)) = best_x_match {
        snapped_rect.x += dx;
        let snapped_x_anchors = [
            snapped_rect.x,
            snapped_rect.x + snapped_rect.width * 0.5,
            snapped_rect.right(),
        ];
        for &(target_x, s_pt, e_pt, kind, ref label) in &x_targets {
            if snapped_x_anchors.iter().any(|&ax| (ax - target_x).abs() <= 0.05) {
                if !guides.iter().any(|g: &SnapGuide| {
                    g.axis == SnapAxis::Vertical && (g.position_pt - target_x).abs() <= 0.05
                }) {
                    guides.push(SnapGuide {
                        axis: SnapAxis::Vertical,
                        position_pt: target_x,
                        start_pt: s_pt,
                        end_pt: e_pt,
                        kind,
                        label: label.clone(),
                    });
                }
            }
        }
    }

    // Find best Y snap delta across candidate [top, center_y, bottom]
    let cand_y_anchors = [
        candidate_rect.y,
        candidate_rect.y + candidate_rect.height * 0.5,
        candidate_rect.bottom(),
    ];
    let mut best_y_match: Option<(f32, f32, f32, f32, SnapTargetKind, String)> = None;
    for &(target_y, s_pt, e_pt, kind, ref label) in &y_targets {
        for &anchor_y in &cand_y_anchors {
            let delta = target_y - anchor_y;
            let dist = delta.abs();
            if dist <= threshold {
                if best_y_match
                    .as_ref()
                    .map_or(true, |(best_dist, ..)| dist < *best_dist)
                {
                    best_y_match = Some((dist, delta, target_y, s_pt.min(e_pt), kind, label.clone()));
                }
            }
        }
    }

    if let Some((_, dy, ..)) = best_y_match {
        snapped_rect.y += dy;
        let snapped_y_anchors = [
            snapped_rect.y,
            snapped_rect.y + snapped_rect.height * 0.5,
            snapped_rect.bottom(),
        ];
        for &(target_y, s_pt, e_pt, kind, ref label) in &y_targets {
            if snapped_y_anchors.iter().any(|&ay| (ay - target_y).abs() <= 0.05) {
                if !guides.iter().any(|g: &SnapGuide| {
                    g.axis == SnapAxis::Horizontal && (g.position_pt - target_y).abs() <= 0.05
                }) {
                    guides.push(SnapGuide {
                        axis: SnapAxis::Horizontal,
                        position_pt: target_y,
                        start_pt: s_pt,
                        end_pt: e_pt,
                        kind,
                        label: label.clone(),
                    });
                }
            }
        }
    }

    (snapped_rect, guides)
}

#[cfg(test)]
mod tests {
    use super::*;
    use book_core::{
        BookDocument, ColorRgba, Frame, Page, PagePreset, PageSetup, Point, Rect, ShapeKind, Story,
        TextAlignment, TextStyle, WrapMode, WrapSettings,
    };

    #[test]
    fn test_character_and_font_family_metrics() {
        let narrow_i = estimate_char_width_pt('i', 12.0, "Literata", false, false);
        let regular_a = estimate_char_width_pt('a', 12.0, "Literata", false, false);
        let wide_m = estimate_char_width_pt('M', 12.0, "Literata", false, false);
        assert!(narrow_i < regular_a, "narrow glyph 'i' should be narrower than 'a'");
        assert!(regular_a < wide_m, "'a' should be narrower than uppercase 'M'");

        let bold_a = estimate_char_width_pt('a', 12.0, "Literata", true, false);
        let bold_italic_a = estimate_char_width_pt('a', 12.0, "Literata", true, true);
        assert!(bold_a > regular_a, "bold multiplier should increase glyph advance");
        assert!(
            bold_italic_a > bold_a,
            "italic multiplier should slightly increase glyph advance"
        );

        let mono_i = estimate_char_width_pt('i', 12.0, "JetBrains Mono", false, false);
        let mono_m = estimate_char_width_pt('M', 12.0, "JetBrains Mono", false, false);
        assert!(
            (mono_i - mono_m).abs() < 1e-4,
            "monospace font family must give identical advance to 'i' and 'M'"
        );
    }

    #[test]
    fn test_single_frame_wrapping_and_alignments() {
        let alignments = [
            TextAlignment::Left,
            TextAlignment::Center,
            TextAlignment::Right,
            TextAlignment::Justified,
        ];
        let mut first_line_xs = Vec::new();

        for alignment in alignments {
            let mut doc = BookDocument::default();
            let style = TextStyle {
                font_family: "Literata".to_string(),
                font_size_pt: 12.0,
                line_height_factor: 1.35,
                bold: false,
                italic: false,
                color: ColorRgba::BLACK,
                alignment,
            };
            let story_id = doc.insert_story(Story::new(
                "Alignment Story",
                "Short title\nSecond paragraph with several words that wrap cleanly across multiple lines inside the frame.",
                style,
            ));
            doc.pages[0]
                .frames
                .push(Frame::new_text(Rect::new(40.0, 50.0, 240.0, 180.0), story_id, 0));

            let layout = ComputedBookLayout::compute(&doc);
            let tf = layout.pages[0].text_frames.values().next().unwrap();
            assert!(!tf.is_overflowing);
            assert!(tf.lines.len() >= 3);
            assert_eq!(tf.lines[0].text, "Short title");
            assert_eq!(tf.lines[0].alignment, alignment);
            first_line_xs.push(tf.lines[0].x_pt);
        }

        let left_x = first_line_xs[0];
        let center_x = first_line_xs[1];
        let right_x = first_line_xs[2];
        let justified_x = first_line_xs[3];

        assert!((left_x - 46.0).abs() < 1e-3, "left-aligned line starts at inner.x");
        assert!(center_x > left_x + 20.0, "center-aligned short line shifts right");
        assert!(right_x > center_x + 20.0, "right-aligned short line shifts further right");
        assert!(
            (justified_x - left_x).abs() < 1e-3,
            "final line of a justified paragraph starts at inner.x"
        );
    }

    #[test]
    fn test_live_obstacle_text_wrap_bounding_box_and_jump_object() {
        let mut doc = BookDocument::default();
        let prose = "Word ".repeat(140);
        let story_id = doc.insert_story(Story::new("Wrap Story", prose, TextStyle::default()));

        let text_frame = Frame::new_text(Rect::new(36.0, 36.0, 360.0, 420.0), story_id, 0);
        let tf_id = text_frame.id;
        doc.pages[0].frames.push(text_frame);

        // Place a BoundingBox obstacle in the horizontal center so lines split into left & right segments
        let mut bbox_obs = Frame::new_shape(
            Rect::new(166.0, 100.0, 100.0, 80.0),
            ShapeKind::Rectangle,
            ColorRgba::WARM_GOLD,
        );
        bbox_obs.wrap = WrapSettings {
            mode: WrapMode::BoundingBox,
            padding_pt: 8.0,
        };
        let bbox_id = bbox_obs.id;
        doc.pages[0].frames.push(bbox_obs);

        let layout = ComputedBookLayout::compute(&doc);
        let tf = &layout.pages[0].text_frames[&tf_id];

        // At y around 110..170, there should be two segments on the same Y coordinate (left & right of obstacle)
        let split_band_lines: Vec<&PositionedTextLine> = tf
            .lines
            .iter()
            .filter(|l| l.y_pt >= 100.0 && l.y_pt <= 170.0)
            .collect();
        assert!(
            split_band_lines.len() >= 4,
            "BoundingBox obstacle in center should split horizontal bands into left and right segments"
        );
        let sample_y = split_band_lines[0].y_pt;
        let same_y: Vec<&&PositionedTextLine> = split_band_lines
            .iter()
            .filter(|l| (l.y_pt - sample_y).abs() < 1e-3)
            .collect();
        assert_eq!(
            same_y.len(),
            2,
            "A horizontal band intersecting a central BoundingBox obstacle must produce 2 side-by-side segments"
        );
        assert!(same_y[0].x_pt < 166.0);
        assert!(same_y[1].x_pt >= 266.0);

        // Switch obstacle to JumpObject and verify no lines are placed inside the vertical jump band
        doc.pages[0]
            .frames
            .iter_mut()
            .find(|f| f.id == bbox_id)
            .unwrap()
            .wrap
            .mode = WrapMode::JumpObject;

        let jump_layout = ComputedBookLayout::compute(&doc);
        let jump_tf = &jump_layout.pages[0].text_frames[&tf_id];
        let expanded_jump = Rect::new(166.0, 100.0, 100.0, 80.0).expand(8.0);
        for line in &jump_tf.lines {
            let line_rect = Rect::new(line.x_pt, line.y_pt, line.width_pt, line.height_pt);
            assert!(
                !line_rect.intersects(&expanded_jump),
                "JumpObject mode must completely skip vertical bands overlapping the expanded obstacle"
            );
        }
    }

    #[test]
    fn test_multi_page_threaded_frames_and_overflow_detection() {
        let mut doc = BookDocument::default();
        doc.pages.push(Page::new("Page 2", None));

        let medium_text = (1..=45)
            .map(|n| format!("word{n}"))
            .collect::<Vec<_>>()
            .join(" ");
        let story_id =
            doc.insert_story(Story::new("Threaded Story", medium_text, TextStyle::default()));

        // Frame 1 on Page 1 (small box: cannot hold all 45 words alone)
        let f1 = Frame::new_text(Rect::new(40.0, 40.0, 160.0, 70.0), story_id, 0);
        let f1_id = f1.id;
        doc.pages[0].frames.push(f1);

        let layout_single = ComputedBookLayout::compute(&doc);
        let c1_single = &layout_single.pages[0].text_frames[&f1_id];
        assert!(
            c1_single.is_overflowing,
            "Single small frame must report is_overflowing = true"
        );
        assert!(c1_single.overflow_char_count > 0);
        assert!(c1_single.next_linked_frame.is_none());

        // Add Frame 2 on Page 2 and link Frame 1 -> Frame 2
        let f2 = Frame::new_text(Rect::new(40.0, 40.0, 320.0, 360.0), story_id, 1);
        let f2_id = f2.id;
        doc.pages[1].frames.push(f2);

        let layout_linked = ComputedBookLayout::compute(&doc);
        let c1 = &layout_linked.pages[0].text_frames[&f1_id];
        let c2 = &layout_linked.pages[1].text_frames[&f2_id];

        assert!(
            !c1.is_overflowing,
            "First frame in a chain is never marked overflowing"
        );
        assert_eq!(c1.overflow_char_count, 0);
        assert_eq!(
            c1.next_linked_frame.map(|(p, id, _)| (p, id)),
            Some((1, f2_id))
        );
        assert!(
            !c2.lines.is_empty(),
            "Remaining story text must flow into Frame 2 on Page 2"
        );
        assert!(
            !c2.is_overflowing,
            "Large second frame fits all remaining text without overflow"
        );
        assert_eq!(c2.overflow_char_count, 0);

        // Now make the story huge so even the 2-frame chain overflows on Frame 2
        if let Some(story) = doc.stories.get_mut(&story_id) {
            story.text = (1..=450)
                .map(|n| format!("word{n}"))
                .collect::<Vec<_>>()
                .join(" ");
        }
        let layout_chain_overflow = ComputedBookLayout::compute(&doc);
        assert!(!layout_chain_overflow.pages[0].text_frames[&f1_id].is_overflowing);
        assert!(layout_chain_overflow.pages[1].text_frames[&f2_id].is_overflowing);
        assert!(layout_chain_overflow.pages[1].text_frames[&f2_id].overflow_char_count > 0);
    }

    #[test]
    fn test_smart_snap_guides_and_frame_hit_testing() {
        let setup = PageSetup::from_preset(PagePreset::UsTrade6x9); // 432 x 648 pt
        let mut page = Page::new("Snap Test Page", None);

        let mut bottom_frame = Frame::new_shape(
            Rect::new(50.0, 50.0, 120.0, 100.0),
            ShapeKind::Rectangle,
            ColorRgba::ACCENT_BLUE,
        );
        bottom_frame.z_index = 5;
        let bottom_id = bottom_frame.id;

        let mut top_frame = Frame::new_shape(
            Rect::new(90.0, 80.0, 120.0, 100.0),
            ShapeKind::Rectangle,
            ColorRgba::WARM_GOLD,
        );
        top_frame.z_index = 20;
        let top_id = top_frame.id;

        page.frames.push(bottom_frame);
        page.frames.push(top_frame);

        // Hit test overlapping region (100, 100) -> should return top_frame (z_index 20)
        assert_eq!(hit_test_page_frames(&page, Point::new(100.0, 100.0)), Some(top_id));
        // Hit test non-overlapping region of bottom_frame (60, 60) -> should return bottom_frame
        assert_eq!(hit_test_page_frames(&page, Point::new(60.0, 60.0)), Some(bottom_id));
        // Hit test empty canvas -> None
        assert_eq!(hit_test_page_frames(&page, Point::new(400.0, 600.0)), None);

        // Snap candidate to Page Center (216.0, 324.0)
        let candidate_center = Rect::new(164.0, 276.0, 100.0, 100.0); // center is (214, 326)
        let (snapped_c, guides_c) =
            compute_smart_snap_guides(&page, &setup, 0, None, candidate_center, 5.0);
        assert!((snapped_c.center().x - 216.0).abs() < 1e-3);
        assert!((snapped_c.center().y - 324.0).abs() < 1e-3);
        assert!(guides_c.iter().any(|g| g.kind == SnapTargetKind::PageCenter && g.axis == SnapAxis::Vertical));
        assert!(guides_c.iter().any(|g| g.kind == SnapTargetKind::PageCenter && g.axis == SnapAxis::Horizontal));

        // Snap candidate to Safe Margins (left = 45.0, top = 36.0)
        let candidate_margin = Rect::new(47.5, 33.0, 80.0, 60.0);
        let (snapped_m, guides_m) =
            compute_smart_snap_guides(&page, &setup, 0, None, candidate_margin, 5.0);
        assert!((snapped_m.x - 45.0).abs() < 1e-3);
        assert!((snapped_m.y - 36.0).abs() < 1e-3);
        assert!(guides_m.iter().any(|g| g.kind == SnapTargetKind::SafeMargin));

        // Snap candidate left edge to bottom_frame right edge (170.0)
        let candidate_frame_edge = Rect::new(172.0, 220.0, 80.0, 60.0);
        let (snapped_f, guides_f) =
            compute_smart_snap_guides(&page, &setup, 0, top_id, candidate_frame_edge, 5.0);
        assert!((snapped_f.x - 170.0).abs() < 1e-3);
        assert!(guides_f.iter().any(|g| g.kind == SnapTargetKind::FrameEdge));
    }

    #[test]
    fn test_resize_handles_and_text_cursor_hit_testing() {
        let rect = Rect::new(50.0, 60.0, 200.0, 100.0);
        assert_eq!(
            hit_test_resize_handle(rect, Point::new(51.0, 61.0), 6.0),
            Some(ResizeHandle::TopLeft)
        );
        assert_eq!(
            hit_test_resize_handle(rect, Point::new(250.0, 160.0), 6.0),
            Some(ResizeHandle::BottomRight)
        );
        assert_eq!(
            hit_test_resize_handle(rect, Point::new(150.0, 110.0), 6.0),
            None
        );

        let mut doc = BookDocument::default();
        let story_id = doc.insert_story(Story::new(
            "Cursor Story",
            "Hello world on line one\nSecond line here",
            TextStyle::default(),
        ));
        let frame = Frame::new_text(Rect::new(40.0, 40.0, 300.0, 120.0), story_id, 0);
        let frame_id = frame.id;
        doc.pages[0].frames.push(frame);

        let layout = ComputedBookLayout::compute(&doc);
        let tf = &layout.pages[0].text_frames[&frame_id];
        let (line_idx, char_idx) =
            hit_test_text_cursor_index(tf, Point::new(46.0, 48.0)).expect("cursor hit");
        assert_eq!(line_idx, 0);
        assert_eq!(char_idx, 0);

        let (line2_idx, end_char_idx) =
            hit_test_text_cursor_index(tf, Point::new(300.0, 66.0)).expect("cursor hit line 2");
        assert_eq!(line2_idx, 1);
        assert_eq!(end_char_idx, "Second line here".chars().count());
    }
}
