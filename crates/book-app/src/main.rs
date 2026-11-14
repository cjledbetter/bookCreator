//! Desktop application entry point (`book-creator`) built with `eframe` / `egui`.
//!
//! Combines InDesign-style fluid WYSIWYG layout (live obstacle text-wrap, threaded
//! multi-page stories, smart alignment snap guides, direct on-canvas text editing)
//! with Scribus-style open portable `.book` / `.bookdir` bundles, print bleed/crop
//! marks, and equal-priority Print PDF + EPUB 3.3 export.

pub mod canvas;
pub mod help_drawer;
pub mod inspector;
pub mod toolbar;

use book_core::{
    AssetKind, BookDocument, ColorRgba, CommandHistory, Frame, FrameId, FrameKind, Page, Point,
    PreflightIssue, Rect, ShapeKind, Story, TextStyle,
};
use book_epub::EpubLayoutMode;
use book_help::{create_tutorial_book, run_preflight};
use book_layout::ComputedBookLayout;
use eframe::egui;
use std::path::Path;
use uuid::Uuid;

use crate::canvas::CanvasInteractionState;

/// The 5 core layout tools in the top studio toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    /// Select, move, resize, or double-click text frames to edit (`V`).
    SelectMove,
    /// Click-and-drag on the canvas to create a new Story Text Frame (`T`).
    TextFrame,
    /// Click-and-drag on the canvas to create a new Image Frame (`I`).
    ImageFrame,
    /// Click-and-drag on the canvas to draw a new Vector Shape Frame (`S`).
    ShapeFrame,
    /// Click source Text Frame then target Text Frame to link/thread stories (`L`).
    LinkText,
}

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Book Creator — Visual Print PDF & EPUB 3 Studio")
            .with_inner_size([1420.0, 880.0])
            .with_min_inner_size([1024.0, 660.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "Book Creator",
        native_options,
        Box::new(|cc| {
            configure_studio_visuals(&cc.egui_ctx);
            Ok(Box::new(BookCreatorApp::new()))
        }),
    )
}

fn configure_studio_visuals(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.window_corner_radius = egui::CornerRadius::same(8);
    visuals.panel_fill = egui::Color32::from_rgb(24, 27, 34);
    visuals.window_fill = egui::Color32::from_rgb(26, 29, 37);
    visuals.selection.bg_fill = egui::Color32::from_rgb(37, 99, 235);
    ctx.set_visuals(visuals);
}

pub struct BookCreatorApp {
    pub doc: BookDocument,
    pub history: CommandHistory,
    pub layout: ComputedBookLayout,
    pub preflight_issues: Vec<PreflightIssue>,
    pub current_page: usize,
    pub selected_frame_id: Option<FrameId>,
    pub editing_text_frame_id: Option<FrameId>,
    pub active_tool: Tool,
    pub shape_tool_kind: ShapeKind,
    pub link_source_frame_id: Option<FrameId>,
    pub zoom: f32,
    pub pan_offset: egui::Vec2,
    pub show_margins_and_bleed: bool,
    pub show_thread_lines: bool,
    pub show_help: bool,
    pub help_search_query: String,
    pub help_selected_category: Option<String>,
    pub show_preflight_drawer: bool,
    pub pdf_include_bleed: bool,
    pub pdf_include_crop_marks: bool,
    pub epub_layout_mode: EpubLayoutMode,
    pub status_message: String,
    pub canvas_interaction: CanvasInteractionState,
}

impl Default for BookCreatorApp {
    fn default() -> Self {
        Self::new()
    }
}

impl BookCreatorApp {
    pub fn new() -> Self {
        let doc = create_tutorial_book();
        let layout = ComputedBookLayout::compute(&doc);
        let preflight_issues = run_preflight(&doc, &layout);
        Self {
            doc,
            history: CommandHistory::default(),
            layout,
            preflight_issues,
            current_page: 0,
            selected_frame_id: None,
            editing_text_frame_id: None,
            active_tool: Tool::SelectMove,
            shape_tool_kind: ShapeKind::RoundedRectangle,
            link_source_frame_id: None,
            zoom: 1.0,
            pan_offset: egui::Vec2::ZERO,
            show_margins_and_bleed: true,
            show_thread_lines: true,
            show_help: false,
            help_search_query: String::new(),
            help_selected_category: None,
            show_preflight_drawer: false,
            pdf_include_bleed: true,
            pdf_include_crop_marks: true,
            epub_layout_mode: EpubLayoutMode::FixedLayout,
            status_message:
                "Loaded 4-Page Interactive Starter Tutorial Book. Drag elements or press F1 for Help Manual."
                    .to_string(),
            canvas_interaction: CanvasInteractionState::default(),
        }
    }

    pub fn recompute_layout_and_preflight(&mut self) {
        self.layout = ComputedBookLayout::compute(&self.doc);
        self.preflight_issues = run_preflight(&self.doc, &self.layout);
    }

    pub fn record_undo_snapshot(&mut self) {
        self.history.push_snapshot(&self.doc);
    }

    pub fn undo(&mut self) -> bool {
        if let Some(prev_doc) = self.history.undo(&self.doc) {
            self.doc = prev_doc;
            self.current_page = self.current_page.min(self.doc.pages.len().saturating_sub(1));
            self.selected_frame_id = None;
            self.editing_text_frame_id = None;
            self.recompute_layout_and_preflight();
            self.status_message = "Undid last edit.".to_string();
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(next_doc) = self.history.redo(&self.doc) {
            self.doc = next_doc;
            self.current_page = self.current_page.min(self.doc.pages.len().saturating_sub(1));
            self.selected_frame_id = None;
            self.editing_text_frame_id = None;
            self.recompute_layout_and_preflight();
            self.status_message = "Redid edit.".to_string();
            true
        } else {
            false
        }
    }

    pub fn select_tool(&mut self, tool: Tool) {
        self.active_tool = tool;
        if tool != Tool::LinkText {
            self.link_source_frame_id = None;
        }
    }

    pub fn new_blank_book(&mut self) {
        self.record_undo_snapshot();
        self.doc = BookDocument::default();
        self.current_page = 0;
        self.selected_frame_id = None;
        self.editing_text_frame_id = None;
        self.link_source_frame_id = None;
        self.recompute_layout_and_preflight();
        self.status_message = "Created new blank book project.".to_string();
    }

    pub fn load_tutorial_book(&mut self) {
        self.record_undo_snapshot();
        self.doc = create_tutorial_book();
        self.current_page = 0;
        self.selected_frame_id = None;
        self.editing_text_frame_id = None;
        self.link_source_frame_id = None;
        self.recompute_layout_and_preflight();
        self.status_message = "Loaded 4-Page Interactive Starter Tutorial Book.".to_string();
    }

    pub fn fit_page_in_viewport(&mut self) {
        let canvas_size = self.canvas_interaction.last_canvas_size;
        if canvas_size.x > 100.0 && canvas_size.y > 100.0 {
            let zx = (canvas_size.x - 80.0) / self.doc.setup.width_pt.max(100.0);
            let zy = (canvas_size.y - 80.0) / self.doc.setup.height_pt.max(100.0);
            self.zoom = zx.min(zy).clamp(0.35, 2.5);
        } else {
            self.zoom = 1.0;
        }
        self.pan_offset = egui::Vec2::ZERO;
    }

    // --- Page Rail Operations ---

    pub fn add_page(&mut self) {
        self.record_undo_snapshot();
        let master_id = self.doc.master_pages.first().map(|m| m.id);
        let new_num = self.doc.pages.len() + 1;
        self.doc
            .pages
            .push(Page::new(format!("Page {new_num}"), master_id));
        self.current_page = self.doc.pages.len() - 1;
        self.selected_frame_id = None;
        self.recompute_layout_and_preflight();
        self.status_message = format!("Added Page {new_num}.");
    }

    pub fn duplicate_current_page(&mut self) {
        if let Some(current) = self.doc.pages.get(self.current_page).cloned() {
            self.record_undo_snapshot();
            let mut dup = current;
            dup.id = Uuid::new_v4();
            dup.title = format!("{} (Copy)", dup.title);
            for frame in &mut dup.frames {
                frame.id = Uuid::new_v4();
                // Give duplicated unthreaded text frames their own independent story copy
                if let FrameKind::Text {
                    ref mut story_id,
                    thread_order,
                    ..
                } = frame.kind
                {
                    if thread_order == 0 {
                        if let Some(orig_story) = self.doc.stories.get(story_id).cloned() {
                            let new_story = Story::new(
                                format!("{} (Copy)", orig_story.title),
                                orig_story.text,
                                orig_story.style,
                            );
                            *story_id = self.doc.insert_story(new_story);
                        }
                    }
                }
            }
            let insert_idx = (self.current_page + 1).min(self.doc.pages.len());
            self.doc.pages.insert(insert_idx, dup);
            self.current_page = insert_idx;
            self.selected_frame_id = None;
            self.recompute_layout_and_preflight();
            self.status_message = format!("Duplicated page to Page {}.", insert_idx + 1);
        }
    }

    pub fn delete_current_page(&mut self) {
        if self.doc.pages.len() <= 1 {
            self.status_message = "Cannot delete the only page in the book.".to_string();
            return;
        }
        self.record_undo_snapshot();
        self.doc.pages.remove(self.current_page);
        if self.current_page >= self.doc.pages.len() {
            self.current_page = self.doc.pages.len() - 1;
        }
        self.selected_frame_id = None;
        self.editing_text_frame_id = None;
        self.recompute_layout_and_preflight();
        self.status_message = "Deleted page.".to_string();
    }

    pub fn move_current_page_up(&mut self) {
        if self.current_page > 0 && self.current_page < self.doc.pages.len() {
            self.record_undo_snapshot();
            self.doc.pages.swap(self.current_page, self.current_page - 1);
            self.current_page -= 1;
            self.recompute_layout_and_preflight();
        }
    }

    pub fn move_current_page_down(&mut self) {
        if self.current_page + 1 < self.doc.pages.len() {
            self.record_undo_snapshot();
            self.doc.pages.swap(self.current_page, self.current_page + 1);
            self.current_page += 1;
            self.recompute_layout_and_preflight();
        }
    }

    // --- Frame Creation, Z-Order, Deletion & Story Threading ---

    pub fn create_frame_from_drag(
        &mut self,
        page_idx: usize,
        tool: Tool,
        rect: Rect,
    ) -> Option<FrameId> {
        if page_idx >= self.doc.pages.len() {
            return None;
        }
        self.record_undo_snapshot();
        let next_z = self.doc.pages[page_idx]
            .frames
            .iter()
            .map(|f| f.z_index)
            .max()
            .unwrap_or(0)
            + 5;

        let mut frame = match tool {
            Tool::TextFrame => {
                let story = Story::new(
                    format!("Story {}", self.doc.stories.len() + 1),
                    "Double-click this text frame on the canvas (or edit in the Right Inspector) to write your story. Drag an image or shape over this box to see live obstacle text wrap!",
                    TextStyle::default(),
                );
                let story_id = self.doc.insert_story(story);
                Frame::new_text(rect, story_id, 0)
            }
            Tool::ImageFrame => Frame::new_image(rect, None),
            Tool::ShapeFrame => {
                Frame::new_shape(rect, self.shape_tool_kind, ColorRgba::ACCENT_BLUE)
            }
            Tool::SelectMove | Tool::LinkText => return None,
        };

        frame.z_index = next_z;
        let id = frame.id;
        self.doc.pages[page_idx].frames.push(frame);
        self.selected_frame_id = Some(id);
        self.recompute_layout_and_preflight();
        self.status_message = format!("Created new frame ({:.0}×{:.0} pt).", rect.width, rect.height);
        Some(id)
    }

    pub fn handle_link_tool_click(&mut self, clicked_frame_id: FrameId) -> bool {
        let is_text_frame = self
            .doc
            .pages
            .iter()
            .flat_map(|p| &p.frames)
            .any(|f| f.id == clicked_frame_id && matches!(f.kind, FrameKind::Text { .. }));

        if !is_text_frame {
            self.status_message =
                "Link Text (L) only connects Text Frames. Click a Text Frame.".to_string();
            return false;
        }

        if let Some(src_id) = self.link_source_frame_id {
            if src_id == clicked_frame_id {
                self.link_source_frame_id = None;
                self.status_message = "Cancelled text frame link selection.".to_string();
                return false;
            }
            self.record_undo_snapshot();
            if self.doc.link_text_frames(src_id, clicked_frame_id) {
                self.link_source_frame_id = None;
                self.selected_frame_id = Some(clicked_frame_id);
                self.active_tool = Tool::SelectMove;
                self.recompute_layout_and_preflight();
                self.status_message =
                    "Linked text frames! Story now flows across the threaded chain.".to_string();
                return true;
            }
        } else {
            self.link_source_frame_id = Some(clicked_frame_id);
            self.selected_frame_id = Some(clicked_frame_id);
            self.status_message =
                "Source Text Frame selected. Now click the destination Text Frame (on this or another page) to link."
                    .to_string();
        }
        false
    }

    pub fn delete_selected_frame(&mut self) {
        let Some(sel_id) = self.selected_frame_id else {
            return;
        };
        if let Some(page) = self.doc.pages.get(self.current_page) {
            if page.frames.iter().any(|f| f.id == sel_id) {
                self.record_undo_snapshot();
                self.doc.pages[self.current_page]
                    .frames
                    .retain(|f| f.id != sel_id);
                self.selected_frame_id = None;
                self.editing_text_frame_id = None;
                self.recompute_layout_and_preflight();
                self.status_message = "Deleted frame.".to_string();
            }
        }
    }

    pub fn bring_selected_to_front(&mut self) {
        let Some(sel_id) = self.selected_frame_id else {
            return;
        };
        if let Some(page) = self.doc.pages.get(self.current_page) {
            let max_z = page.frames.iter().map(|f| f.z_index).max().unwrap_or(10);
            self.record_undo_snapshot();
            if let Some(frame) = self.doc.pages[self.current_page]
                .frames
                .iter_mut()
                .find(|f| f.id == sel_id)
            {
                frame.z_index = max_z + 5;
            }
        }
    }

    pub fn send_selected_to_back(&mut self) {
        let Some(sel_id) = self.selected_frame_id else {
            return;
        };
        if let Some(page) = self.doc.pages.get(self.current_page) {
            let min_z = page.frames.iter().map(|f| f.z_index).min().unwrap_or(0);
            self.record_undo_snapshot();
            if let Some(frame) = self.doc.pages[self.current_page]
                .frames
                .iter_mut()
                .find(|f| f.id == sel_id)
            {
                frame.z_index = min_z - 5;
            }
        }
    }

    pub fn jump_to_preflight_issue(
        &mut self,
        page_idx: Option<usize>,
        frame_id: Option<FrameId>,
    ) {
        if let Some(p_idx) = page_idx {
            if p_idx < self.doc.pages.len() {
                self.current_page = p_idx;
            }
        }
        if let Some(f_id) = frame_id {
            self.selected_frame_id = Some(f_id);
        }
        self.status_message = "Jumped to preflight issue target.".to_string();
    }

    /// Import an image asset using strictly relative bundle paths (`assets/images/...`)
    /// and populate `target_frame` (or create a new `ImageFrame` at `drop_point`).
    pub fn import_image_bytes_into_doc(
        &mut self,
        filename: &str,
        bytes: Vec<u8>,
        target_frame: Option<FrameId>,
        drop_point: Option<Point>,
    ) -> FrameId {
        self.record_undo_snapshot();

        let ext = Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png")
            .to_lowercase();
        let mime_type = match ext.as_str() {
            "jpg" | "jpeg" => "image/jpeg",
            "webp" => "image/webp",
            "svg" => "image/svg+xml",
            _ => "image/png",
        };

        let asset_key = format!("img_{}", &Uuid::new_v4().simple().to_string()[..8]);
        let clean_name = Path::new(filename)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("image.png")
            .to_string();
        let asset_id = self.doc.add_asset_bytes(
            asset_key,
            AssetKind::Image,
            &clean_name,
            mime_type,
            bytes,
            None,
        );

        if let Some(f_id) = target_frame {
            if let Some(frame) = self
                .doc
                .pages
                .get_mut(self.current_page)
                .and_then(|p| p.frames.iter_mut().find(|f| f.id == f_id))
            {
                if let FrameKind::Image {
                    asset_id: ref mut a_id,
                    ref mut caption,
                    ..
                } = frame.kind
                {
                    *a_id = Some(asset_id);
                    if caption.is_empty() {
                        *caption = clean_name.clone();
                    }
                    self.recompute_layout_and_preflight();
                    self.status_message = format!("Imported image '{clean_name}' into frame.");
                    return f_id;
                }
            }
        }

        // Otherwise create a new ImageFrame at the drop point (or centered on page)
        let w = 220.0;
        let h = 160.0;
        let pt = drop_point.unwrap_or_else(|| {
            Point::new(
                (self.doc.setup.width_pt - w) * 0.5,
                (self.doc.setup.height_pt - h) * 0.5,
            )
        });
        let rect = Rect::new(
            (pt.x - w * 0.5).clamp(18.0, (self.doc.setup.width_pt - w - 18.0).max(18.0)),
            (pt.y - h * 0.5).clamp(18.0, (self.doc.setup.height_pt - h - 18.0).max(18.0)),
            w,
            h,
        );
        let mut new_img = Frame::new_image(rect, Some(asset_id));
        new_img.label = clean_name.clone();
        if let FrameKind::Image {
            ref mut caption, ..
        } = new_img.kind
        {
            *caption = clean_name.clone();
        }
        let id = new_img.id;
        if let Some(page) = self.doc.pages.get_mut(self.current_page) {
            page.frames.push(new_img);
        }
        self.selected_frame_id = Some(id);
        self.recompute_layout_and_preflight();
        self.status_message = format!("Placed image '{clean_name}' onto Page {}.", self.current_page + 1);
        id
    }

    fn handle_os_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if dropped.is_empty() {
            return;
        }

        for file in dropped {
            if let Some(ref path) = file.path {
                let ext = path
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if ext == "book" {
                    if let Ok(loaded) = BookDocument::load_book_archive(path) {
                        self.record_undo_snapshot();
                        self.doc = loaded;
                        self.current_page = 0;
                        self.selected_frame_id = None;
                        self.recompute_layout_and_preflight();
                        self.status_message = format!("Opened dropped project '{}'", path.display());
                    }
                    continue;
                } else if ext == "bookdir" || path.is_dir() {
                    if let Ok(loaded) = BookDocument::load_book_directory(path) {
                        self.record_undo_snapshot();
                        self.doc = loaded;
                        self.current_page = 0;
                        self.selected_frame_id = None;
                        self.recompute_layout_and_preflight();
                        self.status_message =
                            format!("Opened dropped .bookdir '{}'", path.display());
                    }
                    continue;
                } else if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "svg") {
                    if let Ok(bytes) = std::fs::read(path) {
                        let filename = path
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("dropped_image.png");
                        let target_img_frame = self.selected_frame_id.filter(|&sel_id| {
                            self.doc
                                .pages
                                .get(self.current_page)
                                .and_then(|p| p.frames.iter().find(|f| f.id == sel_id))
                                .map_or(false, |f| matches!(f.kind, FrameKind::Image { .. }))
                        });
                        self.import_image_bytes_into_doc(filename, bytes, target_img_frame, None);
                    }
                }
            } else if let Some(ref bytes) = file.bytes {
                let name = if file.name.is_empty() {
                    "dropped_image.png"
                } else {
                    &file.name
                };
                self.import_image_bytes_into_doc(name, bytes.to_vec(), None, None);
            }
        }
    }
}

impl eframe::App for BookCreatorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_os_dropped_files(ctx);

        toolbar::show_top_toolbar(self, ctx);
        toolbar::show_left_page_rail(self, ctx);
        inspector::show_right_inspector(self, ctx);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                canvas::show_spread_canvas(self, ui);
            });

        help_drawer::show_help_manual_window(self, ctx);
        help_drawer::show_preflight_window(self, ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::{compute_smart_snap, hit_test_frame, ResizeHandle};

    #[test]
    fn starter_tutorial_book_loads_and_computes_layout_cleanly() {
        let app = BookCreatorApp::new();
        assert_eq!(app.doc.pages.len(), 4);
        assert_eq!(app.layout.pages.len(), 4);
        assert_eq!(app.active_tool, Tool::SelectMove);
    }

    #[test]
    fn tool_switching_and_undo_redo_integration_work() {
        let mut app = BookCreatorApp::new();
        app.select_tool(Tool::TextFrame);
        assert_eq!(app.active_tool, Tool::TextFrame);

        let initial_frames = app.doc.pages[0].frames.len();
        let new_id = app
            .create_frame_from_drag(0, Tool::ShapeFrame, Rect::new(50.0, 60.0, 140.0, 90.0))
            .expect("should create shape frame");
        assert_eq!(app.doc.pages[0].frames.len(), initial_frames + 1);
        assert_eq!(app.selected_frame_id, Some(new_id));
        assert!(app.history.can_undo());

        assert!(app.undo());
        assert_eq!(app.doc.pages[0].frames.len(), initial_frames);
        assert!(app.history.can_redo());

        assert!(app.redo());
        assert_eq!(app.doc.pages[0].frames.len(), initial_frames + 1);
    }

    #[test]
    fn frame_creation_and_story_threading_via_link_tool_work() {
        let mut app = BookCreatorApp::new();
        app.new_blank_book();
        assert_eq!(app.doc.pages.len(), 1);

        let f1 = app
            .create_frame_from_drag(0, Tool::TextFrame, Rect::new(36.0, 36.0, 180.0, 80.0))
            .unwrap();
        let f2 = app
            .create_frame_from_drag(0, Tool::TextFrame, Rect::new(36.0, 150.0, 180.0, 120.0))
            .unwrap();

        app.select_tool(Tool::LinkText);
        // First click selects source text frame
        assert!(!app.handle_link_tool_click(f1));
        assert_eq!(app.link_source_frame_id, Some(f1));

        // Second click links f1 -> f2 and returns to SelectMove tool
        assert!(app.handle_link_tool_click(f2));
        assert_eq!(app.link_source_frame_id, None);
        assert_eq!(app.active_tool, Tool::SelectMove);

        // Verify both frames share the same StoryId and sequential thread_order
        let story_id = match app.doc.pages[0]
            .frames
            .iter()
            .find(|f| f.id == f1)
            .unwrap()
            .kind
        {
            FrameKind::Text { story_id, .. } => story_id,
            _ => panic!("expected text frame"),
        };
        let chain = app.doc.threaded_frames_for_story(story_id);
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[0].1, f1);
        assert_eq!(chain[1].1, f2);
    }

    #[test]
    fn smart_alignment_snapping_and_resize_handles_work() {
        let app = BookCreatorApp::new();
        let page = &app.doc.pages[0];
        let setup = &app.doc.setup;
        let moving_id = page.frames[0].id;

        // Place a candidate rectangle whose center X is within 3pt of page center (216pt on 6x9)
        let candidate = Rect::new(118.0, 100.0, 200.0, 100.0); // center.x = 218.0, page_center_x = 216.0
        let (snapped, guides) = compute_smart_snap(page, setup, 0, moving_id, candidate, 5.0);
        assert!((snapped.center().x - setup.width_pt * 0.5).abs() < 0.01);
        assert!(!guides.is_empty());

        // Test 8-handle resize delta application and minimum size enforcement
        let orig = Rect::new(50.0, 50.0, 120.0, 80.0);
        let resized = ResizeHandle::SouthEast.apply_delta(orig, 30.0, 20.0, 24.0);
        assert_eq!(resized, Rect::new(50.0, 50.0, 150.0, 100.0));

        // Hit testing selects topmost frame in z-order
        let hit = hit_test_frame(page, Point::new(100.0, 100.0));
        assert!(hit.is_some());
    }

    #[test]
    fn page_operations_and_relative_image_import_work() {
        let mut app = BookCreatorApp::new();
        app.new_blank_book();
        app.add_page();
        assert_eq!(app.doc.pages.len(), 2);
        assert_eq!(app.current_page, 1);

        app.move_current_page_up();
        assert_eq!(app.current_page, 0);

        app.duplicate_current_page();
        assert_eq!(app.doc.pages.len(), 3);

        // Import an image with an OS path and verify stored path is strictly relative
        let img_frame_id = app.import_image_bytes_into_doc(
            "/Users/macbook/Pictures/illustration.png",
            vec![137, 80, 78, 71],
            None,
            Some(Point::new(200.0, 250.0)),
        );
        assert_eq!(app.selected_frame_id, Some(img_frame_id));
        for asset in app.doc.assets.values() {
            assert!(asset.relative_path.starts_with("assets/images/"));
            assert!(!asset.relative_path.contains("/Users/"));
        }
    }
}
