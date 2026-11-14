//! Core document model, frame geometry, rich-text stories, undo/redo history,
//! and portable `.book` (ZIP) / `.bookdir` (directory) bundle I/O.
//!
//! All spatial coordinates are stored in typographic points (`1.0 pt = 1/72 inch`).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

pub type PageId = Uuid;
pub type MasterPageId = Uuid;
pub type FrameId = Uuid;
pub type StoryId = Uuid;
pub type AssetId = String;

#[derive(Debug, Error)]
pub enum BookError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("ZIP archive error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("Asset not found: {0}")]
    AssetNotFound(String),
    #[error("Invalid book bundle: {0}")]
    InvalidBundle(String),
}

/// 2D point in typographic points (`1 pt = 1/72 in`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Axis-aligned rectangle in typographic points (`1 pt = 1/72 in`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    pub fn center(&self) -> Point {
        Point::new(self.x + self.width * 0.5, self.y + self.height * 0.5)
    }

    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x <= self.right() && p.y >= self.y && p.y <= self.bottom()
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.right()
            && self.right() > other.x
            && self.y < other.bottom()
            && self.bottom() > other.y
    }

    pub fn expand(&self, padding: f32) -> Rect {
        Rect::new(
            self.x - padding,
            self.y - padding,
            (self.width + padding * 2.0).max(0.0),
            (self.height + padding * 2.0).max(0.0),
        )
    }
}

/// sRGB color with alpha (`0..=255`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorRgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl ColorRgba {
    pub const BLACK: Self = Self:: rgb(20, 22, 28);
    pub const WHITE: Self = Self::rgb(255, 255, 255);
    pub const TRANSPARENT: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };
    pub const ACCENT_BLUE: Self = Self::rgb(45, 114, 210);
    pub const WARM_GOLD: Self = Self::rgb(232, 168, 56);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

impl Default for ColorRgba {
    fn default() -> Self {
        Self::BLACK
    }
}

/// Standard print & eBook page size presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PagePreset {
    /// 6 × 9 in (432 × 648 pt) — Standard Trade Paperback
    UsTrade6x9,
    /// 8.5 × 11 in (612 × 792 pt) — Textbooks, Cookbooks, Manuals
    UsLetter,
    /// 8.5 × 8.5 in (612 × 612 pt) — Illustrated Children's & Art Books
    Square85x85,
    /// 5.5 × 8.5 in (396 × 612 pt) — Digest / Zine
    Digest55x85,
    /// 210 × 297 mm (595.28 × 841.89 pt)
    IsoA4,
    /// 148 × 210 mm (419.53 × 595.28 pt)
    IsoA5,
    /// Custom dimensions in points
    Custom,
}

impl PagePreset {
    pub fn dimensions_pt(&self) -> (f32, f32) {
        match self {
            PagePreset::UsTrade6x9 => (432.0, 648.0),
            PagePreset::UsLetter => (612.0, 792.0),
            PagePreset::Square85x85 => (612.0, 612.0),
            PagePreset::Digest55x85 => (396.0, 612.0),
            PagePreset::IsoA4 => (595.28, 841.89),
            PagePreset::IsoA5 => (419.53, 595.28),
            PagePreset::Custom => (432.0, 648.0),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            PagePreset::UsTrade6x9 => "US Trade (6 × 9 in)",
            PagePreset::UsLetter => "US Letter (8.5 × 11 in)",
            PagePreset::Square85x85 => "Square Illustrated (8.5 × 8.5 in)",
            PagePreset::Digest55x85 => "Digest / Zine (5.5 × 8.5 in)",
            PagePreset::IsoA4 => "ISO A4 (210 × 297 mm)",
            PagePreset::IsoA5 => "ISO A5 (148 × 210 mm)",
            PagePreset::Custom => "Custom Size",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Margins {
    pub top: f32,
    pub bottom: f32,
    pub inside: f32,
    pub outside: f32,
}

impl Default for Margins {
    fn default() -> Self {
        Self {
            top: 36.0,     // 0.5 in
            bottom: 36.0,  // 0.5 in
            inside: 45.0,  // 0.625 in gutter
            outside: 36.0, // 0.5 in
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PageSetup {
    pub preset: PagePreset,
    pub width_pt: f32,
    pub height_pt: f32,
    pub margins: Margins,
    /// Print bleed around trim edges in points (standard 0.125 in = 9.0 pt).
    pub bleed_pt: f32,
    pub facing_pages: bool,
}

impl PageSetup {
    pub fn from_preset(preset: PagePreset) -> Self {
        let (width_pt, height_pt) = preset.dimensions_pt();
        Self {
            preset,
            width_pt,
            height_pt,
            margins: Margins::default(),
            bleed_pt: 9.0,
            facing_pages: false,
        }
    }

    pub fn safe_margin_rect(&self, page_index: usize) -> Rect {
        let left = if self.facing_pages && page_index % 2 == 1 {
            self.margins.outside
        } else {
            self.margins.inside
        };
        let right = if self.facing_pages && page_index % 2 == 1 {
            self.margins.inside
        } else {
            self.margins.outside
        };
        Rect::new(
            left,
            self.margins.top,
            (self.width_pt - left - right).max(10.0),
            (self.height_pt - self.margins.top - self.margins.bottom).max(10.0),
        )
    }
}

impl Default for PageSetup {
    fn default() -> Self {
        Self::from_preset(PagePreset::UsTrade6x9)
    }
}

/// Controls how neighboring text frames wrap ("runaround") around this frame.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum WrapMode {
    /// Text flows directly underneath/over this frame without wrapping.
    None,
    /// Text wraps around the frame's expanded bounding box on left/right sides.
    BoundingBox,
    /// Text stops above this frame and resumes below it (no side-by-side text).
    JumpObject,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WrapSettings {
    pub mode: WrapMode,
    pub padding_pt: f32,
}

impl Default for WrapSettings {
    fn default() -> Self {
        Self {
            mode: WrapMode::None,
            padding_pt: 8.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAlignment {
    Left,
    Center,
    Right,
    Justified,
}

impl Default for TextAlignment {
    fn default() -> Self {
        Self::Left
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextStyle {
    pub font_family: String,
    pub font_size_pt: f32,
    pub line_height_factor: f32,
    pub bold: bool,
    pub italic: bool,
    pub color: ColorRgba,
    pub alignment: TextAlignment,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_family: "Literata".to_string(),
            font_size_pt: 12.0,
            line_height_factor: 1.35,
            bold: false,
            italic: false,
            color: ColorRgba::BLACK,
            alignment: TextAlignment::Left,
        }
    }
}

/// A story holds text content that can flow across one or more linked `FrameKind::Text` frames.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Story {
    pub id: StoryId,
    pub title: String,
    pub text: String,
    pub style: TextStyle,
}

impl Story {
    pub fn new(title: impl Into<String>, text: impl Into<String>, style: TextStyle) -> Self {
        Self {
            id: Uuid::new_v4(),
            title: title.into(),
            text: text.into(),
            style,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageFit {
    /// Scale proportionally to fit inside frame.
    Contain,
    /// Scale proportionally to fill entire frame (cropping overflow).
    Cover,
    /// Stretch to exact frame dimensions.
    Stretch,
}

impl Default for ImageFit {
    fn default() -> Self {
        Self::Cover
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeKind {
    Rectangle,
    RoundedRectangle,
    Ellipse,
    DividerLine,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FrameKind {
    Text {
        story_id: StoryId,
        /// Order of this frame in the story's thread chain (`0, 1, 2...`).
        thread_order: usize,
        padding_pt: f32,
        background: Option<ColorRgba>,
    },
    Image {
        asset_id: Option<AssetId>,
        fit: ImageFit,
        caption: String,
        corner_radius_pt: f32,
    },
    Shape {
        kind: ShapeKind,
        fill: ColorRgba,
        stroke_color: ColorRgba,
        stroke_width_pt: f32,
        corner_radius_pt: f32,
    },
}

/// An individual layout object positioned on a `Page` or `MasterPage`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub id: FrameId,
    pub label: String,
    pub rect: Rect,
    pub rotation_deg: f32,
    pub z_index: i32,
    pub locked: bool,
    pub wrap: WrapSettings,
    pub kind: FrameKind,
}

impl Frame {
    pub fn new_text(rect: Rect, story_id: StoryId, thread_order: usize) -> Self {
        Self {
            id: Uuid::new_v4(),
            label: "Text Frame".to_string(),
            rect,
            rotation_deg: 0.0,
            z_index: 10,
            locked: false,
            wrap: WrapSettings::default(),
            kind: FrameKind::Text {
                story_id,
                thread_order,
                padding_pt: 6.0,
                background: None,
            },
        }
    }

    pub fn new_image(rect: Rect, asset_id: Option<AssetId>) -> Self {
        Self {
            id: Uuid::new_v4(),
            label: "Image Frame".to_string(),
            rect,
            rotation_deg: 0.0,
            z_index: 20,
            locked: false,
            wrap: WrapSettings {
                mode: WrapMode::BoundingBox,
                padding_pt: 10.0,
            },
            kind: FrameKind::Image {
                asset_id,
                fit: ImageFit::Cover,
                caption: String::new(),
                corner_radius_pt: 4.0,
            },
        }
    }

    pub fn new_shape(rect: Rect, kind: ShapeKind, fill: ColorRgba) -> Self {
        Self {
            id: Uuid::new_v4(),
            label: "Shape Frame".to_string(),
            rect,
            rotation_deg: 0.0,
            z_index: 5,
            locked: false,
            wrap: WrapSettings::default(),
            kind: FrameKind::Shape {
                kind,
                fill,
                stroke_color: ColorRgba::rgb(40, 44, 52),
                stroke_width_pt: 1.0,
                corner_radius_pt: 8.0,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MasterPage {
    pub id: MasterPageId,
    pub name: String,
    pub background_color: ColorRgba,
    pub show_page_numbers: bool,
    pub header_text: String,
    pub frames: Vec<Frame>,
}

impl Default for MasterPage {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: "A-Master (Standard)".to_string(),
            background_color: ColorRgba::WHITE,
            show_page_numbers: true,
            header_text: String::new(),
            frames: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Page {
    pub id: PageId,
    pub title: String,
    pub master_id: Option<MasterPageId>,
    pub background_color: ColorRgba,
    pub frames: Vec<Frame>,
}

impl Page {
    pub fn new(title: impl Into<String>, master_id: Option<MasterPageId>) -> Self {
        Self {
            id: Uuid::new_v4(),
            title: title.into(),
            master_id,
            background_color: ColorRgba::WHITE,
            frames: Vec::new(),
        }
    }

    pub fn frames_sorted_by_z(&self) -> Vec<&Frame> {
        let mut refs: Vec<&Frame> = self.frames.iter().collect();
        refs.sort_by_key(|f| f.z_index);
        refs
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetKind {
    Image,
    Font,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetEntry {
    pub id: AssetId,
    pub kind: AssetKind,
    /// Relative path inside the `.book` bundle (`assets/images/...` or `assets/fonts/...`).
    pub relative_path: String,
    pub original_filename: String,
    pub mime_type: String,
    #[serde(default)]
    pub pixel_dimensions: Option<(u32, u32)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BookMetadata {
    pub format_version: u32,
    pub title: String,
    pub subtitle: String,
    pub author: String,
    pub language: String,
    pub isbn: String,
    pub publisher: String,
}

impl Default for BookMetadata {
    fn default() -> Self {
        Self {
            format_version: 1,
            title: "Untitled Book".to_string(),
            subtitle: String::new(),
            author: "Author Name".to_string(),
            language: "en".to_string(),
            isbn: String::new(),
            publisher: String::new(),
        }
    }
}

/// Complete, self-contained book project state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BookDocument {
    pub metadata: BookMetadata,
    pub setup: PageSetup,
    pub master_pages: Vec<MasterPage>,
    pub pages: Vec<Page>,
    pub stories: BTreeMap<StoryId, Story>,
    pub assets: BTreeMap<AssetId, AssetEntry>,
    /// In-memory binary blobs keyed by `AssetId` so portable `.book` bundles
    /// carry all images & fonts cleanly between Arch Linux and Mac M1 Air.
    #[serde(skip)]
    pub asset_bytes: BTreeMap<AssetId, Vec<u8>>,
}

impl Default for BookDocument {
    fn default() -> Self {
        let master = MasterPage::default();
        let master_id = master.id;
        Self {
            metadata: BookMetadata::default(),
            setup: PageSetup::default(),
            master_pages: vec![master],
            pages: vec![Page::new("Page 1", Some(master_id))],
            stories: BTreeMap::new(),
            assets: BTreeMap::new(),
            asset_bytes: BTreeMap::new(),
        }
    }
}

impl BookDocument {
    /// Add or update a story and return its `StoryId`.
    pub fn insert_story(&mut self, story: Story) -> StoryId {
        let id = story.id;
        self.stories.insert(id, story);
        id
    }

    /// Return all text frames linked to `story_id` across the entire book,
    /// ordered by `(thread_order, page_index)`.
    pub fn threaded_frames_for_story(&self, story_id: StoryId) -> Vec<(usize, FrameId, Rect)> {
        let mut chain = Vec::new();
        for (page_idx, page) in self.pages.iter().enumerate() {
            for frame in &page.frames {
                if let FrameKind::Text {
                    story_id: s_id,
                    thread_order,
                    ..
                } = frame.kind
                {
                    if s_id == story_id {
                        chain.push((thread_order, page_idx, frame.id, frame.rect));
                    }
                }
            }
        }
        chain.sort_by_key(|(order, page_idx, _, _)| (*order, *page_idx));
        chain
            .into_iter()
            .map(|(_, page_idx, frame_id, rect)| (page_idx, frame_id, rect))
            .collect()
    }

    /// Link `source_frame_id` to `target_frame_id` so text overflows from source into target.
    pub fn link_text_frames(&mut self, source_frame_id: FrameId, target_frame_id: FrameId) -> bool {
        if source_frame_id == target_frame_id {
            return false;
        }
        let mut source_info = None;
        for page in &self.pages {
            for frame in &page.frames {
                if frame.id == source_frame_id {
                    if let FrameKind::Text {
                        story_id,
                        thread_order,
                        ..
                    } = frame.kind
                    {
                        source_info = Some((story_id, thread_order));
                    }
                }
            }
        }
        let Some((story_id, src_order)) = source_info else {
            return false;
        };

        for page in &mut self.pages {
            for frame in &mut page.frames {
                if frame.id == target_frame_id {
                    if let FrameKind::Text {
                        story_id: ref mut dst_story,
                        thread_order: ref mut dst_order,
                        ..
                    } = frame.kind
                    {
                        *dst_story = story_id;
                        *dst_order = src_order + 1;
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Register an image or font asset in the portable bundle using relative paths only.
    pub fn add_asset_bytes(
        &mut self,
        id: impl Into<String>,
        kind: AssetKind,
        filename: impl Into<String>,
        mime_type: impl Into<String>,
        bytes: Vec<u8>,
        pixel_dimensions: Option<(u32, u32)>,
    ) -> AssetId {
        let id = id.into();
        let filename_str = Path::new(&filename.into())
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("asset.bin")
            .to_string();
        let subdir = match kind {
            AssetKind::Image => "assets/images",
            AssetKind::Font => "assets/fonts",
        };
        let relative_path = format!("{subdir}/{id}_{filename_str}");
        self.assets.insert(
            id.clone(),
            AssetEntry {
                id: id.clone(),
                kind,
                relative_path,
                original_filename: filename_str,
                mime_type: mime_type.into(),
                pixel_dimensions,
            },
        );
        self.asset_bytes.insert(id.clone(), bytes);
        id
    }

    /// Save as a single-file portable `.book` ZIP archive.
    pub fn save_book_archive(&self, path: impl AsRef<Path>) -> Result<(), BookError> {
        let file = File::create(path)?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        let manifest_json = serde_json::to_vec_pretty(&self.metadata)?;
        zip.start_file("manifest.json", options)?;
        zip.write_all(&manifest_json)?;

        let doc_json = serde_json::to_vec_pretty(self)?;
        zip.start_file("document.json", options)?;
        zip.write_all(&doc_json)?;

        for (asset_id, entry) in &self.assets {
            if let Some(bytes) = self.asset_bytes.get(asset_id) {
                zip.start_file(&entry.relative_path, options)?;
                zip.write_all(bytes)?;
            }
        }

        zip.finish()?;
        Ok(())
    }

    /// Load a single-file portable `.book` ZIP archive.
    pub fn load_book_archive(path: impl AsRef<Path>) -> Result<Self, BookError> {
        let file = File::open(path)?;
        let mut archive = zip::ZipArchive::new(file)?;

        let mut doc_bytes = Vec::new();
        {
            let mut doc_file = archive
                .by_name("document.json")
                .map_err(|_| BookError::InvalidBundle("Missing document.json".into()))?;
            doc_file.read_to_end(&mut doc_bytes)?;
        }

        let mut doc: BookDocument = serde_json::from_slice(&doc_bytes)?;
        let asset_entries: Vec<(AssetId, String)> = doc
            .assets
            .iter()
            .map(|(k, v)| (k.clone(), v.relative_path.clone()))
            .collect();

        for (asset_id, rel_path) in asset_entries {
            if let Ok(mut asset_file) = archive.by_name(&rel_path) {
                let mut buf = Vec::new();
                asset_file.read_to_end(&mut buf)?;
                doc.asset_bytes.insert(asset_id, buf);
            }
        }

        Ok(doc)
    }

    /// Save as an unpacked, Git-friendly `.bookdir` directory.
    pub fn save_book_directory(&self, dir: impl AsRef<Path>) -> Result<(), BookError> {
        let dir = dir.as_ref();
        fs::create_dir_all(dir.join("assets/images"))?;
        fs::create_dir_all(dir.join("assets/fonts"))?;

        fs::write(
            dir.join("manifest.json"),
            serde_json::to_vec_pretty(&self.metadata)?,
        )?;
        fs::write(dir.join("document.json"), serde_json::to_vec_pretty(self)?)?;

        for (asset_id, entry) in &self.assets {
            if let Some(bytes) = self.asset_bytes.get(asset_id) {
                let target: PathBuf = dir.join(&entry.relative_path);
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(target, bytes)?;
            }
        }
        Ok(())
    }

    /// Load from an unpacked `.bookdir` directory.
    pub fn load_book_directory(dir: impl AsRef<Path>) -> Result<Self, BookError> {
        let dir = dir.as_ref();
        let doc_bytes = fs::read(dir.join("document.json"))?;
        let mut doc: BookDocument = serde_json::from_slice(&doc_bytes)?;

        for (asset_id, entry) in &doc.assets {
            let asset_path = dir.join(&entry.relative_path);
            if asset_path.exists() {
                doc.asset_bytes
                    .insert(asset_id.clone(), fs::read(asset_path)?);
            }
        }
        Ok(doc)
    }
}

/// Undo / Redo snapshot history for non-destructive editing.
#[derive(Debug, Clone)]
pub struct CommandHistory {
    undo_stack: Vec<BookDocument>,
    redo_stack: Vec<BookDocument>,
    max_entries: usize,
}

impl Default for CommandHistory {
    fn default() -> Self {
        Self::new(64)
    }
}

impl CommandHistory {
    pub fn new(max_entries: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_entries: max_entries.max(4),
        }
    }

    pub fn push_snapshot(&mut self, current: &BookDocument) {
        if self.undo_stack.len() >= self.max_entries {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(current.clone());
        self.redo_stack.clear();
    }

    pub fn undo(&mut self, current: &BookDocument) -> Option<BookDocument> {
        let prev = self.undo_stack.pop()?;
        self.redo_stack.push(current.clone());
        Some(prev)
    }

    pub fn redo(&mut self, current: &BookDocument) -> Option<BookDocument> {
        let next = self.redo_stack.pop()?;
        self.undo_stack.push(current.clone());
        Some(next)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }
}

/// Pre-flight check severity levels (inspired by InDesign Live Preflight & Scribus Verifier).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreflightSeverity {
    Info,
    Warning,
    Error,
}

/// Actionable pre-flight diagnostic with 1-click jump targets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreflightIssue {
    pub severity: PreflightSeverity,
    pub code: String,
    pub title: String,
    pub explanation: String,
    pub how_to_fix: String,
    pub page_index: Option<usize>,
    pub frame_id: Option<FrameId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_book_archive_and_directory_preserves_assets_and_threads() {
        let mut doc = BookDocument::default();
        doc.metadata.title = "Cross-Platform Test Book".into();
        let story = Story::new(
            "Chapter 1",
            "Hello from Arch Linux and Mac M1 Air!",
            TextStyle::default(),
        );
        let story_id = doc.insert_story(story);

        let f1 = Frame::new_text(Rect::new(36.0, 36.0, 200.0, 120.0), story_id, 0);
        let f2 = Frame::new_text(Rect::new(36.0, 200.0, 200.0, 120.0), Uuid::new_v4(), 0);
        let f1_id = f1.id;
        let f2_id = f2.id;
        doc.pages[0].frames.push(f1);
        doc.pages[0].frames.push(f2);
        assert!(doc.link_text_frames(f1_id, f2_id));
        assert_eq!(doc.threaded_frames_for_story(story_id).len(), 2);

        doc.add_asset_bytes(
            "cover_img",
            AssetKind::Image,
            "/home/user/Pictures/cover.png",
            "image/png",
            vec![137, 80, 78, 71],
            Some((1200, 1600)),
        );

        let tmp = tempfile::tempdir().unwrap();
        let archive_path = tmp.path().join("sample.book");
        doc.save_book_archive(&archive_path).unwrap();

        let loaded = BookDocument::load_book_archive(&archive_path).unwrap();
        assert_eq!(loaded.metadata.title, "Cross-Platform Test Book");
        assert_eq!(loaded.asset_bytes.get("cover_img").unwrap(), &vec![137, 80, 78, 71]);
        // Ensure relative path stripped any OS prefix
        assert_eq!(
            loaded.assets.get("cover_img").unwrap().relative_path,
            "assets/images/cover_img_cover.png"
        );

        let dir_path = tmp.path().join("sample.bookdir");
        loaded.save_book_directory(&dir_path).unwrap();
        let dir_loaded = BookDocument::load_book_directory(&dir_path).unwrap();
        assert_eq!(dir_loaded.threaded_frames_for_story(story_id).len(), 2);
    }
}
