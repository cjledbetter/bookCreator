//! Top 5-tool studio toolbar (Tools, Undo/Redo, File I/O, Equal-Priority PDF & EPUB 3
//! Export, Live Preflight badge, Zoom, View toggles, Help Manual button) and Left Page
//! & Master Page Rail with miniature visual frame thumbnails.

use book_core::{BookDocument, FrameKind, PagePreset, PaperStock, PreflightSeverity, ShapeKind};
use book_epub::{export_epub_bytes, EpubLayoutMode};
use book_help::primary_modifier_label;
use book_pdf::{export_pdf_bytes, PdfExportOptions};
use eframe::egui;

use crate::canvas::to_color32;
use crate::{default_filename_for_title, ensure_extension, BookCreatorApp, Tool};

/// Render the top studio toolbar and process global keyboard shortcuts.
pub fn show_top_toolbar(app: &mut BookCreatorApp, ctx: &egui::Context) {
    process_keyboard_shortcuts(app, ctx);
    let mod_key = primary_modifier_label();

    egui::TopBottomPanel::top("top_studio_toolbar")
        .exact_height(46.0)
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;

                // --- 1. File Actions Menu & Quick Buttons ---
                ui.menu_button("📁 File", |ui| {
                    ui.set_min_width(250.0);
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
                        .on_hover_text("Open a portable single-file .book archive (or drag & drop a .book file onto the window)")
                        .clicked()
                    {
                        ui.close_menu();
                        open_book_archive_dialog(app);
                    }
                    if ui
                        .button(format!("💾 Save ({mod_key}+S)"))
                        .on_hover_text("Save current project in-place (or download .book in browser)")
                        .clicked()
                    {
                        ui.close_menu();
                        let _ = app.save_project();
                    }
                    if ui
                        .button(format!("💾 Save As .book... ({mod_key}+Shift+S)"))
                        .on_hover_text("Save portable single-file .book archive under a new name/path")
                        .clicked()
                    {
                        ui.close_menu();
                        trigger_save_as_action(app);
                    }
                    if ui
                        .button("✏ Save Project As (Custom Filename)...")
                        .on_hover_text("Choose custom filename to download/save .book, .pdf, or .epub")
                        .clicked()
                    {
                        ui.close_menu();
                        app.show_save_as_modal = true;
                    }
                    ui.separator();
                    if ui
                        .button("🗂 Save As .bookdir Folder (Git-friendly)...")
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
                    .button("💾 Save")
                    .on_hover_text(format!(
                        "Save Project ({mod_key}+S) — or use File ▸ Save As ({mod_key}+Shift+S)"
                    ))
                    .clicked()
                {
                    let _ = app.save_project();
                }

                if ui
                    .button("🎓 Tutorial")
                    .on_hover_text("Reset / Reload the 4-Page Interactive Starter Tutorial Book")
                    .clicked()
                {
                    app.load_tutorial_book();
                }

                // --- 2. Undo / Redo ---
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

                // --- 5. Covers & Equal-Priority Print PDF / EPUB 3 Exporters ---
                ui.menu_button("📕 Covers ▾", |ui| {
                    ui.set_min_width(290.0);
                    ui.label(egui::RichText::new("Front, Back & Wrap-Around Covers").strong());
                    if ui
                        .button("✨ Generate Front & Back Cover (1-Click)")
                        .on_hover_text(
                            "Ensure Page 1 is a Front Cover and append/update a Back Cover with Blurb, Author Bio & 2.0×1.2 in ISBN Barcode Box",
                        )
                        .clicked()
                    {
                        app.generate_front_and_back_covers();
                        ui.close_menu();
                    }

                    ui.horizontal(|ui| {
                        if ui.button("⬅ Jump to Front Cover").clicked() {
                            app.jump_to_front_cover();
                            ui.close_menu();
                        }
                        if ui.button("➡ Jump to Back Cover").clicked() {
                            app.jump_to_back_cover();
                            ui.close_menu();
                        }
                    });

                    if let Some(page) = app.doc.pages.get(app.current_page) {
                        let mut is_cover = page.master_id.is_none();
                        if ui
                            .checkbox(
                                &mut is_cover,
                                "Cover Page (Hide Page Numbers & Headers)",
                            )
                            .changed()
                        {
                            let cur = app.current_page;
                            app.set_page_is_cover(cur, is_cover);
                        }
                    }

                    ui.separator();
                    ui.label(egui::RichText::new("Wrap-Around Print Jacket & Spine").strong());
                    ui.horizontal(|ui| {
                        ui.label("Paper Stock:");
                        for stock in [
                            PaperStock::White,
                            PaperStock::Cream,
                            PaperStock::ColorIllustrated,
                        ] {
                            ui.selectable_value(
                                &mut app.cover_paper_stock,
                                stock,
                                stock.short_label(),
                            );
                        }
                    });

                    let page_count = app.doc.pages.len();
                    let spine_pt = app.doc.calculate_spine_width_pt(app.cover_paper_stock);
                    let spine_in = spine_pt / 72.0;
                    let spine_mm = spine_in * 25.4;
                    ui.label(
                        egui::RichText::new(format!(
                            "Spine Width ({} pgs, min 24): {:.2} pt | {:.3} in | {:.2} mm",
                            page_count, spine_pt, spine_in, spine_mm
                        ))
                        .small()
                        .color(egui::Color32::from_rgb(232, 168, 56)),
                    );

                    if ui
                        .button("📏 Add Wrap-Around Cover (Back + Spine + Front)")
                        .on_hover_text(
                            "Append a complete wrap-around cover spread with Back Cover, calculated Spine strip, and Front Cover",
                        )
                        .clicked()
                    {
                        app.generate_wraparound_cover();
                        ui.close_menu();
                    }
                });

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

            // Quick 1-Click Cover Buttons in Left Page Rail
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                if ui
                    .small_button("✨ Generate Front & Back Cover (1-Click)")
                    .on_hover_text(
                        "Create Front Cover (Page 1) and Back Cover (final page with Blurb, Author Bio & ISBN Barcode)",
                    )
                    .clicked()
                {
                    app.generate_front_and_back_covers();
                }
                if ui
                    .small_button("📏 Wrap-Around")
                    .on_hover_text("Append a Wrap-Around Cover (Back + Spine + Front)")
                    .clicked()
                {
                    app.generate_wraparound_cover();
                }
                if ui
                    .small_button("Front")
                    .on_hover_text("Jump to Front Cover (Page 1)")
                    .clicked()
                {
                    app.jump_to_front_cover();
                }
                if ui
                    .small_button("Back")
                    .on_hover_text("Jump to Back Cover")
                    .clicked()
                {
                    app.jump_to_back_cover();
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

            if let Some(page) = app.doc.pages.get(app.current_page) {
                let mut is_cover = page.master_id.is_none();
                if ui
                    .checkbox(
                        &mut is_cover,
                        "Cover Page (Hide Page Numbers & Headers)",
                    )
                    .changed()
                {
                    let cur = app.current_page;
                    app.set_page_is_cover(cur, is_cover);
                }
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

    let modifiers = ctx.input(|i| i.modifiers);
    let cmd_or_ctrl = modifiers.command || modifiers.ctrl;

    // Ctrl+S / Cmd+S (Save) and Ctrl+Shift+S / Cmd+Shift+S (Save As...) work globally
    if cmd_or_ctrl && ctx.input(|i| i.key_pressed(egui::Key::S)) {
        if modifiers.shift {
            trigger_save_as_action(app);
        } else {
            let _ = app.save_project();
        }
        return;
    }

    // Do not intercept single-letter tool shortcuts while typing in a text field
    if ctx.wants_keyboard_input() {
        return;
    }

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

/// Trigger "Save As..." — opens native file dialog on Desktop or the Save As modal on WebAssembly.
pub fn trigger_save_as_action(app: &mut BookCreatorApp) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        save_book_archive_as_dialog(app);
    }
    #[cfg(target_arch = "wasm32")]
    {
        if app.save_as_filename.trim().is_empty() {
            app.save_as_filename = default_filename_for_title(&app.doc.metadata.title, "book");
        }
        app.show_save_as_modal = true;
    }
}

/// Render the in-app "Save Project As..." modal window (available on WebAssembly and Desktop).
pub fn show_save_as_modal_window(app: &mut BookCreatorApp, ctx: &egui::Context) {
    if !app.show_save_as_modal {
        return;
    }

    let mut open = app.show_save_as_modal;
    let mut action_book = false;
    let mut action_pdf = false;
    let mut action_epub = false;
    let mut close_requested = false;

    egui::Window::new("💾 Save Project As...")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(380.0)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            ui.label(
                "Enter a filename for your portable .book bundle or exported publication:",
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("Filename:");
                ui.text_edit_singleline(&mut app.save_as_filename);
            });
            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .button(egui::RichText::new("💾 Download .book").strong())
                    .on_hover_text("Save or download portable .book archive with this filename")
                    .clicked()
                {
                    action_book = true;
                }
                if ui
                    .button("🖨 Export PDF")
                    .on_hover_text("Export print-ready PDF with this filename")
                    .clicked()
                {
                    action_pdf = true;
                }
                if ui
                    .button("📖 Export EPUB 3")
                    .on_hover_text("Export EPUB 3.3 eBook with this filename")
                    .clicked()
                {
                    action_epub = true;
                }
                if ui.button("Cancel").clicked() {
                    close_requested = true;
                }
            });
        });

    if action_book {
        let filename = ensure_extension(&app.save_as_filename, "book");
        app.save_as_filename = filename.clone();
        match app.doc.to_book_archive_bytes() {
            Ok(bytes) => {
                match save_or_download_bytes(&filename, "Book Project (.book)", "book", &bytes) {
                    Ok(saved_target) => {
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            app.current_project_path =
                                Some(std::path::PathBuf::from(&saved_target));
                        }
                        app.status_message = format!("Saved portable .book to '{saved_target}'");
                    }
                    Err(err) if err != "Cancelled" => {
                        app.status_message = format!("Failed to save .book: {err}");
                    }
                    _ => {}
                }
            }
            Err(err) => {
                app.status_message = format!("Failed to serialize .book: {err}");
            }
        }
        open = false;
    } else if action_pdf {
        export_pdf_dialog(app);
        open = false;
    } else if action_epub {
        export_epub_dialog(app);
        open = false;
    } else if close_requested {
        open = false;
    }

    app.show_save_as_modal = open;
}

/// Cross-platform byte saver:
/// - On native desktop (`not(target_arch = "wasm32")`): opens `rfd::FileDialog` and writes `bytes` to disk.
/// - On WebAssembly (`target_arch = "wasm32"`): creates a `js_sys::Uint8Array`, wraps it in a `web_sys::Blob`,
///   creates an object URL, clicks a temporary `<a download="...">` element to trigger an instant browser download,
///   and revokes the URL.
pub fn save_or_download_bytes(
    default_filename: &str,
    filter_name: &str,
    ext: &str,
    bytes: &[u8],
) -> Result<String, String> {
    let filename = ensure_extension(default_filename, ext);

    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(mut path) = rfd::FileDialog::new()
            .set_file_name(&filename)
            .add_filter(filter_name, &[ext])
            .save_file()
        else {
            return Err("Cancelled".to_string());
        };
        if path.extension().is_none() {
            path.set_extension(ext);
        }
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        Ok(path.display().to_string())
    }

    #[cfg(target_arch = "wasm32")]
    {
        let _ = filter_name;
        let mime_type = match ext {
            "pdf" => "application/pdf",
            "epub" => "application/epub+zip",
            "book" => "application/zip",
            _ => "application/octet-stream",
        };
        trigger_browser_download(&filename, mime_type, bytes)
    }
}

#[cfg(target_arch = "wasm32")]
pub fn trigger_browser_download(
    filename: &str,
    mime_type: &str,
    bytes: &[u8],
) -> Result<String, String> {
    use wasm_bindgen::JsCast as _;

    let window = web_sys::window().ok_or_else(|| "No browser window".to_string())?;
    let document = window
        .document()
        .ok_or_else(|| "No browser document".to_string())?;

    let uint8_array = js_sys::Uint8Array::from(bytes);
    let blob_parts = js_sys::Array::new();
    blob_parts.push(&uint8_array);

    let options = web_sys::BlobPropertyBag::new();
    options.set_type(mime_type);

    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&blob_parts, &options)
        .map_err(|e| format!("Failed to create Blob: {e:?}"))?;

    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|e| format!("Failed to create Object URL: {e:?}"))?;

    let elem = document
        .create_element("a")
        .map_err(|e| format!("Failed to create anchor element: {e:?}"))?;
    let anchor = elem
        .dyn_into::<web_sys::HtmlAnchorElement>()
        .map_err(|_| "Element is not an HtmlAnchorElement".to_string())?;

    anchor.set_href(&url);
    anchor.set_download(filename);
    if let Some(body) = document.body() {
        let _ = body.append_child(&anchor);
    }
    anchor.click();
    anchor.remove();

    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(filename.to_string())
}

pub fn open_book_archive_dialog(app: &mut BookCreatorApp) {
    #[cfg(not(target_arch = "wasm32"))]
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
                app.editing_text_frame_id = None;
                app.current_project_path = Some(path.clone());
                if let Some(fname) = path.file_name().and_then(|s| s.to_str()) {
                    app.save_as_filename = fname.to_string();
                }
                app.recompute_layout_and_preflight();
                app.status_message = format!("Opened '{}'", path.display());
            }
            Err(err) => {
                app.status_message = format!("Failed to open .book: {err}");
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        app.status_message =
            "Tip (Web): Drag and drop any .book project archive directly from your computer onto this browser window!"
                .to_string();
    }
}

pub fn save_book_archive_dialog(app: &mut BookCreatorApp) {
    save_book_archive_as_dialog(app);
}

pub fn save_book_archive_as_dialog(app: &mut BookCreatorApp) {
    let default_name = if app.save_as_filename.trim().is_empty() {
        default_filename_for_title(&app.doc.metadata.title, "book")
    } else {
        ensure_extension(&app.save_as_filename, "book")
    };

    #[cfg(not(target_arch = "wasm32"))]
    if let Some(mut path) = rfd::FileDialog::new()
        .add_filter("Book Project (.book)", &["book"])
        .set_file_name(&default_name)
        .save_file()
    {
        if path.extension().is_none() {
            path.set_extension("book");
        }
        let _ = app.save_project_as_path(&path);
    }

    #[cfg(target_arch = "wasm32")]
    {
        app.save_as_filename = default_name;
        app.show_save_as_modal = true;
    }
}

pub fn save_bookdir_dialog(app: &mut BookCreatorApp) {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(folder) = rfd::FileDialog::new()
        .set_file_name("project.bookdir")
        .save_file()
    {
        match app.doc.save_book_directory(&folder) {
            Ok(()) => {
                app.current_project_path = Some(folder.clone());
                app.status_message =
                    format!("Saved Git-friendly .bookdir to '{}'", folder.display());
            }
            Err(err) => {
                app.status_message = format!("Failed to save .bookdir: {err}");
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        app.status_message =
            "In the web browser, use 'Save' or 'Save As .book' to download a single-file portable .book archive."
                .to_string();
    }
}

pub fn open_bookdir_dialog(app: &mut BookCreatorApp) {
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
        match BookDocument::load_book_directory(&folder) {
            Ok(doc) => {
                app.record_undo_snapshot();
                app.doc = doc;
                app.current_page = 0;
                app.selected_frame_id = None;
                app.editing_text_frame_id = None;
                app.current_project_path = Some(folder.clone());
                app.recompute_layout_and_preflight();
                app.status_message = format!("Opened .bookdir '{}'", folder.display());
            }
            Err(err) => {
                app.status_message = format!("Failed to open .bookdir: {err}");
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        app.status_message =
            "Tip (Web): Drag and drop a .book archive onto the browser window to open a project!"
                .to_string();
    }
}

pub fn export_pdf_dialog(app: &mut BookCreatorApp) {
    let options = PdfExportOptions {
        include_bleed: app.pdf_include_bleed,
        include_crop_marks: app.pdf_include_crop_marks,
    };
    let stem = app
        .save_as_filename
        .strip_suffix(".book")
        .unwrap_or(&app.save_as_filename);
    let default_pdf = if stem.trim().is_empty() {
        default_filename_for_title(&app.doc.metadata.title, "pdf")
    } else {
        ensure_extension(stem, "pdf")
    };

    match export_pdf_bytes(&app.doc, &app.layout, options) {
        Ok(bytes) => {
            match save_or_download_bytes(&default_pdf, "Print-Ready PDF (.pdf)", "pdf", &bytes) {
                Ok(dest) => {
                    app.status_message = format!(
                        "Exported Print PDF (bleed={}, crop_marks={}) to '{}'",
                        options.include_bleed, options.include_crop_marks, dest
                    );
                }
                Err(err) if err != "Cancelled" => {
                    app.status_message = format!("PDF save error: {err}");
                }
                _ => {}
            }
        }
        Err(err) => {
            app.status_message = format!("PDF export error: {err}");
        }
    }
}

pub fn export_epub_dialog(app: &mut BookCreatorApp) {
    let stem = app
        .save_as_filename
        .strip_suffix(".book")
        .unwrap_or(&app.save_as_filename);
    let default_epub = if stem.trim().is_empty() {
        default_filename_for_title(&app.doc.metadata.title, "epub")
    } else {
        ensure_extension(stem, "epub")
    };

    match export_epub_bytes(&app.doc, &app.layout, app.epub_layout_mode) {
        Ok(bytes) => {
            match save_or_download_bytes(&default_epub, "EPUB 3.3 eBook (.epub)", "epub", &bytes) {
                Ok(dest) => {
                    let mode_str = match app.epub_layout_mode {
                        EpubLayoutMode::FixedLayout => "Fixed-Layout",
                        EpubLayoutMode::Reflowable => "Reflowable",
                    };
                    app.status_message = format!("Exported {mode_str} EPUB 3.3 to '{dest}'");
                }
                Err(err) if err != "Cancelled" => {
                    app.status_message = format!("EPUB save error: {err}");
                }
                _ => {}
            }
        }
        Err(err) => {
            app.status_message = format!("EPUB export error: {err}");
        }
    }
}
