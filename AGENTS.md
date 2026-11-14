# AGENTS.md — Rust Book Creator (`book-creator`)

> This file is automatically loaded by Antigravity agents and `/swarm` workers. Keep rules concise and strictly follow crate ownership boundaries.

## 1. Project Mission
Build a fast, intuitive, cross-platform desktop book creator in pure Rust combining:
- **InDesign's fluid UX:** Easy drag-and-drop canvas, smart snapping guides, direct on-canvas text editing, threaded text frames across pages, and live obstacle text-wrap ("runaround") when moving images/shapes over text.
- **Scribus's openness & prepress power:** Open, portable `.book` (ZIP) and `.bookdir` (Git-friendly folder) project formats, print trim/bleed/crop marks, and preflight checks.
- **Equal-priority output:** Print-ready **PDF** (`crates/book-pdf`) and layout-accurate **EPUB 3.3** (`crates/book-epub`, supporting both Fixed-Layout `pre-paginated` and Reflowable modes).
- **Out-of-the-box usability:** Simple 5-tool UI + a built-in searchable **Help Manual (`F1`)** and an **Interactive 4-Page Starter Tutorial Book** (`crates/book-help`).

## 2. Target Platforms & Portability Rules (Arch Linux ↔ Mac M1 Air)
1. **Targets:** Must compile and run natively on **Arch Linux (`x86_64-unknown-linux-gnu`)** and **Mac M1 Air (`aarch64-apple-darwin`)** via `cargo run --release`.
2. **No Hardcoded OS Paths:** Never store absolute paths (`/home/...`, `/Users/...`) inside `.book` or `.bookdir` projects. Always use relative bundle paths (`assets/images/...`, `assets/fonts/...`) and `std::path::Path`.
3. **Bundled Fallback Font:** Never panic if system fonts are missing; always fall back cleanly so projects render identically across Linux and macOS.
4. **OS-Aware Shortcuts:** Display `Cmd` (`⌘`) on `cfg!(target_os = "macos")` and `Ctrl` on Linux/Windows.
5. **Universal Units:** All document/page/frame coordinates (`x`, `y`, `width`, `height`, `bleed`, `margin`, `font_size`) are in **typographic points (`1.0 pt = 1.0 / 72.0 inch`)**.

## 3. Swarm Crate Ownership Boundaries
When running in `/swarm`, each worker **MUST ONLY** modify files within its assigned crate(s) to guarantee zero merge conflicts:

| Worker Role | Owned Paths | Do Not Touch |
| :--- | :--- | :--- |
| **`@core`** / Phase 1 | `crates/book-core/**` | Other crates |
| **`@layout-engine`** | `crates/book-layout/**` | `book-core`, exporters, app |
| **`@exporters`** | `crates/book-pdf/**`, `crates/book-epub/**` | `book-core`, `book-layout`, `book-app` |
| **`@help-manual`** | `crates/book-help/**` | `book-core`, exporters, `book-app` |
| **`@canvas-ui`** | `crates/book-app/**` | Library crates (consume their public APIs) |
| **`@verifier`** | Read-only verification & integration tests | Author crates directly |

## 4. Definition of Done for Swarm Workers
Before posting `[done]` on the swarm board:
1. Run `cargo check --workspace` and `cargo test -p <your-crate>` and ensure all tests pass with zero errors.
2. Commit your changes cleanly in your workspace branch (`git add -A && git commit -m "..."`).
3. Post `@coordinator [done] <concise summary of APIs implemented and test output>`.
