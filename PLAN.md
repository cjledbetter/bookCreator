# PLAN.md — Phased Implementation Roadmap

This file tracks the phased build of **Book Creator (`book-creator`)** so single agents and parallel `/swarm` workers follow a single source of truth.

---

## Phase 1: Git Repository, Workspace Skeleton & Core Data Contracts (`crates/book-core`)
- [x] Initialize Git repository, `.gitignore`, `AGENTS.md` (with `GEMINI.md` symlink), and `PLAN.md`.
- [x] Create 6-crate Cargo workspace (`book-core`, `book-layout`, `book-pdf`, `book-epub`, `book-help`, `book-app`).
- [x] Implement `crates/book-core`:
  - `Point`, `Rect`, `ColorRgba`, `PageSetup` (standard presets: US Trade 6×9", US Letter 8.5×11", Children's Square 8.5×8.5", Digest 5.5×8.5", A4, A5 + bleed & margins in `pt`).
  - `BookDocument`, `MasterPage`, `Page`, `Frame` (`FrameId`, `Rect`, `rotation_deg`, `z_index`, `locked`, `WrapSettings`, `FrameKind`).
  - `FrameKind::Text` (linked to `StoryId` + `thread_index` for multi-frame text threading), `FrameKind::Image` (`AssetId`, `ImageFit`), `FrameKind::Shape` (`ShapeKind`, fill, stroke).
  - `Story` & `TextSpan` / `ParagraphStyle` (font family, size in `pt`, line height, weight, italic, alignment, color).
  - `CommandHistory` (snapshot Undo/Redo stack for drag-and-drop and text edits).
  - Portable bundle I/O: save and load both `.book` (single-file ZIP bundle) and `.bookdir/` (unpacked Git-friendly directory) with relative `assets/images/` and `assets/fonts/`.
  - `PreflightIssue` & `PreflightSeverity` types.
- [x] Define shared public layout traits/structs in `crates/book-layout` so downstream crates (`book-pdf`, `book-epub`, `book-help`, `book-app`) can compile in parallel immediately.

---

## Phase 2: Parallel `/swarm` Feature Implementation

### Track A — `@layout-engine` (`crates/book-layout`)
- [ ] Implement deterministic multi-line text shaping and wrapping (`cosmic-text` integration + fallback metrics).
- [ ] Implement **Threaded Story Flow**: flow paragraphs across ordered `FrameKind::Text` frames sharing a `StoryId` across pages, setting `is_overflowing = true` (`[+]` indicator) on the last frame if text remains.
- [ ] Implement **Live Obstacle Text-Wrap ("Runaround")**: when an Image or Shape frame with `WrapMode::BoundingBox` or `WrapMode::JumpObject` overlaps a Text frame on the same page, compute available horizontal line segments `(x_start, x_end)` per vertical slice so text wraps dynamically around dragged objects.
- [ ] Implement point-in-frame, resize-handle, and text-cursor hit-testing helpers for the canvas.
- [ ] Add unit tests for single-frame wrap, obstacle runaround reflow, and 2-page threaded story overflow.

### Track B — `@exporters` (`crates/book-pdf` & `crates/book-epub`)
- [ ] **`crates/book-pdf` (Print-Ready PDF Exporter):**
  - Render `BookDocument` + `ComputedBookLayout` to valid PDF bytes using `krilla` (or pure-Rust PDF writer).
  - Support exact page trim size, optional print bleed expansion, and corner crop/trim marks.
  - Render vector shapes (rectangles, rounded rects, ellipses, lines), embedded raster/SVG images (`image`/`resvg`), and positioned text runs with accurate point coordinates and colors.
  - Unit tests verifying valid `%PDF-` output, multi-page export, and crop mark generation.
- [ ] **`crates/book-epub` (Equal-Priority EPUB 3.3 Exporter):**
  - Generate standards-compliant EPUB 3.3 ZIP archive (`mimetype` uncompressed first entry, `META-INF/container.xml`, `OEBPS/content.opf`, `OEBPS/nav.xhtml`, `OEBPS/styles.css`).
  - **Fixed-Layout Mode (`rendition:layout="pre-paginated"`)**: Emit exact `<meta name="viewport" content="width=W, height=H"/>` per-page XHTML files with absolute-positioned frames, SVG shapes, embedded images, and `@font-face` rules so Apple Books (Mac M1 Air) and Foliate/Calibre (Arch Linux) match the canvas 1:1.
  - **Reflowable Mode**: Emit semantic chapter XHTML ordered by story threads and top-to-bottom page reading order.
  - Unit tests verifying EPUB archive structure, `mimetype` header rule, and XHTML/OPF validity.

### Track C — `@help-manual` (`crates/book-help`)
- [ ] **Built-In Searchable Help Manual:**
  - Rich, categorized offline manual covering:
    1. *Quick Start (Your First Book in 60 Seconds)*
    2. *Canvas Navigation & Drag-and-Drop Controls*
    3. *Text Frames, Typography & Threading Text Across Pages*
    4. *Live Text Wrap (Moving Images Through Text)*
    5. *Master Pages, Bleed & Print Prepress Setup*
    6. *Exporting to Print PDF vs. Fixed-Layout & Reflowable EPUB 3*
    7. *Sharing `.book` Projects Between Mac M1 Air & Arch Linux*
    8. *Keyboard Shortcuts Reference (auto-switching `⌘ Cmd` on macOS / `Ctrl` on Linux)*
  - Instant search/filter query API + reusable `egui` Help Drawer/Modal UI widget (`F1` or `? Help`).
- [ ] **Interactive 4-Page Starter Tutorial Book (`create_tutorial_book()`)**:
  - Builds a complete, visually attractive 4-page layout-heavy `BookDocument` out of the box:
    - *Page 1 (Cover Spread):* Layered title typography, decorative vector shapes, and hero illustration frame.
    - *Page 2 (Drag & Live Text Wrap):* Multi-column/rich story with a draggable badge/illustration wrapped right inside the prose so the user can drag it and watch text reflow live.
    - *Page 3 & Page 4 (Threaded Story Across Pages):* Two linked text frames demonstrating automatic story continuation across pages plus a preflight checklist callout.
- [ ] **Plain-English Preflight Engine (`run_preflight()`)**:
  - Detects overflowing text frames (`[+]`), low-DPI print images (< 300 DPI), frames crossing trim without bleed, and empty frames, returning actionable fix advice + target `(PageId, FrameId)`.

### Track D — `@canvas-ui` (`crates/book-app`)
- [ ] **`eframe` / `egui` Desktop Application:**
  - Clean dark/light studio theme with zero panel clutter.
  - **Top Toolbar:** 5 core tools (`Select/Move [V]`, `Text Frame [T]`, `Image Frame [I]`, `Shape [S]`, `Link/Thread Text [L]`), Undo/Redo, Zoom (`Fit Page`, `50%`–`400%`), `New / Open / Save .book`, `Export PDF`, `Export EPUB`, Preflight status pill, and prominent **`? Help Manual (F1)`** button.
  - **Left Rail:** Visual Page & Master Page thumbnails, add/duplicate/delete/reorder pages, quick button to reload the **4-Page Tutorial Book**.
  - **Center Interactive Spread Canvas:**
    - Pan (middle-drag / space-drag / trackpad scroll) & zoom (`Ctrl`/`Cmd` + wheel).
    - Draw new frames by clicking and dragging; select and move frames with live **Smart Alignment Snap Guides** (page center, margins, other frame edges).
    - 8 resize handles + top rotation handle on selected frame.
    - **Direct On-Canvas Text Editing:** Double-click any text frame to edit text directly on the page.
    - **Visual Threading Lines & `[+]` Overflow Port:** Click the red `[+]` port on an overflowing text frame and click another text frame to link them, drawing a clean InDesign-style thread arrow when `View Threads` or `Link Tool` is active.
    - **OS File Drag-and-Drop:** Drop `.png`/`.jpg`/`.svg` files directly from macOS Finder or Linux file manager onto the canvas to create an `ImageFrame`, or drop a `.book` file to open it.
  - **Right Context Inspector:**
    - Shows only relevant controls for the selection: Frame geometry (`X, Y, W, H, Rotation, Z-order`), **Text Wrap (`None`, `Wrap Around Box`, `Jump Object` + padding slider)**, Typography (Font, Size, Line Height, Alignment, Color), Shape style (Fill, Stroke, Corner radius), or Document Trim/Bleed setup when nothing is selected.

---

## Phase 3: Integration, Verification & Cross-Platform Check (`@verifier`)
- [ ] Verify `cargo test --workspace` and `cargo build --release` succeed with zero errors.
- [ ] Verify end-to-end test: generate the 4-page Starter Tutorial Book -> compute layout -> save `.book` archive -> reload `.book` archive -> export PDF -> export Fixed-Layout & Reflowable EPUB 3.
- [ ] Verify README includes copy-paste build & app-bundling commands for both **Arch Linux** and **Mac M1 Air**.
