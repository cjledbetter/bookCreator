# Book Creator (`book-creator`)

A fast, cross-platform desktop and web book creator in pure Rust combining **Adobe InDesign's fluid drag-and-drop canvas, live text wrap, and threaded text frames** with **Scribus's open file format and print prepress controls**—with equal-priority **Print PDF** and **EPUB 3.3** output.

## Supported Platforms
- **Arch Linux (`x86_64-unknown-linux-gnu`)** — Wayland / X11
- **Mac M1 Air (`aarch64-apple-darwin`)** — macOS Apple Silicon (Metal / Cocoa)
- **WebAssembly / GitHub Pages (`wasm32-unknown-unknown`)** — Runs directly in the browser via `eframe` WebGL2 + `Trunk`, with instant browser file downloads (`.book`, `.pdf`, `.epub`) and drag-and-drop file import!

## Quick Start (Arch Linux & Mac M1 Air)

```bash
# Run the desktop studio (opens with the 4-Page Interactive Starter Tutorial Book)
cargo run --release -p book-app

# Run the full workspace test suite
cargo test --workspace
```

## Running in the Browser (WebAssembly / GitHub Pages)

Every push to `main` automatically builds and deploys the WebAssembly studio to **GitHub Pages** via [`.github/workflows/build.yml`](./.github/workflows/build.yml) (make sure **Settings ▸ Pages ▸ Source** is set to **GitHub Actions**).

To build or serve the WebAssembly bundle locally with [Trunk](https://trunkrs.dev/):

```bash
rustup target add wasm32-unknown-unknown
trunk serve --open
```

## Save (`Ctrl+S` / `Cmd+S`) & Save As (`Ctrl+Shift+S` / `Cmd+Shift+S`)
- **Save (`Ctrl+S` / `Cmd+S`)**: Saves in-place to the active `.book` or `.bookdir` path on desktop (or prompts for a path on first save), and triggers an instant `.book` archive download in the browser.
- **Save As (`Ctrl+Shift+S` / `Cmd+Shift+S`)**: Opens the native Save As file dialog on desktop, or opens the in-app **Save Project As...** modal on WebAssembly to customize the filename for `.book`, `.pdf`, or `.epub` downloads.

## Portable `.book` Projects Across macOS, Linux & Web
Projects saved as `.book` (single-file ZIP bundle) or `.bookdir/` (Git-friendly unpacked directory) package all stories, page geometry, images, and fonts using strictly relative paths (`assets/images/*`, `assets/fonts/*`) so you can copy projects freely between **Mac M1 Air**, **Arch Linux**, and **GitHub Pages** without broken links or missing fonts.

## Agent & Swarm Documentation
- [AGENTS.md](./AGENTS.md) (and symlinked [GEMINI.md](./GEMINI.md)): Workspace rules and strict crate ownership boundaries for `/swarm`.
- [PLAN.md](./PLAN.md): Phased build checklist across all 6 workspace crates.
