//! Deterministic typography, multi-frame story threading, and live obstacle
//! text-wrap ("runaround") layout engine shared by the Canvas, PDF, and EPUB exporters.

use book_core::{BookDocument, ColorRgba, FrameId, PageId, Point, Rect, TextAlignment};
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
    pub next_linked_frame: Option<( usize, FrameId, Point )>,
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
        let obstacles_per_page: Vec<Vec<(FrameId, Rect, book_core::WrapMode)>> = doc
            .pages
            .iter()
            .map(|page| {
                page.frames
                    .iter()
                    .filter(|f| f.wrap.mode != book_core::WrapMode::None)
                    .map(|f| (f.id, f.rect.expand(f.wrap.padding_pt), f.wrap.mode))
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

        Self { pages }
    }
}

fn flow_story_into_chain(
    doc: &BookDocument,
    story: &book_core::Story,
    chain: &[(usize, FrameId, Rect)],
    obstacles_per_page: &[Vec<(FrameId, Rect, book_core::WrapMode)>],
    pages: &mut [ComputedPageLayout],
) {
    let line_height = (story.style.font_size_pt * story.style.line_height_factor).max(8.0);
    let approx_char_w = (story.style.font_size_pt * 0.52).max(3.5);

    // Tokenize paragraphs into words while preserving explicit line breaks
    let paragraphs: Vec<Vec<&str>> = story
        .text
        .split('\n')
        .map(|para| para.split_whitespace().collect())
        .collect();

    let mut para_idx = 0;
    let mut word_idx = 0;

    for (chain_pos, &(page_idx, frame_id, frame_rect)) in chain.iter().enumerate() {
        let padding = doc.pages.get(page_idx).and_then(|p| {
            p.frames.iter().find(|f| f.id == frame_id).and_then(|f| match f.kind {
                book_core::FrameKind::Text { padding_pt, .. } => Some(padding_pt),
                _ => None,
            })
        }).unwrap_or(6.0);

        let inner = Rect::new(
            frame_rect.x + padding,
            frame_rect.y + padding,
            (frame_rect.width - padding * 2.0).max(12.0),
            (frame_rect.height - padding * 2.0).max(12.0),
        );

        let mut lines = Vec::new();
        let mut y = inner.y;

        while y + line_height <= inner.bottom() && para_idx < paragraphs.len() {
            let band_rect = Rect::new(inner.x, y, inner.width, line_height);
            let segments = available_line_segments(
                frame_id,
                band_rect,
                obstacles_per_page.get(page_idx).map(|v| v.as_slice()).unwrap_or(&[]),
            );

            if segments.is_empty() {
                y += line_height * 0.5;
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

                let max_chars = ((seg_w / approx_char_w).floor() as usize).max(1);
                let mut current_line = String::new();

                while word_idx < words.len() {
                    let word = words[word_idx];
                    let needed = if current_line.is_empty() {
                        word.chars().count()
                    } else {
                        current_line.chars().count() + 1 + word.chars().count()
                    };
                    if needed <= max_chars || current_line.is_empty() && seg_w >= inner.width * 0.6 {
                        if !current_line.is_empty() {
                            current_line.push(' ');
                        }
                        current_line.push_str(word);
                        word_idx += 1;
                    } else {
                        break;
                    }
                }

                if !current_line.is_empty() {
                    lines.push(PositionedTextLine {
                        x_pt: seg_x,
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
                p.iter().skip(start).map(|w| w.len() + 1).sum::<usize>()
            })
            .sum();

        let is_last_in_chain = chain_pos + 1 == chain.len();
        let next_linked_frame = chain.get(chain_pos + 1).map(|&(next_page, next_id, next_rect)| {
            (next_page, next_id, next_rect.center())
        });

        if let Some(page_layout) = pages.get_mut(page_idx) {
            page_layout.text_frames.insert(
                frame_id,
                ComputedTextFrame {
                    frame_id,
                    lines,
                    is_overflowing: is_last_in_chain && remaining_chars > 0,
                    overflow_char_count: if is_last_in_chain { remaining_chars } else { 0 },
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
    obstacles: &[(FrameId, Rect, book_core::WrapMode)],
) -> Vec<(f32, f32)> {
    let mut intervals = vec![(band.x, band.right())];
    for &(obs_id, obs_rect, mode) in obstacles {
        if obs_id == text_frame_id || !band.intersects(&obs_rect) {
            continue;
        }
        if mode == book_core::WrapMode::JumpObject {
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
