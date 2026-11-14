//! Built-in Searchable Help Manual (`F1`), OS-Aware Shortcut Reference (`Cmd ⌘` on
//! Mac M1 Air vs `Ctrl` on Arch Linux), 1-click 4-Page Interactive Tutorial Book loader,
//! and Live Preflight Verifier Drawer with 1-click "Jump to Page & Select Frame" actions.

use book_core::PreflightSeverity;
use book_help::{builtin_help_articles, primary_modifier_label, search_help_articles};
use eframe::egui;

use crate::BookCreatorApp;

/// Show the Searchable Out-of-the-Box Help Manual modal/window (`F1`).
pub fn show_help_manual_window(app: &mut BookCreatorApp, ctx: &egui::Context) {
    if !app.show_help {
        return;
    }

    let mut is_open = app.show_help;
    let mut trigger_tutorial_reload = false;

    egui::Window::new("📚 Book Creator — Built-In Help Manual & Quick Recipes (F1)")
        .open(&mut is_open)
        .default_size([740.0, 580.0])
        .min_size([520.0, 420.0])
        .resizable(true)
        .show(ctx, |ui| {
            // Top bar: 1-Click Starter Tutorial Book button + Search Box
            egui::Frame::NONE
                .fill(egui::Color32::from_rgb(30, 41, 59))
                .corner_radius(6.0)
                .inner_margin(10.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new("Interactive 4-Page Starter Tutorial Book")
                                    .strong()
                                    .color(egui::Color32::WHITE),
                            );
                            ui.label(
                                egui::RichText::new(
                                    "Practice dragging wrapped shapes through text, threading stories across pages, and exporting PDF & EPUB 3.",
                                )
                                .small()
                                .color(egui::Color32::from_rgb(190, 205, 228)),
                            );
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let btn = egui::Button::new(
                                egui::RichText::new("🎓 Reset / Open 4-Page Interactive Tutorial Book")
                                    .strong()
                                    .color(egui::Color32::WHITE),
                            )
                            .fill(egui::Color32::from_rgb(37, 99, 235));
                            if ui.add(btn).clicked() {
                                trigger_tutorial_reload = true;
                            }
                        });
                    });
                });

            ui.add_space(6.0);

            // Instant Search Input
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("🔍 Search Manual:").strong());
                ui.add_sized(
                    [340.0, 24.0],
                    egui::TextEdit::singleline(&mut app.help_search_query)
                        .hint_text("Type e.g. 'wrap', 'thread', 'bleed', 'epub', 'mac m1', 'arch'..."),
                );
                if !app.help_search_query.is_empty() && ui.small_button("✖ Clear").clicked() {
                    app.help_search_query.clear();
                }
            });

            // Category Filter Tabs
            let all_articles = builtin_help_articles();
            let mut categories: Vec<&'static str> = Vec::new();
            for art in &all_articles {
                if !categories.contains(&art.category) {
                    categories.push(art.category);
                }
            }

            ui.horizontal_wrapped(|ui| {
                if ui
                    .selectable_label(app.help_selected_category.is_none(), "All Topics")
                    .clicked()
                {
                    app.help_selected_category = None;
                }
                for cat in categories {
                    let selected = app.help_selected_category.as_deref() == Some(cat);
                    if ui.selectable_label(selected, cat).clicked() {
                        app.help_selected_category = Some(cat.to_string());
                    }
                }
            });

            ui.separator();

            egui::ScrollArea::vertical()
                .id_salt("help_manual_scroll")
                .show(ui, |ui| {
                    let filtered = search_help_articles(&app.help_search_query);
                    let matching: Vec<_> = filtered
                        .into_iter()
                        .filter(|a| {
                            app.help_selected_category
                                .as_deref()
                                .map_or(true, |c| a.category == c)
                        })
                        .collect();

                    if matching.is_empty() {
                        ui.label(
                            egui::RichText::new(
                                "No help articles matched your search query. Try clearing the search box above.",
                            )
                            .italics(),
                        );
                    }

                    for article in matching {
                        egui::Frame::NONE
                            .fill(egui::Color32::from_rgb(32, 35, 44))
                            .stroke(egui::Stroke::new(
                                1.0_f32,
                                egui::Color32::from_rgb(58, 65, 80),
                            ))
                            .corner_radius(6.0)
                            .inner_margin(10.0)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(article.title)
                                            .strong()
                                            .size(15.0)
                                            .color(egui::Color32::from_rgb(147, 197, 253)),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.label(
                                                egui::RichText::new(article.category)
                                                    .small()
                                                    .color(egui::Color32::from_rgb(165, 180, 205)),
                                            );
                                        },
                                    );
                                });
                                ui.label(egui::RichText::new(article.summary).italics());
                                ui.add_space(4.0);
                                for (step_idx, step) in article.steps.iter().enumerate() {
                                    ui.horizontal_top(|ui| {
                                        ui.label(
                                            egui::RichText::new(format!("{}.", step_idx + 1))
                                                .strong()
                                                .color(egui::Color32::from_rgb(96, 165, 250)),
                                        );
                                        ui.label(*step);
                                    });
                                }
                            });
                        ui.add_space(8.0);
                    }

                    ui.separator();

                    // OS-Aware Keyboard Shortcuts Reference Table
                    let mod_label = primary_modifier_label();
                    ui.heading(format!(
                        "⌨ Keyboard Shortcuts Reference (Active OS Modifier: {mod_label})"
                    ));
                    ui.label(
                        egui::RichText::new(
                            "Automatically adapts between Cmd (⌘) on Mac M1 Air and Ctrl on Arch Linux.",
                        )
                        .small()
                        .weak(),
                    );
                    ui.add_space(4.0);

                    egui::Grid::new("help_shortcuts_grid")
                        .striped(true)
                        .num_columns(2)
                        .spacing([24.0, 6.0])
                        .show(ui, |ui| {
                            let shortcuts = [
                                ("V", "Select / Move / Resize Tool"),
                                ("T", "Draw New Story Text Frame Tool"),
                                ("I", "Draw New Image Frame Tool"),
                                ("S", "Draw New Vector Shape Frame Tool"),
                                ("L", "Link / Thread Text Frames Across Pages"),
                                ("Double-Click Text Frame", "Direct On-Canvas Story Text Editor"),
                                (&format!("{mod_label} + Z"), "Undo Last Edit / Drag"),
                                (
                                    &format!("{mod_label} + Y / {mod_label} + Shift + Z"),
                                    "Redo Edit",
                                ),
                                (&format!("{mod_label} + Mouse Wheel"), "Zoom Canvas In / Out"),
                                ("Space + Drag / Middle-Drag", "Pan Spread Canvas"),
                                ("Delete / Backspace", "Delete Selected Frame"),
                                ("F1", "Toggle Searchable Help Manual"),
                            ];
                            for (key, desc) in shortcuts {
                                ui.label(egui::RichText::new(key).monospace().strong());
                                ui.label(desc);
                                ui.end_row();
                            }
                        });
                });
        });

    app.show_help = is_open;
    if trigger_tutorial_reload {
        app.load_tutorial_book();
        app.show_help = false;
    }
}

/// Show the Live Preflight Verifier window/drawer with 1-click "Jump to Page & Select Frame" buttons.
pub fn show_preflight_window(app: &mut BookCreatorApp, ctx: &egui::Context) {
    if !app.show_preflight_drawer {
        return;
    }

    let mut is_open = app.show_preflight_drawer;
    let mut jump_target: Option<(Option<usize>, Option<book_core::FrameId>)> = None;

    egui::Window::new("🔍 Live Preflight Verifier (Print PDF & EPUB 3 Readiness)")
        .open(&mut is_open)
        .default_size([520.0, 360.0])
        .resizable(true)
        .show(ctx, |ui| {
            if app.preflight_issues.is_empty() {
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgb(20, 65, 40))
                    .corner_radius(6.0)
                    .inner_margin(12.0)
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("✓ 0 Preflight Issues Detected")
                                .strong()
                                .size(15.0)
                                .color(egui::Color32::from_rgb(134, 239, 172)),
                        );
                        ui.label(
                            "All text stories fit inside their frames and all bundle assets are intact. Ready for Print PDF and EPUB 3 export!",
                        );
                    });
                return;
            }

            ui.label(
                egui::RichText::new(format!(
                    "{} issue(s) found. Click 'Jump to Page & Select Frame' to inspect and fix:",
                    app.preflight_issues.len()
                ))
                .strong(),
            );
            ui.separator();

            egui::ScrollArea::vertical()
                .id_salt("preflight_issues_scroll")
                .show(ui, |ui| {
                    for issue in &app.preflight_issues {
                        let (badge_text, badge_color) = match issue.severity {
                            PreflightSeverity::Error => {
                                ("ERROR", egui::Color32::from_rgb(239, 68, 68))
                            }
                            PreflightSeverity::Warning => {
                                ("WARNING", egui::Color32::from_rgb(245, 158, 11))
                            }
                            PreflightSeverity::Info => {
                                ("INFO", egui::Color32::from_rgb(59, 130, 246))
                            }
                        };

                        egui::Frame::NONE
                            .fill(egui::Color32::from_rgb(35, 38, 48))
                            .stroke(egui::Stroke::new(1.0_f32, badge_color))
                            .corner_radius(6.0)
                            .inner_margin(10.0)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.colored_label(badge_color, format!("[{badge_text}]"));
                                    ui.label(egui::RichText::new(&issue.title).strong());
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if issue.page_index.is_some()
                                                || issue.frame_id.is_some()
                                            {
                                                if ui
                                                    .button("↪ Jump to Page & Select Frame")
                                                    .clicked()
                                                {
                                                    jump_target =
                                                        Some((issue.page_index, issue.frame_id));
                                                }
                                            }
                                        },
                                    );
                                });
                                ui.label(&issue.explanation);
                                ui.label(
                                    egui::RichText::new(format!("How to fix: {}", issue.how_to_fix))
                                        .small()
                                        .color(egui::Color32::from_rgb(147, 197, 253)),
                                );
                            });
                        ui.add_space(6.0);
                    }
                });
        });

    app.show_preflight_drawer = is_open;
    if let Some((page_idx, frame_id)) = jump_target {
        app.jump_to_preflight_issue(page_idx, frame_id);
    }
}
