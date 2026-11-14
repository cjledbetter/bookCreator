# Book Creator (`book-creator`)

A fast, cross-platform desktop book creator in pure Rust combining **Adobe InDesign's fluid drag-and-drop canvas, live text wrap, and threaded text frames** with **Scribus's open file format and print prepress controls**—with equal-priority **Print PDF** and **EPUB 3.3** output.

## Supported Platforms
- **Arch Linux (`x86_64-unknown-linux-gnu`)** — Wayland / X11
- **Mac M1 Air (`aarch64-apple-darwin`)** — macOS Apple Silicon (Metal / Cocoa)

## Quick Start (Arch Linux & Mac M1 Air)

```bash
# Run the desktop studio (opens with the 4-Page Interactive Starter Tutorial Book)
cargo run --release -p book-app

# Run the full workspace test suite
cargo test --workspace
```

## Portable `.book` Projects Across macOS & Linux
Projects saved as `.book` (single-file ZIP bundle) or `.bookdir/` (Git-friendly unpacked directory) package all stories, page geometry, images, and fonts using strictly relative paths (`assets/images/*`, `assets/fonts/*`) so you can copy projects freely between **Mac M1 Air** and **Arch Linux** without broken links or missing fonts.

## Agent & Swarm Documentation
- [AGENTS.md](./AGENTS.md) (and symlinked [GEMINI.md](./GEMINI.md)): Workspace rules and strict crate ownership boundaries for `/swarm`.
- [PLAN.md](./PLAN.md): Phased build checklist across all 6 workspace crates.
