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

/// Standard print-on-demand paper stock thickness for calculating spine width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaperStock {
    /// Standard white uncoated paper (`0.002252 in` / page).
    White,
    /// Cream / natural book paper (`0.0025 in` / page).
    Cream,
    /// Coated color-illustrated paper (`0.002347 in` / page).
    ColorIllustrated,
}

impl Default for PaperStock {
    fn default() -> Self {
        Self::White
    }
}

impl PaperStock {
    /// Thickness of a single page leaf side in inches for standard print-on-demand presses.
    pub const fn thickness_per_page_in(self) -> f32 {
        match self {
            PaperStock::White => 0.002252,
            PaperStock::Cream => 0.0025,
            PaperStock::ColorIllustrated => 0.002347,
        }
    }

    /// Thickness of a single page in typographic points (`1 in = 72 pt`).
    pub const fn thickness_per_page_pt(self) -> f32 {
        self.thickness_per_page_in() * 72.0
    }

    /// Exact spine width in typographic points (`1 in = 72 pt`) for a given `page_count`.
    pub const fn spine_width_pt(self, page_count: usize) -> f32 {
        (page_count as f32) * self.thickness_per_page_pt()
    }

    /// Human-readable label with thickness per page.
    pub const fn label(self) -> &'static str {
        match self {
            PaperStock::White => "White (0.002252 in/pg)",
            PaperStock::Cream => "Cream (0.0025 in/pg)",
            PaperStock::ColorIllustrated => "Color Illustrated (0.002347 in/pg)",
        }
    }

    /// Short display label.
    pub const fn short_label(self) -> &'static str {
        match self {
            PaperStock::White => "White",
            PaperStock::Cream => "Cream",
            PaperStock::ColorIllustrated => "Color Illustrated",
        }
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

    /// Calculate realistic print-on-demand spine width in typographic points (`1 in = 72 pt`).
    /// Uses `self.pages.len().max(24)` so short drafts still display a minimum printable spine.
    pub fn calculate_spine_width_pt(&self, paper: PaperStock) -> f32 {
        paper.spine_width_pt(self.pages.len().max(24))
    }

    /// Format the book's ISBN string for the standard Back Cover barcode box.
    fn formatted_isbn_barcode_line(&self) -> String {
        let raw = self.metadata.isbn.trim();
        if raw.is_empty() {
            "ISBN 978-0-000000-00-0 | 90000".to_string()
        } else if raw.to_uppercase().starts_with("ISBN") {
            if raw.contains('|') {
                raw.to_string()
            } else {
                format!("{raw} | 90000")
            }
        } else {
            format!("ISBN {raw} | 90000")
        }
    }

    /// 1-click Front & Back Cover generator:
    /// - Ensures Page 1 (`index 0`) is a Front Cover (`master_id = None` so page numbers/headers are suppressed),
    ///   populating it with a full-bleed background, Title/Subtitle frame, Hero Cover Art frame, and Author footer
    ///   if it does not yet have a Front Cover layout.
    /// - Appends (or updates) a dedicated **Back Cover** as the final page (`master_id = None`) with:
    ///   1. Full-bleed background `FrameKind::Shape` covering `(-bleed, -bleed, width + 2*bleed, height + 2*bleed)` (`z_index = 1`).
    ///   2. **Back Cover Hook & Blurb** `FrameKind::Text` in the upper safe area using `title` & `subtitle`.
    ///   3. **Author Photo** `FrameKind::Image` + **About the Author** `FrameKind::Text` in the middle band using `author`.
    ///   4. Standard **ISBN Barcode Box** (`144.0 x 86.4 pt` = `2.0 x 1.2 in` crisp white `ShapeKind::Rectangle` +
    ///      barcode label `FrameKind::Text` displaying `self.metadata.isbn` or `"ISBN 978-0-000000-00-0 | 90000"`)
    ///      in the bottom-right safe area.
    /// - Returns `(front_cover_page_index, back_cover_page_index)`.
    pub fn generate_front_and_back_covers(&mut self) -> (usize, usize) {
        let w = self.setup.width_pt.max(200.0);
        let h = self.setup.height_pt.max(260.0);
        let bleed = self.setup.bleed_pt.max(0.0);
        let margin_x = self.setup.margins.outside.clamp(24.0, 54.0);
        let margin_y = self.setup.margins.top.clamp(24.0, 54.0);
        let safe_w = (w - margin_x * 2.0).max(140.0);

        let title = if self.metadata.title.trim().is_empty() {
            "Untitled Book".to_string()
        } else {
            self.metadata.title.trim().to_string()
        };
        let subtitle = if self.metadata.subtitle.trim().is_empty() {
            "A Visual Print & Digital Edition".to_string()
        } else {
            self.metadata.subtitle.trim().to_string()
        };
        let author = if self.metadata.author.trim().is_empty() {
            "Author Name".to_string()
        } else {
            self.metadata.author.trim().to_string()
        };
        let publisher = self.metadata.publisher.trim().to_string();

        if self.pages.is_empty() {
            self.pages.push(Page::new("1. Front Cover", None));
        }

        // --- 1. Ensure Page 1 (index 0) is a Front Cover with `master_id = None` ---
        self.pages[0].master_id = None;
        if self.pages[0].title == "Page 1" || self.pages[0].title.trim().is_empty() {
            self.pages[0].title = "1. Front Cover".to_string();
        }

        let full_bleed_rect = Rect::new(-bleed, -bleed, w + 2.0 * bleed, h + 2.0 * bleed);

        if self.pages[0].frames.is_empty() {
            self.pages[0].background_color = ColorRgba::rgb(20, 34, 60);

            let mut fc_bg = Frame::new_shape(
                full_bleed_rect,
                ShapeKind::Rectangle,
                ColorRgba::rgb(20, 34, 60),
            );
            fc_bg.label = "Front Cover Full-Bleed Background".to_string();
            fc_bg.z_index = 1;
            if let FrameKind::Shape {
                ref mut stroke_width_pt,
                ref mut corner_radius_pt,
                ..
            } = fc_bg.kind
            {
                *stroke_width_pt = 0.0;
                *corner_radius_pt = 0.0;
            }
            self.pages[0].frames.push(fc_bg);

            let fc_title_story = Story::new(
                "Front Cover Title & Subtitle",
                format!("{}\n{}", title.to_uppercase(), subtitle),
                TextStyle {
                    font_family: "Literata".to_string(),
                    font_size_pt: 18.0,
                    line_height_factor: 1.3,
                    bold: true,
                    italic: false,
                    color: ColorRgba::WHITE,
                    alignment: TextAlignment::Center,
                },
            );
            let fc_title_id = self.insert_story(fc_title_story);
            let mut fc_title_frame = Frame::new_text(
                Rect::new(margin_x, margin_y + 12.0, safe_w, (h * 0.22).max(115.0)),
                fc_title_id,
                0,
            );
            fc_title_frame.label = "Cover Title & Subtitle".to_string();
            fc_title_frame.z_index = 15;
            self.pages[0].frames.push(fc_title_frame);

            let art_y = margin_y + (h * 0.25).max(135.0);
            let art_h = (h - art_y - margin_y - 92.0).max(140.0);
            let mut fc_art = Frame::new_image(
                Rect::new(margin_x + 12.0, art_y, (safe_w - 24.0).max(120.0), art_h),
                None,
            );
            fc_art.label = "Hero Cover Art".to_string();
            fc_art.z_index = 10;
            fc_art.wrap = WrapSettings {
                mode: WrapMode::None,
                padding_pt: 0.0,
            };
            if let FrameKind::Image {
                ref mut caption, ..
            } = fc_art.kind
            {
                *caption = "Hero Cover Art (Drop PNG/JPG Here)".to_string();
            }
            self.pages[0].frames.push(fc_art);

            let footer_line = if publisher.is_empty() {
                author.clone()
            } else {
                format!("{author}\n{publisher}")
            };
            let fc_author_story = Story::new(
                "Front Cover Author Footer",
                footer_line,
                TextStyle {
                    font_family: "Literata".to_string(),
                    font_size_pt: 13.0,
                    line_height_factor: 1.3,
                    bold: true,
                    italic: false,
                    color: ColorRgba::WARM_GOLD,
                    alignment: TextAlignment::Center,
                },
            );
            let fc_author_id = self.insert_story(fc_author_story);
            let mut fc_author_frame = Frame::new_text(
                Rect::new(margin_x, h - margin_y - 68.0, safe_w, 60.0),
                fc_author_id,
                0,
            );
            fc_author_frame.label = "Front Cover Author Name".to_string();
            fc_author_frame.z_index = 15;
            self.pages[0].frames.push(fc_author_frame);
        } else {
            // Ensure existing Front Cover also has a full-bleed background frame at z_index = 1
            let has_full_bleed = self.pages[0].frames.iter().any(|f| {
                f.z_index <= 1
                    && (f.rect.x - (-bleed)).abs() < 1.0
                    && (f.rect.y - (-bleed)).abs() < 1.0
            });
            if !has_full_bleed {
                let bg_col = self.pages[0].background_color;
                let mut fc_bg = Frame::new_shape(full_bleed_rect, ShapeKind::Rectangle, bg_col);
                fc_bg.label = "Front Cover Full-Bleed Background".to_string();
                fc_bg.z_index = 1;
                if let FrameKind::Shape {
                    ref mut stroke_width_pt,
                    ref mut corner_radius_pt,
                    ..
                } = fc_bg.kind
                {
                    *stroke_width_pt = 0.0;
                    *corner_radius_pt = 0.0;
                }
                self.pages[0].frames.insert(0, fc_bg);
            }
        }

        // --- 2. Append or update the dedicated Back Cover as the final page ---
        let back_cover_idx = if self.pages.len() > 1
            && self
                .pages
                .last()
                .map_or(false, |p| p.title.to_lowercase().contains("back cover"))
        {
            self.pages.len() - 1
        } else {
            let next_num = self.pages.len() + 1;
            self.pages
                .push(Page::new(format!("{next_num}. Back Cover"), None));
            self.pages.len() - 1
        };

        let mut back_frames = Vec::new();

        // (1) Full-bleed background FrameKind::Shape covering (-bleed, -bleed, width + 2*bleed, height + 2*bleed) with z_index = 1
        let mut bc_bg = Frame::new_shape(
            full_bleed_rect,
            ShapeKind::Rectangle,
            ColorRgba::rgb(20, 34, 60),
        );
        bc_bg.label = "Back Cover Full-Bleed Background".to_string();
        bc_bg.z_index = 1;
        if let FrameKind::Shape {
            ref mut stroke_width_pt,
            ref mut corner_radius_pt,
            ..
        } = bc_bg.kind
        {
            *stroke_width_pt = 0.0;
            *corner_radius_pt = 0.0;
        }
        back_frames.push(bc_bg);

        let mut gold_bar = Frame::new_shape(
            Rect::new(margin_x, margin_y, safe_w, 3.0),
            ShapeKind::DividerLine,
            ColorRgba::WARM_GOLD,
        );
        gold_bar.label = "Back Cover Accent Rule".to_string();
        gold_bar.z_index = 5;
        back_frames.push(gold_bar);

        // (2) Back Cover Hook & Blurb FrameKind::Text in the upper safe area
        let blurb_story = Story::new(
            "Back Cover Hook & Blurb",
            format!(
                "{}\n{}\n\n\
                \"A compelling, finely crafted edition designed for print and digital readers alike.\"\n\n\
                Double-click this frame to customize your back-cover hook, synopsis, and review quotes. \
                The background extends to the full {:.1} pt bleed while keeping all typography inside safe margins.",
                title.to_uppercase(),
                subtitle,
                bleed
            ),
            TextStyle {
                font_family: "Literata".to_string(),
                font_size_pt: 11.5,
                line_height_factor: 1.35,
                bold: false,
                italic: false,
                color: ColorRgba::WHITE,
                alignment: TextAlignment::Left,
            },
        );
        let blurb_story_id = self.insert_story(blurb_story);
        let blurb_y = margin_y + 14.0;
        let blurb_h = (h * 0.35).clamp(160.0, 235.0);
        let mut blurb_frame = Frame::new_text(
            Rect::new(margin_x, blurb_y, safe_w, blurb_h),
            blurb_story_id,
            0,
        );
        blurb_frame.label = "Back Cover Hook & Blurb".to_string();
        blurb_frame.z_index = 10;
        back_frames.push(blurb_frame);

        // (3) Author Photo FrameKind::Image + About the Author FrameKind::Text in the middle band
        let mid_y = blurb_y + blurb_h + 16.0;
        let photo_w = 88.0;
        let photo_h = 108.0;
        let mut author_photo =
            Frame::new_image(Rect::new(margin_x, mid_y, photo_w, photo_h), None);
        author_photo.label = "Author Photo".to_string();
        author_photo.z_index = 15;
        author_photo.wrap = WrapSettings {
            mode: WrapMode::None,
            padding_pt: 0.0,
        };
        if let FrameKind::Image {
            ref mut caption, ..
        } = author_photo.kind
        {
            *caption = "Author Photo".to_string();
        }
        back_frames.push(author_photo);

        let bio_story = Story::new(
            "About the Author",
            format!(
                "ABOUT THE AUTHOR\n{} is the author of {}. Double-click here to add your author bio, credentials, and website.",
                author, title
            ),
            TextStyle {
                font_family: "Literata".to_string(),
                font_size_pt: 10.5,
                line_height_factor: 1.35,
                bold: false,
                italic: false,
                color: ColorRgba::rgb(226, 232, 240),
                alignment: TextAlignment::Left,
            },
        );
        let bio_story_id = self.insert_story(bio_story);
        let bio_x = margin_x + photo_w + 14.0;
        let bio_w = (w - margin_x - bio_x).max(120.0);
        let mut bio_frame =
            Frame::new_text(Rect::new(bio_x, mid_y, bio_w, 124.0), bio_story_id, 0);
        bio_frame.label = "About the Author".to_string();
        bio_frame.z_index = 15;
        back_frames.push(bio_frame);

        // (4) Standard ISBN Barcode Box (144.0 x 86.4 pt = 2.0 x 1.2 in crisp white ShapeKind::Rectangle
        //     + barcode label FrameKind::Text) in the bottom-right safe area.
        let barcode_w = 144.0;
        let barcode_h = 86.4;
        let barcode_x = (w - margin_x - barcode_w).max(margin_x);
        let barcode_y = (h - margin_y - barcode_h).max(mid_y + 128.0);
        let barcode_rect = Rect::new(barcode_x, barcode_y, barcode_w, barcode_h);

        let mut barcode_box =
            Frame::new_shape(barcode_rect, ShapeKind::Rectangle, ColorRgba::WHITE);
        barcode_box.label = "ISBN Barcode Box".to_string();
        barcode_box.z_index = 20;
        barcode_box.wrap = WrapSettings {
            mode: WrapMode::None,
            padding_pt: 0.0,
        };
        if let FrameKind::Shape {
            ref mut stroke_color,
            ref mut stroke_width_pt,
            ref mut corner_radius_pt,
            ..
        } = barcode_box.kind
        {
            *stroke_color = ColorRgba::rgb(30, 41, 59);
            *stroke_width_pt = 1.0;
            *corner_radius_pt = 2.0;
        }
        back_frames.push(barcode_box);

        let isbn_line = self.formatted_isbn_barcode_line();
        let barcode_story = Story::new(
            "ISBN Barcode Label",
            format!("{isbn_line}\n||||||||||||||||||||||\n9 780000 000000  90000"),
            TextStyle {
                font_family: "JetBrains Mono".to_string(),
                font_size_pt: 8.0,
                line_height_factor: 1.25,
                bold: true,
                italic: false,
                color: ColorRgba::BLACK,
                alignment: TextAlignment::Center,
            },
        );
        let barcode_story_id = self.insert_story(barcode_story);
        let mut barcode_label = Frame::new_text(
            Rect::new(
                barcode_x + 4.0,
                barcode_y + 6.0,
                barcode_w - 8.0,
                barcode_h - 10.0,
            ),
            barcode_story_id,
            0,
        );
        barcode_label.label = "ISBN Barcode Label".to_string();
        barcode_label.z_index = 25;
        if let FrameKind::Text {
            ref mut padding_pt, ..
        } = barcode_label.kind
        {
            *padding_pt = 4.0;
        }
        back_frames.push(barcode_label);

        let back_page = &mut self.pages[back_cover_idx];
        back_page.master_id = None;
        back_page.background_color = ColorRgba::rgb(20, 34, 60);
        back_page.frames = back_frames;

        (0, back_cover_idx)
    }

    /// Appends a `"Wrap-Around Cover (Back + Spine + Front)"` page (`master_id = None`)
    /// laying out left-half Back Cover frames (blurb + ISBN barcode box), a center Spine strip
    /// with calculated spine width label, and right-half Front Cover frames.
    /// Returns the newly appended page index.
    pub fn generate_wraparound_cover_page(&mut self, paper: PaperStock) -> usize {
        let spine_pt = self.calculate_spine_width_pt(paper);
        let spine_in = spine_pt / 72.0;
        let spine_mm = spine_in * 25.4;

        let w = self.setup.width_pt.max(240.0);
        let h = self.setup.height_pt.max(260.0);
        let bleed = self.setup.bleed_pt.max(0.0);

        let title = if self.metadata.title.trim().is_empty() {
            "Untitled Book".to_string()
        } else {
            self.metadata.title.trim().to_string()
        };
        let subtitle = if self.metadata.subtitle.trim().is_empty() {
            "Print Wrap-Around Edition".to_string()
        } else {
            self.metadata.subtitle.trim().to_string()
        };
        let author = if self.metadata.author.trim().is_empty() {
            "Author Name".to_string()
        } else {
            self.metadata.author.trim().to_string()
        };

        let mut page = Page::new("Wrap-Around Cover (Back + Spine + Front)", None);
        page.background_color = ColorRgba::rgb(20, 34, 60);

        // 1. Full-bleed wrap-around background
        let mut bg = Frame::new_shape(
            Rect::new(-bleed, -bleed, w + 2.0 * bleed, h + 2.0 * bleed),
            ShapeKind::Rectangle,
            ColorRgba::rgb(20, 34, 60),
        );
        bg.label = "Wrap-Around Full-Bleed Background".to_string();
        bg.z_index = 1;
        if let FrameKind::Shape {
            ref mut stroke_width_pt,
            ref mut corner_radius_pt,
            ..
        } = bg.kind
        {
            *stroke_width_pt = 0.0;
            *corner_radius_pt = 0.0;
        }
        page.frames.push(bg);

        // Geometry for Left (Back Cover), Center (Spine), and Right (Front Cover)
        let visual_spine_w = spine_pt.clamp(32.0, 56.0);
        let spine_x = (w - visual_spine_w) * 0.5;
        let pad = 14.0;
        let left_w = (spine_x - pad * 2.0).max(110.0);
        let right_x = spine_x + visual_spine_w + pad;
        let right_w = (w - right_x - pad).max(110.0);

        // 2. Center Spine Strip & Spine Width Label
        let mut spine_strip = Frame::new_shape(
            Rect::new(spine_x, -bleed, visual_spine_w, h + 2.0 * bleed),
            ShapeKind::Rectangle,
            ColorRgba::rgb(12, 22, 42),
        );
        spine_strip.label = "Center Spine Strip".to_string();
        spine_strip.z_index = 5;
        if let FrameKind::Shape {
            ref mut stroke_color,
            ref mut stroke_width_pt,
            ref mut corner_radius_pt,
            ..
        } = spine_strip.kind
        {
            *stroke_color = ColorRgba::WARM_GOLD;
            *stroke_width_pt = 1.0;
            *corner_radius_pt = 0.0;
        }
        page.frames.push(spine_strip);

        let spine_story = Story::new(
            "Spine Width Label",
            format!(
                "SPINE\n{:.1} pt\n{:.3} in\n{:.1} mm\n({})\n\n{}\n{}",
                spine_pt,
                spine_in,
                spine_mm,
                paper.short_label(),
                title,
                author
            ),
            TextStyle {
                font_family: "Inter".to_string(),
                font_size_pt: 8.0,
                line_height_factor: 1.25,
                bold: true,
                italic: false,
                color: ColorRgba::WARM_GOLD,
                alignment: TextAlignment::Center,
            },
        );
        let spine_story_id = self.insert_story(spine_story);
        let mut spine_label = Frame::new_text(
            Rect::new(spine_x - 6.0, 28.0, visual_spine_w + 12.0, h - 56.0),
            spine_story_id,
            0,
        );
        spine_label.label = format!("Spine Label ({spine_pt:.1} pt / {spine_in:.3} in)");
        spine_label.z_index = 18;
        if let FrameKind::Text {
            ref mut padding_pt, ..
        } = spine_label.kind
        {
            *padding_pt = 2.0;
        }
        page.frames.push(spine_label);

        // 3. Left-Half: Back Cover Blurb + ISBN Barcode Box
        let back_blurb_story = Story::new(
            "Wrap-Around Back Cover Blurb",
            format!(
                "BACK COVER\n{}\n\n\
                Double-click to edit back-cover synopsis, hooks, and review quotes for your print jacket.\n\n\
                Author: {}",
                title.to_uppercase(),
                author
            ),
            TextStyle {
                font_family: "Literata".to_string(),
                font_size_pt: 10.0,
                line_height_factor: 1.3,
                bold: false,
                italic: false,
                color: ColorRgba::WHITE,
                alignment: TextAlignment::Left,
            },
        );
        let back_blurb_id = self.insert_story(back_blurb_story);
        let mut back_blurb_frame = Frame::new_text(
            Rect::new(pad, 36.0, left_w, (h * 0.52).max(200.0)),
            back_blurb_id,
            0,
        );
        back_blurb_frame.label = "Back Cover Hook & Blurb".to_string();
        back_blurb_frame.z_index = 10;
        page.frames.push(back_blurb_frame);

        let bc_box_w = left_w.min(144.0);
        let bc_box_h = 86.4;
        let bc_box_x = (spine_x - pad - bc_box_w).max(pad);
        let bc_box_y = (h - 36.0 - bc_box_h).max(340.0);
        let mut bc_box = Frame::new_shape(
            Rect::new(bc_box_x, bc_box_y, bc_box_w, bc_box_h),
            ShapeKind::Rectangle,
            ColorRgba::WHITE,
        );
        bc_box.label = "Wrap-Around ISBN Barcode Box".to_string();
        bc_box.z_index = 20;
        bc_box.wrap = WrapSettings {
            mode: WrapMode::None,
            padding_pt: 0.0,
        };
        page.frames.push(bc_box);

        let isbn_line = self.formatted_isbn_barcode_line();
        let bc_label_story = Story::new(
            "Wrap-Around ISBN Barcode Label",
            format!("{isbn_line}\n||||||||||||||||||||\n9 780000 000000"),
            TextStyle {
                font_family: "JetBrains Mono".to_string(),
                font_size_pt: 7.5,
                line_height_factor: 1.2,
                bold: true,
                italic: false,
                color: ColorRgba::BLACK,
                alignment: TextAlignment::Center,
            },
        );
        let bc_label_id = self.insert_story(bc_label_story);
        let mut bc_label_frame = Frame::new_text(
            Rect::new(bc_box_x + 3.0, bc_box_y + 5.0, bc_box_w - 6.0, bc_box_h - 8.0),
            bc_label_id,
            0,
        );
        bc_label_frame.label = "Wrap-Around ISBN Label".to_string();
        bc_label_frame.z_index = 25;
        if let FrameKind::Text {
            ref mut padding_pt, ..
        } = bc_label_frame.kind
        {
            *padding_pt = 2.0;
        }
        page.frames.push(bc_label_frame);

        // 4. Right-Half: Front Cover Title, Hero Art & Author Footer
        let front_title_story = Story::new(
            "Wrap-Around Front Cover Title",
            format!("FRONT COVER\n{}\n{}", title.to_uppercase(), subtitle),
            TextStyle {
                font_family: "Literata".to_string(),
                font_size_pt: 12.5,
                line_height_factor: 1.3,
                bold: true,
                italic: false,
                color: ColorRgba::WHITE,
                alignment: TextAlignment::Center,
            },
        );
        let front_title_id = self.insert_story(front_title_story);
        let mut front_title_frame = Frame::new_text(
            Rect::new(right_x, 36.0, right_w, 140.0),
            front_title_id,
            0,
        );
        front_title_frame.label = "Front Cover Title & Subtitle".to_string();
        front_title_frame.z_index = 15;
        page.frames.push(front_title_frame);

        let mut front_art = Frame::new_image(
            Rect::new(right_x, 190.0, right_w, (h - 300.0).max(140.0)),
            None,
        );
        front_art.label = "Front Cover Hero Art".to_string();
        front_art.z_index = 10;
        front_art.wrap = WrapSettings {
            mode: WrapMode::None,
            padding_pt: 0.0,
        };
        if let FrameKind::Image {
            ref mut caption, ..
        } = front_art.kind
        {
            *caption = "Front Cover Art".to_string();
        }
        page.frames.push(front_art);

        let front_author_story = Story::new(
            "Wrap-Around Front Cover Author",
            author,
            TextStyle {
                font_family: "Literata".to_string(),
                font_size_pt: 11.0,
                line_height_factor: 1.25,
                bold: true,
                italic: false,
                color: ColorRgba::WARM_GOLD,
                alignment: TextAlignment::Center,
            },
        );
        let front_author_id = self.insert_story(front_author_story);
        let mut front_author_frame = Frame::new_text(
            Rect::new(right_x, h - 92.0, right_w, 56.0),
            front_author_id,
            0,
        );
        front_author_frame.label = "Front Cover Author".to_string();
        front_author_frame.z_index = 15;
        page.frames.push(front_author_frame);

        self.pages.push(page);
        self.pages.len() - 1
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

    #[test]
    fn paper_stock_spine_width_and_cover_generators_work() {
        // 1. Verify exact PaperStock::spine_width_pt calculations
        let white_100 = PaperStock::White.spine_width_pt(100);
        assert!((white_100 - 100.0 * 0.002252 * 72.0).abs() < 1e-4);

        let cream_100 = PaperStock::Cream.spine_width_pt(100);
        assert!((cream_100 - 100.0 * 0.0025 * 72.0).abs() < 1e-4);

        let color_100 = PaperStock::ColorIllustrated.spine_width_pt(100);
        assert!((color_100 - 100.0 * 0.002347 * 72.0).abs() < 1e-4);

        // 2. Verify BookDocument::calculate_spine_width_pt uses pages.len().max(24)
        let mut doc = BookDocument::default();
        doc.metadata.title = "Rust Systems Publishing".to_string();
        doc.metadata.subtitle = "High-Speed Print & EPUB 3".to_string();
        doc.metadata.author = "Ada Lovelace".to_string();
        doc.metadata.isbn = "978-1-4920-0000-0".to_string();

        assert_eq!(doc.pages.len(), 1);
        assert!(
            (doc.calculate_spine_width_pt(PaperStock::Cream)
                - PaperStock::Cream.spine_width_pt(24))
            .abs()
                < 1e-4
        );

        // 3. Verify generate_front_and_back_covers on a blank 1-page book
        let (front_idx, back_idx) = doc.generate_front_and_back_covers();
        assert_eq!(front_idx, 0);
        assert_eq!(back_idx, 1);
        assert_eq!(doc.pages.len(), 2);
        assert_eq!(doc.pages[front_idx].master_id, None);
        assert_eq!(doc.pages[back_idx].master_id, None);
        assert!(!doc.pages[front_idx].frames.is_empty());

        let bleed = doc.setup.bleed_pt;
        let back_page = &doc.pages[back_idx];
        // Full-bleed background with z_index = 1
        assert!(back_page.frames.iter().any(|f| {
            f.z_index == 1
                && (f.rect.x - (-bleed)).abs() < 0.01
                && (f.rect.y - (-bleed)).abs() < 0.01
                && (f.rect.width - (doc.setup.width_pt + 2.0 * bleed)).abs() < 0.01
                && (f.rect.height - (doc.setup.height_pt + 2.0 * bleed)).abs() < 0.01
        }));
        // Back Cover Hook & Blurb frame
        assert!(back_page
            .frames
            .iter()
            .any(|f| f.label.contains("Hook & Blurb") && matches!(f.kind, FrameKind::Text { .. })));
        // Author Photo + About the Author
        assert!(back_page
            .frames
            .iter()
            .any(|f| f.label.contains("Author Photo") && matches!(f.kind, FrameKind::Image { .. })));
        assert!(back_page
            .frames
            .iter()
            .any(|f| f.label.contains("About the Author") && matches!(f.kind, FrameKind::Text { .. })));
        // Standard 144.0 x 86.4 pt (2.0 x 1.2 in) white ISBN Barcode Box
        assert!(back_page.frames.iter().any(|f| {
            f.label.contains("ISBN Barcode Box")
                && (f.rect.width - 144.0).abs() < 0.01
                && (f.rect.height - 86.4).abs() < 0.01
                && matches!(
                    f.kind,
                    FrameKind::Shape {
                        kind: ShapeKind::Rectangle,
                        fill,
                        ..
                    } if fill == ColorRgba::WHITE
                )
        }));

        // Calling generate_front_and_back_covers again updates Back Cover in place
        let (front_idx2, back_idx2) = doc.generate_front_and_back_covers();
        assert_eq!(front_idx2, 0);
        assert_eq!(back_idx2, 1);
        assert_eq!(doc.pages.len(), 2);

        // 4. Verify generate_wraparound_cover_page
        let wrap_idx = doc.generate_wraparound_cover_page(PaperStock::White);
        assert_eq!(wrap_idx, 2);
        assert_eq!(
            doc.pages[wrap_idx].title,
            "Wrap-Around Cover (Back + Spine + Front)"
        );
        assert_eq!(doc.pages[wrap_idx].master_id, None);
        assert!(doc.pages[wrap_idx]
            .frames
            .iter()
            .any(|f| f.label.contains("Spine")));
    }
}
