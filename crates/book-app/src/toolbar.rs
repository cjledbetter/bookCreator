//! Top 5-tool studio toolbar (Tools, Undo/Redo, File I/O, Equal-Priority PDF & EPUB 3
//! Export, Live Preflight badge, Zoom, View toggles, Help Manual button) and Left Page
//! & Master Page Rail with miniature visual frame thumbnails.

use book_core::{BookDocument, FrameKind, PagePreset, PreflightSeverity, ShapeKind};
use book_epub::{export_epub_to_file, EpubLayoutMode};
use book_help::primary_modifier_label;
use book_pdf::{export_pdf_to_file, PdfExportOptions};
use eframe::egui;

use crate::canvas::to_color32;
use crate::{BookCreatorApp, Tool};

/// Render the top studio toolbar and process global keyboard shortcuts.
pub fn show_top_toolbar(app: &mut BookCreatorApp, ctx: &egui::Context) {
    process_keyboard_shortcuts(app, ctx);

    egui::TopBottomPanel::top("top_studio_toolbar")
        .exact_height(46.0)
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;

                // --- 1. File Actions Menu & Quick Buttons ---
                ui.menu_button("📁 File", |ui| {
                    ui.set_min_width(210.0);
                    if ui.button("📄 New Blank Book").clicked() {
                        app.new_blank_book();
                        ui.close_menu();
                    }
                    if ui
                        .button("🎓 Load 4-Page Tutorial Book")
                        .on_hover_text("Load the interactive 4-page starter tutorial book")
                        .clicked()
                    {
                        app.load_tutorial_book();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .button("📂 Open .book Archive...")
                        .on_hover_text("Open a portable single-file .book archive")
                        .clicked()
                    {
                        ui.close_menu();
                        open_book_archive_dialog(app);
                    }
                    if ui
                        .button("💾 Save .book Archive...")
                        .on_hover_text("Save portable single-file .book archive (Mac M1 Air & Arch Linux)")
                        .clicked()
                    {
                        ui.close_menu();
                        save_book_archive_dialog(app);
                    }
                    if ui
                        .button("🗂 Save .bookdir Folder (Git-friendly)...")
                        .on_hover_text("Save unpacked .bookdir directory for Git version control")
                        .clicked()
                    {
                        ui.close_menu();
                        save_bookdir_dialog(app);
                    }
                    if ui
                        .button("📂 Open .bookdir Folder...")
                        .on_hover_text("Open an unpacked .bookdir directory")
                        .clicked()
                    {
                        ui.close_menu();
                        open_bookdir_dialog(app);
                    }
                });

                if ui
                    .button("🎓 Tutorial")
                    .on_hover_text("Reset / Reload the 4-Page Interactive Starter Tutorial Book")
                    .clicked()
                {
                    app.load_tutorial_book();
                }

                // --- 2. Undo / Redo ---
                let mod_key = primary_modifier_label();
                if ui
                    .add_enabled(app.history.can_undo(), egui::Button::new("↶ Undo"))
                    .on_hover_text(format!("Undo ({mod_key}+Z)"))
                    .clicked()
                {
                    app.undo();
                }
                if ui
                    .add_enabled(app.history.can_redo(), egui::Button::new("↷ Redo"))
                    .on_hover_text(format!("Redo ({mod_key}+Y / {mod_key}+Shift+Z)"))
                    .clicked()
                {
                    app.redo();
                }

                ui.separator();

                // --- 3. The 5 Core Layout Tools ---
                let tools = [
                    (
                        Tool::SelectMove,
                        "↖ Select (V)",
                        "Select, move, resize, or double-click text frames to edit (Shortcut: V)",
                    ),
                    (
                        Tool::TextFrame,
                        "Ｔ Text (T)",
                        "Click and drag on the page to create a Text Frame (Shortcut: T)",
                    ),
                    (
                        Tool::ImageFrame,
                        "🖼 Image (I)",
                        "Click and drag on the page to create an Image Frame (Shortcut: I)",
                    ),
                    (
                        Tool::ShapeFrame,
                        "⬛ Shape (S)",
                        "Click and drag on the page to draw a Shape Frame (Shortcut: S)",
                    ),
                    (
                        Tool::LinkText,
                        "🔗 Link Text (L)",
                        "Click source Text Frame then target Text Frame to thread story across frames/pages (Shortcut: L)",
                    ),
                ];

                for (tool, label, tooltip) in tools {
                    let selected = app.active_tool == tool;
                    let btn = egui::Button::new(label).selected(selected);
                    if ui.add(btn).on_hover_text(tooltip).clicked() {
                        app.select_tool(tool);
                    }
                }

                if app.active_tool == Tool::ShapeFrame {
                    egui::ComboBox::from_id_salt("toolbar_shape_kind")
                        .width(105.0)
                        .selected_text(match app.shape_tool_kind {
                            ShapeKind::Rectangle => "Rectangle",
                            ShapeKind::RoundedRectangle => "Rounded",
                            ShapeKind::Ellipse => "Ellipse",
                            ShapeKind::DividerLine => "Line",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut app.shape_tool_kind,
                                ShapeKind::RoundedRectangle,
                                "Rounded Rect",
                            );
                            ui.selectable_value(
                                &mut app.shape_tool_kind,
                                ShapeKind::Rectangle,
                                "Rectangle",
                            );
                            ui.selectable_value(
                                &mut app.shape_tool_kind,
                                ShapeKind::Ellipse,
                                "Ellipse",
                            );
                            ui.selectable_value(
                                &mut app.shape_tool_kind,
                                ShapeKind::DividerLine,
                                "Divider Line",
                            );
                        });
                }

                ui.separator();

                // --- 4. Zoom & View Toggles ---
                if ui
                    .button("Fit")
                    .on_hover_text("Fit active page comfortably in the viewport")
                    .clicked()
                {
                    app.fit_page_in_viewport();
                }
                if ui.button("−").on_hover_text("Zoom Out").clicked() {
                    app.zoom = (app.zoom * 0.85).clamp(0.25, 4.0);
                }
                if ui
                    .button(format!("{:.0}%", app.zoom * 100.0))
                    .on_hover_text("Click to reset zoom to 100%")
                    .clicked()
                {
                    app.zoom = 1.0;
                    app.pan_offset = egui::Vec2::ZERO;
                }
                if ui.button("+").on_hover_text("Zoom In").clicked() {
                    app.zoom = (app.zoom * 1.15).clamp(0.25, 4.0);
                }

                ui.menu_button("👁 View", |ui| {
                    ui.checkbox(
                        &mut app.show_margins_and_bleed,
                        "Show Safe Margins & Bleed Border",
                    );
                    ui.checkbox(
                        &mut app.show_thread_lines,
                        "Show Text Threading Arrows",
                    );
                });

                ui.separator();

                // --- 5. Equal-Priority Print PDF & EPUB 3 Exporters ---
                ui.menu_button("🖨 Export PDF", |ui| {
                    ui.set_min_width(220.0);
                    ui.label(egui::RichText::new("Print-Ready PDF Setup").strong());
                    ui.checkbox(
                        &mut app.pdf_include_bleed,
                        format!("Include Print Bleed ({:.1} pt)", app.doc.setup.bleed_pt),
                    );
                    ui.checkbox(
                        &mut app.pdf_include_crop_marks,
                        "Include Corner Crop / Trim Marks",
                    );
                    ui.separator();
                    if ui
                        .button("🖨 Export Print PDF Now...")
                        .clicked()
                    {
                        ui.close_menu();
                        export_pdf_dialog(app);
                    }
                });

                ui.menu_button("📖 Export EPUB 3", |ui| {
                    ui.set_min_width(240.0);
                    ui.label(egui::RichText::new("EPUB 3.3 Layout Mode").strong());
                    ui.radio_value(
                        &mut app.epub_layout_mode,
                        EpubLayoutMode::FixedLayout,
                        "Fixed-Layout (pre-paginated 1:1 visual)",
                    );
                    ui.radio_value(
                        &mut app.epub_layout_mode,
                        EpubLayoutMode::Reflowable,
                        "Reflowable (eReader chapter flow)",
                    );
                    ui.separator();
                    if ui
                        .button("📖 Export EPUB 3.3 Now...")
                        .clicked()
                    {
                        ui.close_menu();
                        export_epub_dialog(app);
                    }
                });

                // --- 6. Live Preflight Badge & Prominent `? Help Manual (F1)` ---
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let help_btn = egui::Button::new(
                        egui::RichText::new("? Help Manual (F1)")
                            .strong()
                            .color(egui::Color32::WHITE),
                    )
                    .fill(egui::Color32::from_rgb(37, 99, 235));
                    if ui
                        .add(help_btn)
                        .on_hover_text("Open searchable Help Manual, OS shortcuts & tutorials (Shortcut: F1)")
                        .clicked()
                    {
                        app.show_help = !app.show_help;
                    }

                    let issue_count = app.preflight_issues.len();
                    let has_error = app
                        .preflight_issues
                        .iter()
                        .any(|i| i.severity == PreflightSeverity::Error);
                    let (pill_text, pill_color) = if issue_count == 0 {
                        ("✓ 0 Issues".to_string(), egui::Color32::from_rgb(22, 163, 74))
                    } else if has_error {
                        (
                            format!("⛔ {issue_count} Issues"),
                            egui::Color32::from_rgb(220, 38, 38),
                        )
                    } else {
                        (
                            format!("⚠ {issue_count} Warning{}", if issue_count == 1 { "" } else { "s" }),
                            egui::Color32::from_rgb(217, 119, 6),
                        )
                    };

                    let preflight_btn = egui::Button::new(
                        egui::RichText::new(pill_text)
                            .strong()
                            .color(egui::Color32::WHITE),
                    )
                    .fill(pill_color);
                    if ui
                        .add(preflight_btn)
                        .on_hover_text("Click to toggle Live Preflight Verifier & jump to issues")
                        .clicked()
                    {
                        app.show_preflight_drawer = !app.show_preflight_drawer;
                    }
                });
            });
        });
}

/// Render the Left Page & Master Page Rail with miniature visual page cards.
pub fn show_left_page_rail(app: &mut BookCreatorApp, ctx: &egui::Context) {
    egui::SidePanel::left("left_page_rail")
        .resizable(true)
        .default_width(215.0)
        .min_width(185.0)
        .max_width(290.0)
        .show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading("Pages");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("{} total", app.doc.pages.len()))
                            .small()
                            .weak(),
                    );
                });
            });

            // Page action buttons
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if ui
                    .small_button("+ Add")
                    .on_hover_text("Append a new blank page")
                    .clicked()
                {
                    app.add_page();
                }
                if ui
                    .small_button("⧉ Dup")
                    .on_hover_text("Duplicate current page and its frames")
                    .clicked()
                {
                    app.duplicate_current_page();
                }
                if ui
                    .add_enabled(app.doc.pages.len() > 1, egui::Button::new("🗑 Del").small())
                    .on_hover_text("Delete current page")
                    .clicked()
                {
                    app.delete_current_page();
                }
                if ui
                    .add_enabled(app.current_page > 0, egui::Button::new("↑").small())
                    .on_hover_text("Move current page earlier")
                    .clicked()
                {
                    app.move_current_page_up();
                }
                if ui
                    .add_enabled(
                        app.current_page + 1 < app.doc.pages.len(),
                        egui::Button::new("↓").small(),
                    )
                    .on_hover_text("Move current page later")
                    .clicked()
                {
                    app.move_current_page_down();
                }
            });

            ui.separator();

            // Scrollable miniature visual page thumbnail cards
            let page_w = app.doc.setup.width_pt.max(100.0);
            let page_h = app.doc.setup.height_pt.max(100.0);
            let aspect = page_h / page_w;

            egui::ScrollArea::vertical()
                .id_salt("page_rail_scroll")
                .max_height((ui.available_height() - 185.0).max(160.0))
                .show(ui, |ui| {
                    let mut clicked_page = None;
                    for (idx, page) in app.doc.pages.iter().enumerate() {
                        let is_active = idx == app.current_page;
                        let card_fill = if is_active {
                            egui::Color32::from_rgb(38, 56, 88)
                        } else {
                            egui::Color32::from_rgb(32, 35, 43)
                        };
                        let border_color = if is_active {
                            egui::Color32::from_rgb(56, 139, 253)
                        } else {
                            egui::Color32::from_rgb(58, 63, 75)
                        };

                        egui::Frame::NONE
                            .fill(card_fill)
                            .stroke(egui::Stroke::new(
                                if is_active { 1.8_f32 } else { 1.0_f32 },
                                border_color,
                            ))
                            .corner_radius(6.0)
                            .inner_margin(8.0)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    // Miniature Page Preview Canvas
                                    let thumb_w = 54.0;
                                    let thumb_h = (thumb_w * aspect).clamp(44.0, 82.0);
                                    let (thumb_rect, thumb_resp) = ui.allocate_exact_size(
                                        egui::vec2(thumb_w, thumb_h),
                                        egui::Sense::click(),
                                    );
                                    if thumb_resp.clicked() {
                                        clicked_page = Some(idx);
                                    }

                                    let p = ui.painter_at(thumb_rect);
                                    p.rect_filled(thumb_rect, 2.0, to_color32(page.background_color));
                                    p.rect_stroke(
                                        thumb_rect,
                                        2.0,
                                        egui::Stroke::new(
                                            1.0_f32,
                                            egui::Color32::from_rgb(90, 98, 112),
                                        ),
                                        egui::StrokeKind::Inside,
                                    );

                                    let sx = thumb_w / page_w;
                                    let sy = thumb_h / page_h;
                                    for frame in page.frames_sorted_by_z() {
                                        let mini_rect = egui::Rect::from_min_size(
                                            egui::pos2(
                                                thumb_rect.left() + frame.rect.x * sx,
                                                thumb_rect.top() + frame.rect.y * sy,
                                            ),
                                            egui::vec2(
                                                (frame.rect.width * sx).max(2.0),
                                                (frame.rect.height * sy).max(2.0),
                                            ),
                                        )
                                        .intersect(thumb_rect);

                                        let mini_color = match &frame.kind {
                                            FrameKind::Shape { fill, .. } => to_color32(*fill),
                                            FrameKind::Image { .. } => {
                                                egui::Color32::from_rgb(135, 175, 225)
                                            }
                                            FrameKind::Text { .. } => {
                                                egui::Color32::from_rgba_unmultiplied(70, 85, 110, 115)
                                            }
                                        };
                                        p.rect_filled(mini_rect, 1.0, mini_color);
                                    }

                                    // Page title & frame count summary
                                    ui.vertical(|ui| {
                                        let title_resp = ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(format!(
                                                    "Page {}",
                                                    idx + 1
                                                ))
                                                .strong()
                                                .color(if is_active {
                                                    egui::Color32::WHITE
                                                } else {
                                                    egui::Color32::LIGHT_GRAY
                                                }),
                                            )
                                            .sense(egui::Sense::click()),
                                        );
                                        if title_resp.clicked() {
                                            clicked_page = Some(idx);
                                        }
                                        let sub_resp = ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(&page.title).small(),
                                            )
                                            .sense(egui::Sense::click()),
                                        );
                                        if sub_resp.clicked() {
                                            clicked_page = Some(idx);
                                        }
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "{} frame{}",
                                                page.frames.len(),
                                                if page.frames.len() == 1 { "" } else { "s" }
                                            ))
                                            .small()
                                            .weak(),
                                        );
                                    });
                                });
                            });
                        ui.add_space(4.0);
                    }

                    if let Some(target_idx) = clicked_page {
                        if app.current_page != target_idx {
                            app.current_page = target_idx;
                            app.selected_frame_id = None;
                            app.editing_text_frame_id = None;
                        }
                    }
                });

            ui.separator();

            // Master Page & Page Preset Section
            ui.label(egui::RichText::new("Master Page & Size Preset").strong());
            let mut preset_changed = None;
            let current_preset = app.doc.setup.preset;
            egui::ComboBox::from_id_salt("left_rail_preset_combo")
                .width(ui.available_width() - 8.0)
                .selected_text(current_preset.label())
                .show_ui(ui, |ui| {
                    let presets = [
                        PagePreset::UsTrade6x9,
                        PagePreset::UsLetter,
                        PagePreset::Square85x85,
                        PagePreset::Digest55x85,
                        PagePreset::IsoA4,
                        PagePreset::IsoA5,
                        PagePreset::Custom,
                    ];
                    for p in presets {
                        if ui
                            .selectable_label(current_preset == p, p.label())
                            .clicked()
                        {
                            preset_changed = Some(p);
                        }
                    }
                });

            if let Some(new_preset) = preset_changed {
                app.record_undo_snapshot();
                app.doc.setup.preset = new_preset;
                if new_preset != PagePreset::Custom {
                    let (w, h) = new_preset.dimensions_pt();
                    app.doc.setup.width_pt = w;
                    app.doc.setup.height_pt = h;
                }
                app.recompute_layout_and_preflight();
            }

            if let Some(master) = app.doc.master_pages.first_mut() {
                ui.checkbox(&mut master.show_page_numbers, "Show Master Page Numbers");
                ui.horizontal(|ui| {
                    ui.label("Header:");
                    ui.text_edit_singleline(&mut master.header_text);
                });
            }
        });
}

fn process_keyboard_shortcuts(app: &mut BookCreatorApp, ctx: &egui::Context) {
    // Toggle Help Manual with F1 anywhere
    if ctx.input(|i| i.key_pressed(egui::Key::F1)) {
        app.show_help = !app.show_help;
    }

    // Do not intercept single-letter tool shortcuts while typing in a text field
    if ctx.wants_keyboard_input() {
        return;
    }

    let modifiers = ctx.input(|i| i.modifiers);
    let cmd_or_ctrl = modifiers.command || modifiers.ctrl;

    if cmd_or_ctrl && ctx.input(|i| i.key_pressed(egui::Key::Z)) {
        if modifiers.shift {
            app.redo();
        } else {
            app.undo();
        }
        return;
    }
    if cmd_or_ctrl && ctx.input(|i| i.key_pressed(egui::Key::Y)) {
        app.redo();
        return;
    }

    if ctx.input(|i| i.key_pressed(egui::Key::V)) {
        app.select_tool(Tool::SelectMove);
    } else if ctx.input(|i| i.key_pressed(egui::Key::T)) {
        app.select_tool(Tool::TextFrame);
    } else if ctx.input(|i| i.key_pressed(egui::Key::I)) {
        app.select_tool(Tool::ImageFrame);
    } else if ctx.input(|i| i.key_pressed(egui::Key::S)) {
        app.select_tool(Tool::ShapeFrame);
    } else if ctx.input(|i| i.key_pressed(egui::Key::L)) {
        app.select_tool(Tool::LinkText);
    } else if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.select_tool(Tool::SelectMove);
        app.selected_frame_id = None;
        app.editing_text_frame_id = None;
        app.link_source_frame_id = None;
    } else if ctx.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
    {
        app.delete_selected_frame();
    }
}

pub fn open_book_archive_dialog(app: &mut BookCreatorApp) {
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("Book Project (.book)", &["book"])
        .pick_file()
    {
        match BookDocument::load_book_archive(&path) {
            Ok(doc) => {
                app.record_undo_snapshot();
                app.doc = doc;
                app.current_page = 0;
                app.selected_frame_id = None;
                app.recompute_layout_and_preflight();
                app.status_message = format!("Opened '{}'", path.display());
            }
            Err(err) => {
                app.status_message = format!("Failed to open .book: {err}");
            }
        }
    }
}

pub fn save_book_archive_dialog(app: &mut BookCreatorApp) {
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("Book Project (.book)", &["book"])
        .set_file_name("project.book")
        .save_file()
    {
        match app.doc.save_book_archive(&path) {
            Ok(()) => {
                app.status_message = format!("Saved portable .book to '{}'", path.display());
            }
            Err(err) => {
                app.status_message = format!("Failed to save .book: {err}");
            }
        }
    }
}

pub fn save_bookdir_dialog(app: &mut BookCreatorApp) {
    if let Some(folder) = rfd::FileDialog::new()
        .set_file_name("project.bookdir")
        .save_file()
    {
        match app.doc.save_book_directory(&folder) {
            Ok(()) => {
                app.status_message =
                    format!("Saved Git-friendly .bookdir to '{}'", folder.display());
            }
            Err(err) => {
                app.status_message = format!("Failed to save .bookdir: {err}");
            }
        }
    }
}

pub fn open_bookdir_dialog(app: &mut BookCreatorApp) {
    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
        match BookDocument::load_book_directory(&folder) {
            Ok(doc) => {
                app.record_undo_snapshot();
                app.doc = doc;
                app.current_page = 0;
                app.selected_frame_id = None;
                app.recompute_layout_and_preflight();
                app.status_message = format!("Opened .bookdir '{}'", folder.display());
            }
            Err(err) => {
                app.status_message = format!("Failed to open .bookdir: {err}");
            }
        }
    }
}

pub fn export_pdf_dialog(app: &mut BookCreatorApp) {
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("Print-Ready PDF (.pdf)", &["pdf"])
        .set_file_name("book-print.pdf")
        .save_file()
    {
        let options = PdfExportOptions {
            include_bleed: app.pdf_include_bleed,
            include_crop_marks: app.pdf_include_crop_marks,
        };
        match export_pdf_to_file(&app.doc, &app.layout, options, &path) {
            Ok(()) => {
                app.status_message = format!(
                    "Exported Print PDF (bleed={}, crop_marks={}) to '{}'",
                    options.include_bleed,
                    options.include_crop_marks,
                    path.display()
                );
            }
            Err(err) => {
                app.status_message = format!("PDF export error: {err}");
            }
        }
    }
}

pub fn export_epub_dialog(app: &mut BookCreatorApp) {
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("EPUB 3.3 eBook (.epub)", &["epub"])
        .set_file_name("book.epub")
        .save_file()
    {
        match export_epub_to_file(&app.doc, &app.layout, app.epub_layout_mode, &path) {
            Ok(()) => {
                let mode_str = match app.epub_layout_mode {
                    EpubLayoutMode::FixedLayout => "Fixed-Layout",
                    EpubLayoutMode::Reflowable => "Reflowable",
                };
                app.status_message =
                    format!("Exported {mode_str} EPUB 3.3 to '{}'", path.display());
            }
            Err(err) => {
                app.status_message = format!("EPUB export error: {err}");
            }
        }
    }
}
