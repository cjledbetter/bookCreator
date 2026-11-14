//! Out-of-the-box searchable Help Manual, OS-aware shortcut guide (Mac M1 Air
//! vs Arch Linux), plain-English Preflight checker, and 4-page Interactive
//! Starter Tutorial Book generator.

use book_core::{
    BookDocument, ColorRgba, Frame, FrameKind, MasterPage, Page, PagePreset, PageSetup,
    PreflightIssue, PreflightSeverity, Rect, ShapeKind, Story, TextAlignment, TextStyle,
    WrapMode, WrapSettings,
};
use book_layout::ComputedBookLayout;

#[derive(Debug, Clone)]
pub struct HelpArticle {
    pub id: &'static str,
    pub category: &'static str,
    pub title: &'static str,
    pub summary: &'static str,
    pub steps: &'static [&'static str],
    pub keywords: &'static [&'static str],
}

/// Returns `"Cmd (⌘)"` on macOS (Mac M1 Air) and `"Ctrl"` on Arch Linux / Windows.
pub const fn primary_modifier_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "Cmd (⌘)"
    } else {
        "Ctrl"
    }
}

pub fn builtin_help_articles() -> Vec<HelpArticle> {
    vec![
        HelpArticle {
            id: "quick-start",
            category: "Getting Started",
            title: "1. Quick Start: Your First Book in 60 Seconds",
            summary: "How to place text, drag images, and export a print-ready PDF or EPUB 3 right out of the box.",
            steps: &[
                "Pick one of the 5 top toolbar tools: Select/Move (V), Text Frame (T), Image Frame (I), Shape (S), or Link Text (L).",
                "With Text Frame (T), click and drag on the page to create a frame, then double-click inside it to edit text directly on the canvas.",
                "With Image Frame (I) or Shape (S), drag a box onto the page—or drop any PNG/JPG file straight from Finder (macOS) or your Linux file manager.",
                "Use the Right Inspector to toggle 'Text Wrap (Runaround)' so your story flows smoothly around dragged images.",
                "Click 'Export PDF' for print-ready output (with optional bleed & crop marks) or 'Export EPUB' for Fixed-Layout or Reflowable eBooks.",
            ],
            keywords: &["start", "tutorial", "first", "beginner", "tools", "drag", "drop"],
        },
        HelpArticle {
            id: "text-wrap",
            category: "Layout & Typography",
            title: "2. Live Text Wrap: Moving Images Through Text",
            summary: "Make text automatically wrap around illustrations, callouts, and shapes as you drag them.",
            steps: &[
                "Select any Image Frame or Shape Frame on the canvas using the Select/Move (V) tool.",
                "In the Right Inspector under 'Text Wrap (Runaround)', choose 'Bounding Box' (wraps left/right) or 'Jump Object' (breaks text above and below).",
                "Adjust the 'Wrap Padding (pt)' slider to control the breathing room between the object and neighboring text.",
                "Drag the image across any Text Frame—the text reflows in real time at 60fps.",
            ],
            keywords: &["wrap", "runaround", "image", "reflow", "padding", "obstacle"],
        },
        HelpArticle {
            id: "threading-text",
            category: "Layout & Typography",
            title: "3. Threading Text Frames Across Pages",
            summary: "Link multiple text frames together so long chapters automatically flow from frame to frame and page to page.",
            steps: &[
                "When a Text Frame has more text than fits inside its box, a red '[+]' overflow port appears at its bottom-right corner.",
                "Select the 'Link / Thread Text (L)' tool in the top toolbar (or click the '[+]' port).",
                "Click the source Text Frame first, then click the destination Text Frame (on the same page or another page).",
                "Your story now flows continuously across both frames! Resizing the first frame automatically pushes or pulls lines into the second frame.",
            ],
            keywords: &["thread", "link", "overflow", "story", "chain", "pages", "flow"],
        },
        HelpArticle {
            id: "cross-platform-portable",
            category: "Portability (Mac M1 Air & Arch Linux)",
            title: "4. Moving Projects Between Mac M1 Air & Arch Linux",
            summary: "How the portable .book format guarantees identical layouts across macOS Apple Silicon and Linux.",
            steps: &[
                "Use 'Save .book' to package your entire project (pages, stories, images, and fonts) into a single self-contained .book archive.",
                "All asset paths inside a .book file are strictly relative (assets/images/*, assets/fonts/*)—never tied to /Users/ or /home/.",
                "Copy the .book file via Git, Syncthing, AirDrop, or USB between your Mac M1 Air and Arch Linux machine and open it directly.",
                "For Git version control of large books, you can also save/load unpacked '.bookdir' directories containing human-readable JSON.",
            ],
            keywords: &["mac", "m1", "linux", "arch", "portable", "book", "git", "import", "fonts"],
        },
        HelpArticle {
            id: "export-pdf-epub",
            category: "Export & Prepress",
            title: "5. Exporting Print-Ready PDF vs. EPUB 3",
            summary: "Equal-priority output for professional print presses and modern eReaders (Apple Books, Foliate, Calibre, Kobo).",
            steps: &[
                "Check the 'Preflight' badge in the top toolbar before exporting—it warns you if any text frame has unplaced overflow '[+]'.",
                "For Print PDF: Enable 'Include Bleed (9 pt / 0.125 in)' and 'Crop Marks' if sending to a commercial printer.",
                "For Layout-Heavy Books (Children's books, cookbooks, visual guides): Choose 'Fixed-Layout EPUB 3' so every frame's exact position is preserved.",
                "For Novel/Chapter eReaders: Choose 'Reflowable EPUB 3' to export clean, resizable HTML chapters in story thread order.",
            ],
            keywords: &["pdf", "epub", "print", "bleed", "crop", "fixed", "reflowable", "apple books"],
        },
    ]
}

pub fn search_help_articles(query: &str) -> Vec<HelpArticle> {
    let q = query.trim().to_lowercase();
    let all = builtin_help_articles();
    if q.is_empty() {
        return all;
    }
    all.into_iter()
        .filter(|a| {
            a.title.to_lowercase().contains(&q)
                || a.category.to_lowercase().contains(&q)
                || a.summary.to_lowercase().contains(&q)
                || a.steps.iter().any(|s| s.to_lowercase().contains(&q))
                || a.keywords.iter().any(|k| k.to_lowercase().contains(&q))
        })
        .collect()
}

/// Generates a rich 4-page layout-heavy Starter Tutorial Book so users can
/// immediately drag elements, test live text-wrap, and try PDF/EPUB export.
pub fn create_tutorial_book() -> BookDocument {
    let mut doc = BookDocument {
        setup: PageSetup::from_preset(PagePreset::UsTrade6x9),
        ..BookDocument::default()
    };
    doc.metadata.title = "Book Creator: Interactive Visual Guide".to_string();
    doc.metadata.subtitle = "Drag, Wrap, Thread & Export on Arch Linux and Mac M1 Air".to_string();
    doc.metadata.author = "Book Creator Studio".to_string();

    let master = MasterPage::default();
    let master_id = master.id;
    doc.master_pages = vec![master];
    doc.pages.clear();

    // --- PAGE 1: Cover Spread ---
    let mut page1 = Page::new("1. Cover", Some(master_id));
    page1.background_color = ColorRgba::rgb(248, 245, 238);

    let banner = Frame::new_shape(
        Rect::new(36.0, 36.0, 360.0, 180.0),
        ShapeKind::RoundedRectangle,
        ColorRgba::rgb(31, 58, 96),
    );
    page1.frames.push(banner);

    let cover_title_story = Story::new(
        "Cover Title",
        "BOOK CREATOR\nVisual Desktop Publishing in Rust\n\nDouble-click any text frame to edit, or drag frames to feel live snapping!",
        TextStyle {
            font_family: "Literata".to_string(),
            font_size_pt: 16.0,
            line_height_factor: 1.4,
            bold: true,
            italic: false,
            color: ColorRgba::WHITE,
            alignment: TextAlignment::Left,
        },
    );
    let cover_title_id = doc.insert_story(cover_title_story);
    let mut title_frame = Frame::new_text(Rect::new(54.0, 56.0, 324.0, 140.0), cover_title_id, 0);
    title_frame.label = "Cover Title Frame".to_string();
    title_frame.z_index = 15;
    page1.frames.push(title_frame);

    let mut hero_img = Frame::new_image(Rect::new(54.0, 246.0, 324.0, 210.0), None);
    hero_img.label = "Cover Illustration".to_string();
    if let FrameKind::Image { ref mut caption, .. } = hero_img.kind {
        *caption = "Drop any PNG/JPG image here or drag this frame around!".to_string();
    }
    page1.frames.push(hero_img);

    let cover_footer_story = Story::new(
        "Cover Footer",
        "Press F1 (or click '? Help Manual' in the top bar) anytime for step-by-step recipes.\nSwitch to Page 2 in the Left Page Rail to test Live Text Wrap!",
        TextStyle {
            font_size_pt: 11.5,
            color: ColorRgba::rgb(45, 52, 64),
            ..TextStyle::default()
        },
    );
    let cover_footer_id = doc.insert_story(cover_footer_story);
    page1.frames.push(Frame::new_text(
        Rect::new(54.0, 480.0, 324.0, 110.0),
        cover_footer_id,
        0,
    ));
    doc.pages.push(page1);

    // --- PAGE 2: Live Text Wrap Playground ---
    let mut page2 = Page::new("2. Live Text Wrap", Some(master_id));
    let wrap_story = Story::new(
        "Live Text Wrap Story",
        "TRY THIS NOW: Select the gold callout shape or image frame sitting in the middle of this page and drag it around! Because its Text Wrap mode is set to BoundingBox in the Right Inspector, every line in this story automatically recalculates its available horizontal width and wraps cleanly around the obstacle in real time.\n\nThis combines the best of Adobe InDesign's live runaround composer with Scribus's deterministic frame geometry. Whether you are designing an illustrated children's book, a cookbook, a tabletop RPG manual, or a technical guide, you can freely experiment with visual composition without breaking your paragraphs.",
        TextStyle {
            font_size_pt: 12.0,
            line_height_factor: 1.38,
            ..TextStyle::default()
        },
    );
    let wrap_story_id = doc.insert_story(wrap_story);
    let mut body_frame = Frame::new_text(Rect::new(36.0, 42.0, 360.0, 540.0), wrap_story_id, 0);
    body_frame.label = "Main Wrapped Story".to_string();
    page2.frames.push(body_frame);

    let mut obstacle_shape = Frame::new_shape(
        Rect::new(136.0, 175.0, 160.0, 130.0),
        ShapeKind::RoundedRectangle,
        ColorRgba::WARM_GOLD,
    );
    obstacle_shape.label = "Draggable Wrap Obstacle".to_string();
    obstacle_shape.z_index = 25;
    obstacle_shape.wrap = WrapSettings {
        mode: WrapMode::BoundingBox,
        padding_pt: 12.0,
    };
    page2.frames.push(obstacle_shape);
    doc.pages.push(page2);

    // --- PAGE 3 & PAGE 4: Threaded Story Across Pages ---
    let threaded_story = Story::new(
        "Threaded Chapter Across Pages",
        "MULTI-FRAME TEXT THREADING: In a layout-heavy book, stories often span several custom-sized boxes across multiple pages. This story starts inside the top frame on Page 3, flows automatically into the bottom-right frame on Page 3, and continues seamlessly onto Page 4!\n\nTry resizing the first frame on Page 3 using its corner handles—watch how lines automatically pull back or push forward into the linked frames down the chain. If a story ever runs out of room in its final frame, a red [+] overflow badge appears and Preflight alerts you before you export to Print PDF or EPUB 3.\n\nBecause your .book project stores stories, frames, fonts, and images with strictly relative paths, you can save this project on Arch Linux and open it directly on your Mac M1 Air with zero missing assets.",
        TextStyle {
            font_size_pt: 12.0,
            line_height_factor: 1.4,
            ..TextStyle::default()
        },
    );
    let thread_story_id = doc.insert_story(threaded_story);

    let mut page3 = Page::new("3. Threaded Flow (A)", Some(master_id));
    let mut tf_a = Frame::new_text(Rect::new(36.0, 42.0, 360.0, 210.0), thread_story_id, 0);
    tf_a.label = "Thread Frame #1".to_string();
    let mut tf_b = Frame::new_text(Rect::new(96.0, 285.0, 300.0, 210.0), thread_story_id, 1);
    tf_b.label = "Thread Frame #2".to_string();
    page3.frames.push(tf_a);
    page3.frames.push(tf_b);
    doc.pages.push(page3);

    let mut page4 = Page::new("4. Threaded Flow (B) & Export", Some(master_id));
    let mut tf_c = Frame::new_text(Rect::new(36.0, 42.0, 360.0, 260.0), thread_story_id, 2);
    tf_c.label = "Thread Frame #3 (Page 4)".to_string();
    page4.frames.push(tf_c);

    let export_callout = Frame::new_shape(
        Rect::new(36.0, 330.0, 360.0, 180.0),
        ShapeKind::RoundedRectangle,
        ColorRgba::rgb(230, 240, 252),
    );
    page4.frames.push(export_callout);
    doc.pages.push(page4);

    doc
}

/// Runs plain-English preflight checks on the document and computed layout.
pub fn run_preflight(doc: &BookDocument, layout: &ComputedBookLayout) -> Vec<PreflightIssue> {
    let mut issues = Vec::new();

    for (page_idx, page_layout) in layout.pages.iter().enumerate() {
        for (frame_id, tf) in &page_layout.text_frames {
            if tf.is_overflowing {
                issues.push(PreflightIssue {
                    severity: PreflightSeverity::Warning,
                    code: "TEXT_OVERFLOW".to_string(),
                    title: format!(
                        "Overflowing Text on Page {} ({} chars hidden)",
                        page_idx + 1,
                        tf.overflow_char_count
                    ),
                    explanation: "This text frame is the end of its thread chain and contains more text than fits inside its box.".to_string(),
                    how_to_fix: "Enlarge the frame, decrease font size, or use the Link Text (L) tool to thread it into another text frame.".to_string(),
                    page_index: Some(page_idx),
                    frame_id: Some(*frame_id),
                });
            }
        }
    }

    for (page_idx, page) in doc.pages.iter().enumerate() {
        for frame in &page.frames {
            if let FrameKind::Image {
                asset_id: Some(ref id),
                ..
            } = frame.kind
            {
                if !doc.assets.contains_key(id) {
                    issues.push(PreflightIssue {
                        severity: PreflightSeverity::Error,
                        code: "MISSING_ASSET".to_string(),
                        title: format!("Missing Image Asset '{}' on Page {}", id, page_idx + 1),
                        explanation: "The image frame references an asset ID that is not present in the .book bundle.".to_string(),
                        how_to_fix: "Select the image frame and import a replacement PNG/JPG file.".to_string(),
                        page_index: Some(page_idx),
                        frame_id: Some(frame.id),
                    });
                }
            }
        }
    }

    issues
}
