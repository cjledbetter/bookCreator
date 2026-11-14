//! Out-of-the-box searchable Help Manual, OS-aware shortcut guide (Mac M1 Air
//! vs Arch Linux), plain-English Preflight checker, and 4-page Interactive
//! Starter Tutorial Book generator.

use book_core::{
    BookDocument, ColorRgba, Frame, FrameKind, MasterPage, Page, PagePreset, PageSetup,
    PreflightIssue, PreflightSeverity, Rect, ShapeKind, Story, TextAlignment, TextStyle, WrapMode,
    WrapSettings,
};
use book_layout::ComputedBookLayout;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpArticle {
    pub id: &'static str,
    pub category: &'static str,
    pub title: &'static str,
    pub summary: &'static str,
    pub steps: &'static [&'static str],
    pub keywords: &'static [&'static str],
}

/// Cross-platform keyboard shortcut descriptor showing both Mac M1 Air (`⌘ Cmd`)
/// and Arch Linux (`Ctrl`) bindings side by side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShortcutEntry {
    pub action: &'static str,
    pub mac_shortcut: &'static str,
    pub linux_shortcut: &'static str,
    pub category: &'static str,
}

impl ShortcutEntry {
    /// Returns the shortcut string for the current target operating system.
    pub const fn active_shortcut(&self) -> &'static str {
        if cfg!(target_os = "macos") {
            self.mac_shortcut
        } else {
            self.linux_shortcut
        }
    }
}

/// Returns `"Cmd (⌘)"` on macOS (Mac M1 Air) and `"Ctrl"` on Arch Linux / Windows.
pub const fn primary_modifier_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "Cmd (⌘)"
    } else {
        "Ctrl"
    }
}

/// Returns the complete table of keyboard shortcuts for both Mac M1 Air and Arch Linux.
pub fn keyboard_shortcuts() -> Vec<ShortcutEntry> {
    vec![
        ShortcutEntry {
            action: "Select / Move Tool",
            mac_shortcut: "V",
            linux_shortcut: "V",
            category: "Tools",
        },
        ShortcutEntry {
            action: "Text Frame Tool",
            mac_shortcut: "T",
            linux_shortcut: "T",
            category: "Tools",
        },
        ShortcutEntry {
            action: "Image Frame Tool",
            mac_shortcut: "I",
            linux_shortcut: "I",
            category: "Tools",
        },
        ShortcutEntry {
            action: "Shape Frame Tool",
            mac_shortcut: "S",
            linux_shortcut: "S",
            category: "Tools",
        },
        ShortcutEntry {
            action: "Link / Thread Text Tool",
            mac_shortcut: "L",
            linux_shortcut: "L",
            category: "Tools",
        },
        ShortcutEntry {
            action: "Undo Last Action",
            mac_shortcut: "⌘ Cmd + Z",
            linux_shortcut: "Ctrl + Z",
            category: "Editing",
        },
        ShortcutEntry {
            action: "Redo Action",
            mac_shortcut: "⌘ Cmd + Shift + Z",
            linux_shortcut: "Ctrl + Y",
            category: "Editing",
        },
        ShortcutEntry {
            action: "Duplicate Selected Frame",
            mac_shortcut: "⌘ Cmd + D",
            linux_shortcut: "Ctrl + D",
            category: "Editing",
        },
        ShortcutEntry {
            action: "Delete Selected Frame",
            mac_shortcut: "Delete / Backspace",
            linux_shortcut: "Delete / Backspace",
            category: "Editing",
        },
        ShortcutEntry {
            action: "Save Portable .book Archive",
            mac_shortcut: "⌘ Cmd + S",
            linux_shortcut: "Ctrl + S",
            category: "File & Export",
        },
        ShortcutEntry {
            action: "Open .book Archive",
            mac_shortcut: "⌘ Cmd + O",
            linux_shortcut: "Ctrl + O",
            category: "File & Export",
        },
        ShortcutEntry {
            action: "Export Print-Ready PDF",
            mac_shortcut: "⌘ Cmd + E",
            linux_shortcut: "Ctrl + E",
            category: "File & Export",
        },
        ShortcutEntry {
            action: "Export EPUB 3 (Fixed / Reflowable)",
            mac_shortcut: "⌘ Cmd + Shift + E",
            linux_shortcut: "Ctrl + Shift + E",
            category: "File & Export",
        },
        ShortcutEntry {
            action: "Pan Spread Canvas",
            mac_shortcut: "Space + Drag / Two-Finger Scroll",
            linux_shortcut: "Space + Drag / Middle-Drag",
            category: "Canvas & View",
        },
        ShortcutEntry {
            action: "Zoom Canvas In / Out",
            mac_shortcut: "⌘ Cmd + Scroll",
            linux_shortcut: "Ctrl + Scroll",
            category: "Canvas & View",
        },
        ShortcutEntry {
            action: "Fit Active Page in Window",
            mac_shortcut: "⌘ Cmd + 0",
            linux_shortcut: "Ctrl + 0",
            category: "Canvas & View",
        },
        ShortcutEntry {
            action: "Toggle Searchable Help Manual",
            mac_shortcut: "F1",
            linux_shortcut: "F1",
            category: "Help",
        },
    ]
}

/// Returns `(action, active_os_shortcut, category)` tuples tailored to the running OS.
pub fn active_os_shortcuts() -> Vec<(&'static str, &'static str, &'static str)> {
    keyboard_shortcuts()
        .into_iter()
        .map(|entry| (entry.action, entry.active_shortcut(), entry.category))
        .collect()
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
                "With Image Frame (I) or Shape (S), drag a box onto the page—or drop any PNG/JPG/SVG file straight from Finder (macOS) or your Linux file manager.",
                "Use the Right Inspector to toggle 'Text Wrap (Runaround)' so your story flows smoothly around dragged images and callouts.",
                "Click 'Export PDF' for print-ready output (with optional bleed & crop marks) or 'Export EPUB' for Fixed-Layout or Reflowable eBooks.",
            ],
            keywords: &[
                "start", "tutorial", "first", "beginner", "tools", "drag", "drop", "60 seconds",
            ],
        },
        HelpArticle {
            id: "canvas-navigation",
            category: "Canvas & Tools",
            title: "2. Canvas Navigation, Pan/Zoom & Smart Snapping Guides",
            summary: "Fluidly navigate spreads and align frames with precision using live magenta and cyan smart guides.",
            steps: &[
                "Hold Spacebar and drag (or use middle-mouse drag / two-finger trackpad scroll) to pan smoothly across the spread canvas.",
                "Hold Ctrl (Arch Linux) or ⌘ Cmd (Mac M1 Air) while scrolling the mouse wheel to zoom from 25% to 400%, or click 'Fit Page' in the top bar.",
                "Toggle 'Show Margins/Bleed' in the top toolbar to inspect the red dashed print bleed boundary and safe inside/outside margin guides.",
                "As you drag any frame with Select/Move (V), Smart Snapping Guides automatically snap to page centers, safe margins, and neighboring frame edges.",
            ],
            keywords: &[
                "canvas", "pan", "zoom", "snap", "guides", "alignment", "margins", "navigation",
            ],
        },
        HelpArticle {
            id: "working-with-frames",
            category: "Canvas & Tools",
            title: "3. Working with Frames (Move, Resize, Rotate, Z-Order, Lock)",
            summary: "Master non-destructive frame geometry in typographic points (1 pt = 1/72 inch) with full Undo/Redo.",
            steps: &[
                "Click any frame with the Select/Move (V) tool to reveal its 8 corner and edge resize handles.",
                "Drag any handle to resize the frame live, or type exact X, Y, Width, and Height values (in pt) in the Right Inspector.",
                "Use 'Bring to Front' / 'Send to Back' (or edit Z-Index) in the Right Inspector to layer text over decorative background shapes.",
                "Enable the 'Locked' checkbox on background banners or master elements to prevent accidental dragging while laying out foreground content.",
                "Every move, resize, and style change is recorded in CommandHistory—press Ctrl+Z / ⌘+Z anytime to undo.",
            ],
            keywords: &[
                "frame", "move", "resize", "rotate", "z-index", "layer", "lock", "undo", "redo", "points",
            ],
        },
        HelpArticle {
            id: "typography-text-editing",
            category: "Layout & Typography",
            title: "4. Direct On-Canvas Text Editing & Typography Styles",
            summary: "Edit stories directly on the page and customize fonts, point sizes, line height, and paragraph alignment.",
            steps: &[
                "Double-click any Text Frame on the canvas with Select/Move (V) to enter Direct On-Canvas Text Editing mode.",
                "Type or paste multi-paragraph prose directly; the layout engine shapes and wraps lines in real time.",
                "In the Right Inspector, pick a Font Family (Literata, Inter, JetBrains Mono, Helvetica, Times), Font Size (pt), and Line Height multiplier.",
                "Toggle Bold, Italic, Text Color, optional Frame Background fill, and Alignment (Left, Center, Right, or Justified).",
            ],
            keywords: &[
                "text", "edit", "font", "typography", "style", "size", "bold", "italic", "justify", "align",
            ],
        },
        HelpArticle {
            id: "threading-text",
            category: "Layout & Typography",
            title: "5. Threading Text Frames Across Pages (Overflow [+] & Story Chains)",
            summary: "Link multiple text frames together so long chapters automatically flow from column to column and page to page.",
            steps: &[
                "When a Text Frame has more text than fits inside its box, a red '[+]' overflow indicator appears at its bottom-right corner.",
                "Select the 'Link / Thread Text (L)' tool in the top toolbar (or enable 'Show Text Thread Lines' to visualize existing chains).",
                "Click the overflowing source Text Frame first, then click any destination Text Frame on the same page or a later page.",
                "Your story now threads continuously across both frames! Resizing an earlier frame automatically pushes or pulls lines down the chain.",
            ],
            keywords: &[
                "thread", "link", "overflow", "story", "chain", "pages", "flow", "columns", "plus",
            ],
        },
        HelpArticle {
            id: "text-wrap",
            category: "Layout & Typography",
            title: "6. Live Text Wrap (\"Runaround\"): Moving Images & Shapes Through Text",
            summary: "Make text automatically split and wrap around illustrations, pull-quotes, and badges at 60fps.",
            steps: &[
                "Select any Image Frame or Shape Frame that overlaps a Text Frame using the Select/Move (V) tool.",
                "In the Right Inspector under 'Text Wrap (Runaround)', choose 'BoundingBox' (text wraps on left/right sides) or 'JumpObject' (text skips the vertical band).",
                "Adjust the 'Wrap Padding (pt)' slider to control the exact typographic clearance around the obstacle.",
                "Drag the obstacle across the page—every intersecting text line dynamically splits and reflows in real time.",
            ],
            keywords: &[
                "wrap", "runaround", "image", "reflow", "padding", "obstacle", "boundingbox", "jump",
            ],
        },
        HelpArticle {
            id: "print-prepress",
            category: "Export & Prepress",
            title: "7. Print Prepress (Trim, Bleed, Crop Marks, Master Pages & Preflight)",
            summary: "Configure industry-standard trim presets, 0.125-inch print bleed, master page numbering, and live preflight checks.",
            steps: &[
                "With no frame selected, use the Right Inspector (or Left Master Rail) to choose a standard trim size: US Trade (6x9 in), US Letter, Square (8.5x8.5 in), Digest, ISO A4, or A5.",
                "Set Print Bleed (default 9.0 pt = 0.125 in) and extend full-bleed background artwork all the way to the red dashed bleed guide.",
                "Configure Master Page headers and automatic page numbering in the Left Rail so every content page inherits consistent running heads/folios.",
                "Click the 'Preflight' pill in the top toolbar to scan for TEXT_OVERFLOW, MISSING_ASSET, LOW_PRINT_DPI (< 250 DPI), OUT_OF_BOUNDS, and EMPTY_TEXT_FRAME issues.",
            ],
            keywords: &[
                "prepress", "preflight", "bleed", "trim", "crop", "marks", "master", "dpi", "resolution",
            ],
        },
        HelpArticle {
            id: "export-pdf-epub",
            category: "Export & Portability",
            title: "8. Equal-Priority Export (Print PDF vs. EPUB 3) & Mac M1 Air ↔ Arch Linux Portability",
            summary: "Deliver press-ready PDFs, Fixed-Layout or Reflowable EPUB 3.3 eBooks, and 100% portable .book bundles across macOS and Linux.",
            steps: &[
                "For commercial printing: click 'Export Print PDF' and enable 'Include Bleed' and 'Corner Crop Marks' to generate a vector PDF with exact TrimBox and MediaBox.",
                "For visual/illustrated eBooks: click 'Export EPUB 3' and select 'Fixed-Layout (pre-paginated)' so Apple Books and Foliate/Calibre preserve exact point coordinates.",
                "For prose-heavy eReaders: select 'Reflowable EPUB 3' to emit clean, accessible XHTML chapters ordered by story threads.",
                "Use 'Save .book' (single-file ZIP) or 'Save .bookdir' (Git-friendly folder) to move projects seamlessly between Mac M1 Air and Arch Linux—all asset paths are strictly relative (assets/images/*, assets/fonts/*).",
            ],
            keywords: &[
                "pdf", "epub", "print", "fixed", "reflowable", "apple books", "mac", "m1", "linux", "arch", "portable", "book", "git",
            ],
        },
        HelpArticle {
            id: "front-back-covers",
            category: "Covers & Prepress",
            title: "9. Creating Front, Back & Wrap-Around Print Covers",
            summary: "Generate a complete Front Cover, Back Cover (with Blurb, Author Bio, and 2.0x1.2 in ISBN Barcode box), or Wrap-Around Jacket with Spine Calculator in 1 click.",
            steps: &[
                "Click the 'Covers ▾' menu in the top toolbar (or use the quick Cover buttons in the Left Page Rail) and select '✨ Generate Front & Back Cover (1-Click)'.",
                "Page 1 is automatically configured as your Front Cover (with page numbers/headers hidden via Master = None and a full-bleed background extending to -bleed).",
                "A dedicated Back Cover page is appended at the end of your book with a Back Cover Hook & Blurb frame, Author Photo & About the Author bio band, and a crisp white 2.0 × 1.2 in (144 × 86.4 pt) ISBN Barcode Box in the bottom-right safe area.",
                "For print-on-demand paperback jackets, open 'Covers ▾', choose your Paper Stock (White 0.002252 in/pg, Cream 0.0025 in/pg, or Color Illustrated 0.002347 in/pg) to see the live Spine Width in pt/in/mm, and click '📏 Add Wrap-Around Cover (Back + Spine + Front)'.",
                "Toggle 'Cover Page (Hide Page Numbers & Headers)' on any page in the Left Rail or Right Inspector to suppress running headers and folios on half-titles or special spreads.",
            ],
            keywords: &[
                "cover", "front cover", "back cover", "spine", "wrap-around", "barcode", "isbn", "blurb", "author bio", "paper stock", "bleed",
            ],
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
            a.id.to_lowercase().contains(&q)
                || a.title.to_lowercase().contains(&q)
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
    doc.metadata.publisher = "Open Desktop Publishing Press".to_string();
    doc.metadata.isbn = "978-0-00-000001-2".to_string();

    let mut master = MasterPage::default();
    master.show_page_numbers = true;
    master.header_text = "Book Creator — Interactive Visual Guide".to_string();
    let master_id = master.id;
    doc.master_pages = vec![master];
    doc.pages.clear();

    // --- PAGE 1: Cover Spread ---
    let mut page1 = Page::new("1. Cover", Some(master_id));
    page1.background_color = ColorRgba::rgb(248, 245, 238);

    let mut banner = Frame::new_shape(
        Rect::new(36.0, 36.0, 360.0, 176.0),
        ShapeKind::RoundedRectangle,
        ColorRgba::rgb(31, 58, 96),
    );
    banner.label = "Cover Header Banner".to_string();
    banner.z_index = 5;
    page1.frames.push(banner);

    let mut gold_rule = Frame::new_shape(
        Rect::new(54.0, 192.0, 324.0, 4.0),
        ShapeKind::DividerLine,
        ColorRgba::WARM_GOLD,
    );
    gold_rule.label = "Cover Gold Divider".to_string();
    gold_rule.z_index = 10;
    page1.frames.push(gold_rule);

    let cover_title_story = Story::new(
        "Cover Title",
        "BOOK CREATOR\nVisual Desktop Publishing in Rust\n\nDouble-click any text frame to edit on-canvas, or drag frames to feel live smart snapping!",
        TextStyle {
            font_family: "Literata".to_string(),
            font_size_pt: 15.0,
            line_height_factor: 1.35,
            bold: true,
            italic: false,
            color: ColorRgba::WHITE,
            alignment: TextAlignment::Left,
        },
    );
    let cover_title_id = doc.insert_story(cover_title_story);
    let mut title_frame = Frame::new_text(Rect::new(54.0, 48.0, 324.0, 138.0), cover_title_id, 0);
    title_frame.label = "Cover Title Frame".to_string();
    title_frame.z_index = 15;
    page1.frames.push(title_frame);

    let mut hero_img = Frame::new_image(Rect::new(54.0, 232.0, 324.0, 200.0), None);
    hero_img.label = "Cover Illustration".to_string();
    hero_img.wrap = WrapSettings {
        mode: WrapMode::None,
        padding_pt: 8.0,
    };
    if let FrameKind::Image { ref mut caption, .. } = hero_img.kind {
        *caption = "Drop any PNG/JPG image here or drag this frame around!".to_string();
    }
    page1.frames.push(hero_img);

    let cover_footer_story = Story::new(
        "Cover Footer",
        "WELCOME TO YOUR 4-PAGE STARTER BOOK:\n- Press F1 (or click '? Help Manual') anytime for searchable recipes & OS shortcuts.\n- Switch to Page 2 in the Left Page Rail to test 60fps Live Text Wrap.\n- Visit Pages 3 & 4 to explore Multi-Page Story Threading and Print/EPUB Export.",
        TextStyle {
            font_size_pt: 11.0,
            line_height_factor: 1.35,
            color: ColorRgba::rgb(45, 52, 64),
            ..TextStyle::default()
        },
    );
    let cover_footer_id = doc.insert_story(cover_footer_story);
    let mut cover_footer_frame =
        Frame::new_text(Rect::new(54.0, 452.0, 324.0, 148.0), cover_footer_id, 0);
    cover_footer_frame.label = "Cover Quick-Start Guide".to_string();
    page1.frames.push(cover_footer_frame);
    doc.pages.push(page1);

    // --- PAGE 2: Live Text Wrap Playground ---
    let mut page2 = Page::new("2. Live Text Wrap", Some(master_id));

    let mut p2_header_bar = Frame::new_shape(
        Rect::new(36.0, 36.0, 360.0, 4.0),
        ShapeKind::DividerLine,
        ColorRgba::ACCENT_BLUE,
    );
    p2_header_bar.label = "Page 2 Accent Rule".to_string();
    page2.frames.push(p2_header_bar);

    let wrap_story = Story::new(
        "Live Text Wrap Story",
        "TRY THIS NOW: Select the gold callout shape sitting in the middle of this page and drag it anywhere across the prose! Because its Text Wrap mode is set to BoundingBox in the Right Inspector, every line in this story automatically recalculates its available horizontal segments and wraps cleanly around the obstacle in real time.\n\nThis combines Adobe InDesign's fluid runaround composer with Scribus's transparent frame geometry. You can switch the obstacle between BoundingBox (splitting lines on left and right sides) and JumpObject (breaking paragraphs cleanly above and below the frame) or adjust the Wrap Padding slider in points.\n\nWhether you are laying out an illustrated children's book, a cookbook, a tabletop RPG manual, or a technical guide, you can experiment freely with visual composition without breaking your paragraphs.",
        TextStyle {
            font_size_pt: 11.5,
            line_height_factor: 1.35,
            ..TextStyle::default()
        },
    );
    let wrap_story_id = doc.insert_story(wrap_story);
    let mut body_frame = Frame::new_text(Rect::new(36.0, 52.0, 360.0, 548.0), wrap_story_id, 0);
    body_frame.label = "Main Wrapped Story".to_string();
    page2.frames.push(body_frame);

    let mut obstacle_shape = Frame::new_shape(
        Rect::new(136.0, 180.0, 160.0, 124.0),
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
        "MULTI-FRAME TEXT THREADING: In a layout-heavy book, chapters often span several custom-sized frames across multiple pages. This single Story begins inside Thread Frame #1 at the top of Page 3, flows automatically into Thread Frame #2 on the lower half of Page 3, and continues seamlessly into Thread Frame #3 at the top of Page 4!\n\nHOW TO TEST LIVE REFLOW: Select Thread Frame #1 on Page 3 using the Select/Move (V) tool and drag its bottom resize handle upward to make the box shorter. Watch how displaced lines immediately cascade into Thread Frame #2 and push lines forward onto Page 4 in real time.\n\nLINKING NEW FRAMES: Whenever a story runs out of space in its last linked frame, a red [+] overflow port appears at the bottom-right corner and Live Preflight raises a TEXT_OVERFLOW warning. Choose the Link / Thread Text (L) tool in the top toolbar, click the overflowing frame, and click any empty text frame to extend the chain.\n\nCROSS-PLATFORM PORTABILITY: Every story, style, thread link, and embedded asset inside a .book archive uses strictly relative paths so your layout renders identically on Arch Linux and Mac M1 Air.",
        TextStyle {
            font_size_pt: 12.0,
            line_height_factor: 1.4,
            ..TextStyle::default()
        },
    );
    let thread_story_id = doc.insert_story(threaded_story);

    let mut page3 = Page::new("3. Threaded Flow (A)", Some(master_id));
    let mut p3_divider = Frame::new_shape(
        Rect::new(36.0, 240.0, 360.0, 3.0),
        ShapeKind::DividerLine,
        ColorRgba::ACCENT_BLUE,
    );
    p3_divider.label = "Page 3 Thread Divider".to_string();
    page3.frames.push(p3_divider);

    let mut tf_a = Frame::new_text(Rect::new(36.0, 44.0, 360.0, 180.0), thread_story_id, 0);
    tf_a.label = "Thread Frame #1 (Page 3 Top)".to_string();
    let mut tf_b = Frame::new_text(Rect::new(66.0, 256.0, 330.0, 180.0), thread_story_id, 1);
    tf_b.label = "Thread Frame #2 (Page 3 Bottom)".to_string();
    page3.frames.push(tf_a);
    page3.frames.push(tf_b);
    doc.pages.push(page3);

    let mut page4 = Page::new("4. Threaded Flow (B) & Export", Some(master_id));
    let mut tf_c = Frame::new_text(Rect::new(36.0, 44.0, 360.0, 280.0), thread_story_id, 2);
    tf_c.label = "Thread Frame #3 (Page 4)".to_string();
    page4.frames.push(tf_c);

    let mut export_callout = Frame::new_shape(
        Rect::new(36.0, 336.0, 360.0, 250.0),
        ShapeKind::RoundedRectangle,
        ColorRgba::rgb(230, 240, 252),
    );
    export_callout.label = "Prepress & Export Callout Box".to_string();
    export_callout.z_index = 5;
    page4.frames.push(export_callout);

    let checklist_story = Story::new(
        "Prepress & Export Checklist",
        "PREPRESS & EQUAL-PRIORITY EXPORT CHECKLIST:\n1. Live Preflight: Check the top toolbar pill for 0 Issues before exporting.\n2. Print-Ready PDF: Enable 9 pt (0.125 in) Bleed and Corner Crop Marks for commercial presses.\n3. EPUB 3.3 Fixed-Layout: Preserves exact point coordinates 1:1 for Apple Books & Foliate.\n4. EPUB 3.3 Reflowable: Exports clean, accessible chapter XHTML in story thread order.",
        TextStyle {
            font_size_pt: 11.0,
            line_height_factor: 1.35,
            color: ColorRgba::rgb(25, 42, 68),
            ..TextStyle::default()
        },
    );
    let checklist_story_id = doc.insert_story(checklist_story);
    let mut checklist_frame =
        Frame::new_text(Rect::new(52.0, 352.0, 328.0, 218.0), checklist_story_id, 0);
    checklist_frame.label = "Export Checklist Frame".to_string();
    checklist_frame.z_index = 15;
    page4.frames.push(checklist_frame);
    doc.pages.push(page4);

    doc
}

/// Minimum effective image resolution in dots per inch (DPI) before Preflight warns.
pub const MIN_RECOMMENDED_PRINT_DPI: f32 = 250.0;

/// Runs plain-English preflight checks on the document and computed layout:
/// - `TEXT_OVERFLOW`: Overflowing text frames (`[+]`).
/// - `MISSING_ASSET`: Image frame referencing a missing asset ID.
/// - `LOW_PRINT_DPI`: Image asset with `pixel_dimensions` whose effective print DPI is below 250 DPI.
/// - `OUT_OF_BOUNDS`: Frame extending outside page trim + bleed bounds.
/// - `EMPTY_TEXT_FRAME`: Unlinked text frame with empty story text.
pub fn run_preflight(doc: &BookDocument, layout: &ComputedBookLayout) -> Vec<PreflightIssue> {
    let mut issues = Vec::new();

    // 1. Check TEXT_OVERFLOW across all computed page layouts
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

    let bleed = doc.setup.bleed_pt.max(0.0);
    let min_bound_x = -bleed;
    let min_bound_y = -bleed;
    let max_bound_x = doc.setup.width_pt + bleed;
    let max_bound_y = doc.setup.height_pt + bleed;
    let eps = 1.0;

    for (page_idx, page) in doc.pages.iter().enumerate() {
        for frame in &page.frames {
            // 2. Check OUT_OF_BOUNDS against page trim + bleed area
            if frame.rect.x < min_bound_x - eps
                || frame.rect.y < min_bound_y - eps
                || frame.rect.right() > max_bound_x + eps
                || frame.rect.bottom() > max_bound_y + eps
            {
                issues.push(PreflightIssue {
                    severity: PreflightSeverity::Warning,
                    code: "OUT_OF_BOUNDS".to_string(),
                    title: format!(
                        "Frame '{}' Extends Outside Bleed Bounds on Page {}",
                        frame.label,
                        page_idx + 1
                    ),
                    explanation: "Part of this frame extends beyond the page trim + bleed boundary and will be clipped in print and fixed-layout export.".to_string(),
                    how_to_fix: "Move or resize the frame so its edges stay inside the page trim or bleed guides.".to_string(),
                    page_index: Some(page_idx),
                    frame_id: Some(frame.id),
                });
            }

            match &frame.kind {
                // 3. Check MISSING_ASSET and 4. Check LOW_PRINT_DPI
                FrameKind::Image {
                    asset_id: Some(id), ..
                } => match doc.assets.get(id) {
                    None => {
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
                    Some(asset) => {
                        if let Some((px_w, px_h)) = asset.pixel_dimensions {
                            let width_in = (frame.rect.width / 72.0).max(0.01);
                            let height_in = (frame.rect.height / 72.0).max(0.01);
                            let dpi_x = px_w as f32 / width_in;
                            let dpi_y = px_h as f32 / height_in;
                            let effective_dpi = dpi_x.min(dpi_y);
                            if effective_dpi < MIN_RECOMMENDED_PRINT_DPI {
                                issues.push(PreflightIssue {
                                    severity: PreflightSeverity::Warning,
                                    code: "LOW_PRINT_DPI".to_string(),
                                    title: format!(
                                        "Low Print Resolution ({:.0} DPI) on Page {}",
                                        effective_dpi,
                                        page_idx + 1
                                    ),
                                    explanation: format!(
                                        "Image '{}' ({}x{} px) is placed at {:.0}x{:.0} pt, yielding {:.0} effective DPI (at least {:.0} DPI is recommended for crisp print output).",
                                        asset.original_filename,
                                        px_w,
                                        px_h,
                                        frame.rect.width,
                                        frame.rect.height,
                                        effective_dpi,
                                        MIN_RECOMMENDED_PRINT_DPI
                                    ),
                                    how_to_fix: "Reduce the image frame dimensions on the page or import a higher-resolution image asset.".to_string(),
                                    page_index: Some(page_idx),
                                    frame_id: Some(frame.id),
                                });
                            }
                        }
                    }
                },
                // 5. Check EMPTY_TEXT_FRAME for unlinked text frames with empty story text
                FrameKind::Text { story_id, .. } => {
                    let chain_len = doc.threaded_frames_for_story(*story_id).len();
                    let is_empty_story = doc
                        .stories
                        .get(story_id)
                        .map(|s| s.text.trim().is_empty())
                        .unwrap_or(true);
                    if chain_len <= 1 && is_empty_story {
                        issues.push(PreflightIssue {
                            severity: PreflightSeverity::Warning,
                            code: "EMPTY_TEXT_FRAME".to_string(),
                            title: format!(
                                "Empty Text Frame '{}' on Page {}",
                                frame.label,
                                page_idx + 1
                            ),
                            explanation: "This unlinked text frame has no story text and will render as an empty box.".to_string(),
                            how_to_fix: "Double-click the frame to enter text, link it to an existing story with the Link Text (L) tool, or delete the unused frame.".to_string(),
                            page_index: Some(page_idx),
                            frame_id: Some(frame.id),
                        });
                    }
                }
                _ => {}
            }
        }
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use book_core::AssetKind;

    #[test]
    fn builtin_help_manual_has_all_eight_comprehensive_articles_and_search_works() {
        let articles = builtin_help_articles();
        assert!(
            articles.len() >= 8,
            "Expected at least 8 help articles, found {}",
            articles.len()
        );
        for article in &articles {
            assert!(!article.id.is_empty());
            assert!(!article.category.is_empty());
            assert!(!article.title.is_empty());
            assert!(!article.summary.is_empty());
            assert!(
                article.steps.len() >= 4,
                "Article '{}' should have at least 4 actionable steps",
                article.id
            );
            assert!(!article.keywords.is_empty());
        }

        // Empty query returns all articles
        assert_eq!(search_help_articles("   ").len(), articles.len());

        // Search by keyword/topic
        let wrap_results = search_help_articles("runaround");
        assert!(!wrap_results.is_empty());
        assert!(wrap_results.iter().any(|a| a.id == "text-wrap"));

        let thread_results = search_help_articles("overflow");
        assert!(!thread_results.is_empty());
        assert!(thread_results.iter().any(|a| a.id == "threading-text"));

        let portable_results = search_help_articles("mac m1");
        assert!(!portable_results.is_empty());
        assert!(portable_results.iter().any(|a| a.id == "export-pdf-epub"));

        let snap_results = search_help_articles("snapping");
        assert!(!snap_results.is_empty());
        assert!(snap_results.iter().any(|a| a.id == "canvas-navigation"));
    }

    #[test]
    fn os_keyboard_shortcuts_cover_both_mac_m1_and_arch_linux() {
        let label = primary_modifier_label();
        if cfg!(target_os = "macos") {
            assert_eq!(label, "Cmd (⌘)");
        } else {
            assert_eq!(label, "Ctrl");
        }

        let shortcuts = keyboard_shortcuts();
        assert!(shortcuts.len() >= 12);
        assert!(shortcuts.iter().any(|s| s.mac_shortcut.contains("⌘ Cmd")));
        assert!(shortcuts.iter().any(|s| s.linux_shortcut.contains("Ctrl")));
        assert!(shortcuts.iter().any(|s| s.action.contains("Help Manual")));

        let active = active_os_shortcuts();
        assert_eq!(active.len(), shortcuts.len());
        for (action, binding, category) in active {
            assert!(!action.is_empty());
            assert!(!binding.is_empty());
            assert!(!category.is_empty());
        }
    }

    #[test]
    fn tutorial_book_has_four_rich_pages_threaded_story_and_zero_preflight_issues() {
        let doc = create_tutorial_book();
        assert_eq!(doc.pages.len(), 4);
        assert_eq!(doc.master_pages.len(), 1);
        assert!(doc.master_pages[0].show_page_numbers);
        assert!(!doc.master_pages[0].header_text.is_empty());

        // Page 2 must contain a draggable obstacle with BoundingBox text wrap
        let page2_has_wrap_obstacle = doc.pages[1]
            .frames
            .iter()
            .any(|f| f.wrap.mode == WrapMode::BoundingBox);
        assert!(
            page2_has_wrap_obstacle,
            "Page 2 must include an interactive BoundingBox wrap obstacle"
        );

        // Pages 3 and 4 must share a 3-frame threaded story chain
        let threaded_chains: Vec<_> = doc
            .stories
            .keys()
            .map(|&id| doc.threaded_frames_for_story(id))
            .filter(|chain| chain.len() == 3)
            .collect();
        assert_eq!(
            threaded_chains.len(),
            1,
            "Expected a 3-frame threaded story across Pages 3 and 4"
        );
        let chain = &threaded_chains[0];
        assert_eq!(chain[0].0, 2); // Page 3 (index 2)
        assert_eq!(chain[1].0, 2); // Page 3 (index 2)
        assert_eq!(chain[2].0, 3); // Page 4 (index 3)

        // Compute layout and verify lines flow into all 3 linked frames without overflowing
        let layout = ComputedBookLayout::compute(&doc);
        let f1_layout = layout.pages[2].text_frames.get(&chain[0].1).unwrap();
        let f2_layout = layout.pages[2].text_frames.get(&chain[1].1).unwrap();
        let f3_layout = layout.pages[3].text_frames.get(&chain[2].1).unwrap();
        assert!(!f1_layout.lines.is_empty());
        assert!(!f2_layout.lines.is_empty());
        assert!(
            !f3_layout.lines.is_empty(),
            "Threaded story should flow onto Page 4"
        );
        assert!(
            !f3_layout.is_overflowing,
            "Tutorial book must not have accidental text overflow"
        );

        // Fresh tutorial book must pass preflight with zero issues
        let issues = run_preflight(&doc, &layout);
        assert!(
            issues.is_empty(),
            "Expected 0 preflight issues on fresh tutorial book, got: {issues:#?}"
        );
    }

    #[test]
    fn preflight_detects_all_five_diagnostic_rules() {
        let mut doc = BookDocument::default();
        doc.pages.clear();
        let mut page = Page::new("Test Page", None);

        // 1. TEXT_OVERFLOW: tiny frame with long story
        let overflow_story = Story::new(
            "Overflowing",
            "This story is far too long to fit inside a tiny 40x20 pt text frame and must trigger TEXT_OVERFLOW during preflight checking.",
            TextStyle::default(),
        );
        let overflow_story_id = doc.insert_story(overflow_story);
        page.frames.push(Frame::new_text(
            Rect::new(36.0, 36.0, 50.0, 20.0),
            overflow_story_id,
            0,
        ));

        // 2. MISSING_ASSET: image frame referencing non-existent asset ID
        page.frames.push(Frame::new_image(
            Rect::new(36.0, 80.0, 100.0, 100.0),
            Some("ghost_asset_id".to_string()),
        ));

        // 3. LOW_PRINT_DPI: 100x100 px image stretched across 288x288 pt (4x4 inches = 25 DPI < 250 DPI)
        let low_dpi_id = doc.add_asset_bytes(
            "low_res_photo",
            AssetKind::Image,
            "tiny_web_thumb.png",
            "image/png",
            vec![1, 2, 3, 4],
            Some((100, 100)),
        );
        page.frames.push(Frame::new_image(
            Rect::new(36.0, 200.0, 288.0, 288.0),
            Some(low_dpi_id),
        ));

        // 4. OUT_OF_BOUNDS: shape frame extending well past right trim + bleed
        page.frames.push(Frame::new_shape(
            Rect::new(300.0, 500.0, 250.0, 80.0),
            ShapeKind::Rectangle,
            ColorRgba::ACCENT_BLUE,
        ));

        // 5. EMPTY_TEXT_FRAME: standalone text frame with empty story text
        let empty_story = Story::new("Empty", "   ", TextStyle::default());
        let empty_story_id = doc.insert_story(empty_story);
        page.frames.push(Frame::new_text(
            Rect::new(36.0, 500.0, 120.0, 60.0),
            empty_story_id,
            0,
        ));

        doc.pages.push(page);
        let layout = ComputedBookLayout::compute(&doc);
        let issues = run_preflight(&doc, &layout);

        let codes: Vec<&str> = issues.iter().map(|i| i.code.as_str()).collect();
        assert!(codes.contains(&"TEXT_OVERFLOW"), "Missing TEXT_OVERFLOW: {codes:?}");
        assert!(codes.contains(&"MISSING_ASSET"), "Missing MISSING_ASSET: {codes:?}");
        assert!(codes.contains(&"LOW_PRINT_DPI"), "Missing LOW_PRINT_DPI: {codes:?}");
        assert!(codes.contains(&"OUT_OF_BOUNDS"), "Missing OUT_OF_BOUNDS: {codes:?}");
        assert!(
            codes.contains(&"EMPTY_TEXT_FRAME"),
            "Missing EMPTY_TEXT_FRAME: {codes:?}"
        );
    }

    #[test]
    fn preflight_allows_high_dpi_images_full_bleed_frames_and_linked_chains() {
        let mut doc = BookDocument::default();
        doc.pages.clear();
        let mut page = Page::new("Clean Prepress Page", None);

        // Full-bleed background shape extending exactly to the 9 pt bleed boundary
        let bleed = doc.setup.bleed_pt;
        page.frames.push(Frame::new_shape(
            Rect::new(
                -bleed,
                -bleed,
                doc.setup.width_pt + bleed * 2.0,
                doc.setup.height_pt + bleed * 2.0,
            ),
            ShapeKind::Rectangle,
            ColorRgba::WHITE,
        ));

        // 300 DPI image (600x600 px in a 144x144 pt = 2x2 inch frame -> 300 DPI >= 250 DPI)
        let hi_dpi_id = doc.add_asset_bytes(
            "hi_res_photo",
            AssetKind::Image,
            "print_hero_300dpi.png",
            "image/png",
            vec![137, 80, 78, 71],
            Some((600, 600)),
        );
        page.frames.push(Frame::new_image(
            Rect::new(36.0, 36.0, 144.0, 144.0),
            Some(hi_dpi_id),
        ));

        // Linked 2-frame story chain with valid text
        let story = Story::new("Valid Story", "Short chapter prose.", TextStyle::default());
        let story_id = doc.insert_story(story);
        page.frames.push(Frame::new_text(
            Rect::new(36.0, 200.0, 200.0, 100.0),
            story_id,
            0,
        ));
        page.frames.push(Frame::new_text(
            Rect::new(36.0, 320.0, 200.0, 100.0),
            story_id,
            1,
        ));

        doc.pages.push(page);
        let layout = ComputedBookLayout::compute(&doc);
        let issues = run_preflight(&doc, &layout);
        assert!(
            issues.is_empty(),
            "Expected 0 issues for valid full-bleed & 300 DPI page, got: {issues:#?}"
        );
    }

    #[test]
    fn generated_front_and_back_covers_pass_preflight_with_zero_issues() {
        let articles = builtin_help_articles();
        assert_eq!(articles.len(), 9);
        let cover_hits = search_help_articles("barcode");
        assert!(cover_hits.iter().any(|a| a.id == "front-back-covers"));

        // Verify blank book with 1-click Front & Back Covers passes preflight with 0 issues
        let mut blank_doc = BookDocument::default();
        let (front_idx, back_idx) = blank_doc.generate_front_and_back_covers();
        assert_eq!(front_idx, 0);
        assert_eq!(back_idx, 1);
        let blank_layout = ComputedBookLayout::compute(&blank_doc);
        let blank_issues = run_preflight(&blank_doc, &blank_layout);
        assert!(
            blank_issues.is_empty(),
            "Expected 0 preflight issues after generate_front_and_back_covers on blank book, got: {blank_issues:#?}"
        );

        // Verify tutorial book with 1-click Front & Back Covers + Wrap-Around Cover passes preflight with 0 issues
        let mut tutorial_doc = create_tutorial_book();
        let (_, tutorial_back_idx) = tutorial_doc.generate_front_and_back_covers();
        assert_eq!(tutorial_back_idx, 4);
        let wrap_idx = tutorial_doc.generate_wraparound_cover_page(book_core::PaperStock::White);
        assert_eq!(wrap_idx, 5);
        let tutorial_layout = ComputedBookLayout::compute(&tutorial_doc);
        let tutorial_issues = run_preflight(&tutorial_doc, &tutorial_layout);
        assert!(
            tutorial_issues.is_empty(),
            "Expected 0 preflight issues on tutorial book with covers, got: {tutorial_issues:#?}"
        );
    }
}
