//! Context-sensitive Right Inspector Panel:
//! - When a frame is selected: Frame Geometry (`X, Y, W, H pt`, Z-Index, Lock),
//!   Live Text Wrap (`None`, `BoundingBox`, `JumpObject` + Padding slider),
//!   and kind-specific controls (`Text` typography & story editor, `Image` asset import & fit,
//!   `Shape` vector styling).
//! - When no frame is selected: Book Metadata, Document Trim/Margins/Bleed setup,
//!   and Active Page background.

use book_core::{
    ColorRgba, FrameId, FrameKind, ImageFit, PagePreset, ShapeKind, TextAlignment, WrapMode,
};
use eframe::egui;

use crate::canvas::{from_color32, to_color32};
use crate::{BookCreatorApp, Tool};

pub fn show_right_inspector(app: &mut BookCreatorApp, ctx: &egui::Context) {
    egui::SidePanel::right("right_context_inspector")
        .resizable(true)
        .default_width(305.0)
        .min_width(265.0)
        .max_width(400.0)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("inspector_scroll")
                .show(ui, |ui| {
                    ui.add_space(4.0);
                    if let Some(sel_id) = app.selected_frame_id {
                        show_selected_frame_inspector(app, ui, sel_id);
                    } else {
                        show_document_and_page_inspector(app, ui);
                    }
                });
        });
}

fn show_selected_frame_inspector(app: &mut BookCreatorApp, ui: &mut egui::Ui, sel_id: FrameId) {
    let page_idx = app.current_page;
    let Some(frame_idx) = app
        .doc
        .pages
        .get(page_idx)
        .and_then(|p| p.frames.iter().position(|f| f.id == sel_id))
    else {
        app.selected_frame_id = None;
        return;
    };

    ui.horizontal(|ui| {
        ui.heading("Frame Inspector");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button(egui::RichText::new("🗑 Delete").color(egui::Color32::from_rgb(235, 87, 87)))
                .on_hover_text("Delete selected frame (Delete / Backspace)")
                .clicked()
            {
                app.delete_selected_frame();
            }
        });
    });

    if app.selected_frame_id.is_none() {
        return;
    }

    let mut bring_front = false;
    let mut send_back = false;
    let mut activate_link_tool = false;
    let mut import_image_clicked = false;
    let mut layout_dirty = false;

    // Clone story_id if this is a text frame so we can edit both Frame and Story cleanly
    let mut text_story_id = None;

    {
        let frame = &mut app.doc.pages[page_idx].frames[frame_idx];

        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Label:");
            ui.text_edit_singleline(&mut frame.label);
        });

        ui.horizontal(|ui| {
            ui.checkbox(&mut frame.locked, "🔒 Lock Position");
            ui.separator();
            ui.label(format!("Z: {}", frame.z_index));
            if ui
                .small_button("⬆ Front")
                .on_hover_text("Bring frame to front of page")
                .clicked()
            {
                bring_front = true;
            }
            if ui
                .small_button("⬇ Back")
                .on_hover_text("Send frame to back of page")
                .clicked()
            {
                send_back = true;
            }
        });

        ui.separator();
        ui.label(egui::RichText::new("Geometry (Typographic Points — pt)").strong());
        egui::Grid::new("frame_geom_grid")
            .num_columns(4)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                ui.label("X:");
                if ui
                    .add_enabled(
                        !frame.locked,
                        egui::DragValue::new(&mut frame.rect.x)
                            .speed(1.0)
                            .suffix(" pt"),
                    )
                    .changed()
                {
                    layout_dirty = true;
                }
                ui.label("Y:");
                if ui
                    .add_enabled(
                        !frame.locked,
                        egui::DragValue::new(&mut frame.rect.y)
                            .speed(1.0)
                            .suffix(" pt"),
                    )
                    .changed()
                {
                    layout_dirty = true;
                }
                ui.end_row();

                ui.label("W:");
                if ui
                    .add_enabled(
                        !frame.locked,
                        egui::DragValue::new(&mut frame.rect.width)
                            .speed(1.0)
                            .range(16.0..=2000.0)
                            .suffix(" pt"),
                    )
                    .changed()
                {
                    layout_dirty = true;
                }
                ui.label("H:");
                if ui
                    .add_enabled(
                        !frame.locked,
                        egui::DragValue::new(&mut frame.rect.height)
                            .speed(1.0)
                            .range(16.0..=2000.0)
                            .suffix(" pt"),
                    )
                    .changed()
                {
                    layout_dirty = true;
                }
                ui.end_row();
            });

        // --- Live Text Wrap ("Runaround") Section ---
        ui.separator();
        ui.label(egui::RichText::new("Text Wrap (Live Runaround)").strong());
        ui.label(
            egui::RichText::new("Control how neighboring stories wrap around this frame:")
                .small()
                .weak(),
        );

        ui.horizontal(|ui| {
            if ui
                .selectable_value(&mut frame.wrap.mode, WrapMode::None, "None")
                .changed()
            {
                layout_dirty = true;
            }
            if ui
                .selectable_value(&mut frame.wrap.mode, WrapMode::BoundingBox, "Bounding Box")
                .on_hover_text("Split neighboring text lines on left/right sides of this object")
                .changed()
            {
                layout_dirty = true;
            }
            if ui
                .selectable_value(&mut frame.wrap.mode, WrapMode::JumpObject, "Jump Object")
                .on_hover_text("Stop text above this object and resume below it")
                .changed()
            {
                layout_dirty = true;
            }
        });

        if frame.wrap.mode != WrapMode::None {
            ui.horizontal(|ui| {
                ui.label("Wrap Padding:");
                if ui
                    .add(
                        egui::Slider::new(&mut frame.wrap.padding_pt, 0.0..=48.0)
                            .suffix(" pt"),
                    )
                    .changed()
                {
                    layout_dirty = true;
                }
            });
        }

        ui.separator();

        // --- Kind-Specific Controls ---
        match &mut frame.kind {
            FrameKind::Text {
                story_id,
                thread_order,
                padding_pt,
                background,
            } => {
                text_story_id = Some((*story_id, *thread_order));
                ui.label(egui::RichText::new("Text Frame & Story").strong());
                ui.horizontal(|ui| {
                    ui.label("Inset Padding:");
                    if ui
                        .add(egui::Slider::new(padding_pt, 0.0..=36.0).suffix(" pt"))
                        .changed()
                    {
                        layout_dirty = true;
                    }
                });

                let mut has_bg = background.is_some();
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut has_bg, "Fill Background").changed() {
                        if has_bg {
                            *background = Some(ColorRgba::rgb(245, 247, 250));
                        } else {
                            *background = None;
                        }
                    }
                    if let Some(bg_col) = background {
                        let mut c32 = to_color32(*bg_col);
                        if ui.color_edit_button_srgba(&mut c32).changed() {
                            *bg_col = from_color32(c32);
                        }
                    }
                });
            }
            FrameKind::Image {
                asset_id,
                fit,
                caption,
                corner_radius_pt,
            } => {
                ui.label(egui::RichText::new("Image Frame Properties").strong());
                if ui
                    .button("📂 Import Image File (PNG / JPG / WEBP / SVG)...")
                    .clicked()
                {
                    import_image_clicked = true;
                }

                if let Some(id) = asset_id {
                    ui.label(
                        egui::RichText::new(format!("Bundle Asset ID: {id}"))
                            .small()
                            .color(egui::Color32::from_rgb(120, 200, 150)),
                    );
                } else {
                    ui.label(
                        egui::RichText::new("No image imported yet (drop a file onto canvas or click Import above).")
                            .small()
                            .weak(),
                    );
                }

                ui.horizontal(|ui| {
                    ui.label("Image Fit:");
                    ui.selectable_value(fit, ImageFit::Cover, "Cover");
                    ui.selectable_value(fit, ImageFit::Contain, "Contain");
                    ui.selectable_value(fit, ImageFit::Stretch, "Stretch");
                });

                ui.horizontal(|ui| {
                    ui.label("Caption:");
                    ui.text_edit_singleline(caption);
                });

                ui.horizontal(|ui| {
                    ui.label("Corner Radius:");
                    ui.add(egui::Slider::new(corner_radius_pt, 0.0..=48.0).suffix(" pt"));
                });
            }
            FrameKind::Shape {
                kind,
                fill,
                stroke_color,
                stroke_width_pt,
                corner_radius_pt,
            } => {
                ui.label(egui::RichText::new("Vector Shape Style").strong());
                ui.horizontal(|ui| {
                    ui.label("Shape:");
                    egui::ComboBox::from_id_salt("inspector_shape_kind")
                        .selected_text(match kind {
                            ShapeKind::Rectangle => "Rectangle",
                            ShapeKind::RoundedRectangle => "Rounded Rectangle",
                            ShapeKind::Ellipse => "Ellipse",
                            ShapeKind::DividerLine => "Divider Line",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(kind, ShapeKind::Rectangle, "Rectangle");
                            ui.selectable_value(
                                kind,
                                ShapeKind::RoundedRectangle,
                                "Rounded Rectangle",
                            );
                            ui.selectable_value(kind, ShapeKind::Ellipse, "Ellipse");
                            ui.selectable_value(kind, ShapeKind::DividerLine, "Divider Line");
                        });
                });

                ui.horizontal(|ui| {
                    ui.label("Fill Color:");
                    let mut fill_c32 = to_color32(*fill);
                    if ui.color_edit_button_srgba(&mut fill_c32).changed() {
                        *fill = from_color32(fill_c32);
                    }
                    ui.separator();
                    ui.label("Stroke:");
                    let mut stroke_c32 = to_color32(*stroke_color);
                    if ui.color_edit_button_srgba(&mut stroke_c32).changed() {
                        *stroke_color = from_color32(stroke_c32);
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Stroke Width:");
                    ui.add(egui::Slider::new(stroke_width_pt, 0.0..=16.0).suffix(" pt"));
                });

                if *kind == ShapeKind::RoundedRectangle {
                    ui.horizontal(|ui| {
                        ui.label("Corner Radius:");
                        ui.add(egui::Slider::new(corner_radius_pt, 0.0..=64.0).suffix(" pt"));
                    });
                }
            }
        }
    }

    // If this is a Text Frame, show Typography & Story Content controls
    if let Some((story_id, thread_order)) = text_story_id {
        let chain_len = app.doc.threaded_frames_for_story(story_id).len();
        let is_overflowing = app
            .layout
            .pages
            .get(page_idx)
            .and_then(|p| p.text_frames.get(&sel_id))
            .map_or(false, |tf| tf.is_overflowing);

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "Thread Frame #{} of {}",
                    thread_order + 1,
                    chain_len.max(1)
                ))
                .small()
                .strong(),
            );
            if ui
                .small_button("🔗 Thread into Frame (L)")
                .on_hover_text("Link this text frame to another text frame so overflow continues")
                .clicked()
            {
                activate_link_tool = true;
            }
        });

        if is_overflowing {
            ui.colored_label(
                egui::Color32::from_rgb(248, 113, 113),
                "⚠ Story overflows this frame [+]. Enlarge box or click 'Thread into Frame (L)'.",
            );
        }

        if let Some(story) = app.doc.stories.get_mut(&story_id) {
            ui.separator();
            ui.label(egui::RichText::new("Typography").strong());

            ui.horizontal(|ui| {
                ui.label("Font Family:");
                let families = [
                    "Literata",
                    "Inter",
                    "JetBrains Mono",
                    "Helvetica",
                    "Times",
                ];
                egui::ComboBox::from_id_salt("inspector_font_family")
                    .selected_text(&story.style.font_family)
                    .show_ui(ui, |ui| {
                        for fam in families {
                            if ui
                                .selectable_label(story.style.font_family == fam, fam)
                                .clicked()
                            {
                                story.style.font_family = fam.to_string();
                                layout_dirty = true;
                            }
                        }
                    });
            });

            ui.horizontal(|ui| {
                ui.label("Size:");
                if ui
                    .add(
                        egui::DragValue::new(&mut story.style.font_size_pt)
                            .speed(0.5)
                            .range(6.0..=96.0)
                            .suffix(" pt"),
                    )
                    .changed()
                {
                    layout_dirty = true;
                }
                ui.label("Line H:");
                if ui
                    .add(
                        egui::DragValue::new(&mut story.style.line_height_factor)
                            .speed(0.02)
                            .range(1.0..=2.5)
                            . suffix("×"),
                    )
                    .changed()
                {
                    layout_dirty = true;
                }
            });

            ui.horizontal(|ui| {
                if ui.checkbox(&mut story.style.bold, "Bold").changed() {
                    layout_dirty = true;
                }
                if ui.checkbox(&mut story.style.italic, "Italic").changed() {
                    layout_dirty = true;
                }
                ui.separator();
                ui.label("Color:");
                let mut tc = to_color32(story.style.color);
                if ui.color_edit_button_srgba(&mut tc).changed() {
                    story.style.color = from_color32(tc);
                    layout_dirty = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Align:");
                for (align, label) in [
                    (TextAlignment::Left, "Left"),
                    (TextAlignment::Center, "Center"),
                    (TextAlignment::Right, "Right"),
                    (TextAlignment::Justified, "Justify"),
                ] {
                    if ui
                        .selectable_value(&mut story.style.alignment, align, label)
                        .changed()
                    {
                        layout_dirty = true;
                    }
                }
            });

            ui.separator();
            ui.label(egui::RichText::new("Story Content").strong());
            ui.horizontal(|ui| {
                ui.label("Title:");
                ui.text_edit_singleline(&mut story.title);
            });
            if ui
                .add_sized(
                    egui::vec2(ui.available_width(), 165.0),
                    egui::TextEdit::multiline(&mut story.text),
                )
                .changed()
            {
                layout_dirty = true;
            }
        }
    }

    if bring_front {
        app.bring_selected_to_front();
    }
    if send_back {
        app.send_selected_to_back();
    }
    if activate_link_tool {
        app.active_tool = Tool::LinkText;
        app.link_source_frame_id = Some(sel_id);
        app.status_message =
            "Link Mode: Click another Text Frame to thread overflow story.".to_string();
    }
    if import_image_clicked {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg", "webp", "svg"])
            .pick_file()
        {
            if let Ok(bytes) = std::fs::read(&path) {
                let filename = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("image.png")
                    .to_string();
                app.import_image_bytes_into_doc(&filename, bytes, Some(sel_id), None);
            }
        }
    }
    if layout_dirty {
        app.recompute_layout_and_preflight();
    }
}

fn show_document_and_page_inspector(app: &mut BookCreatorApp, ui: &mut egui::Ui) {
    ui.heading("Document & Page Setup");
    ui.label(
        egui::RichText::new("Click any frame on the canvas to inspect its geometry, text wrap, or typography.")
            .small()
            .weak(),
    );

    ui.separator();
    ui.label(egui::RichText::new("Book Metadata (PDF & EPUB 3)").strong());
    egui::Grid::new("book_metadata_grid")
        .num_columns(2)
        .spacing([8.0, 6.0])
        .show(ui, |ui| {
            ui.label("Title:");
            ui.text_edit_singleline(&mut app.doc.metadata.title);
            ui.end_row();

            ui.label("Subtitle:");
            ui.text_edit_singleline(&mut app.doc.metadata.subtitle);
            ui.end_row();

            ui.label("Author:");
            ui.text_edit_singleline(&mut app.doc.metadata.author);
            ui.end_row();

            ui.label("Publisher:");
            ui.text_edit_singleline(&mut app.doc.metadata.publisher);
            ui.end_row();

            ui.label("ISBN / UUID:");
            ui.text_edit_singleline(&mut app.doc.metadata.isbn);
            ui.end_row();

            ui.label("Language:");
            ui.text_edit_singleline(&mut app.doc.metadata.language);
            ui.end_row();
        });

    ui.separator();
    ui.label(egui::RichText::new("Trim Size, Margins & Print Bleed").strong());

    let mut layout_dirty = false;
    let current_preset = app.doc.setup.preset;
    let mut new_preset = None;
    egui::ComboBox::from_id_salt("inspector_page_preset")
        .selected_text(current_preset.label())
        .show_ui(ui, |ui| {
            for p in [
                PagePreset::UsTrade6x9,
                PagePreset::UsLetter,
                PagePreset::Square85x85,
                PagePreset::Digest55x85,
                PagePreset::IsoA4,
                PagePreset::IsoA5,
                PagePreset::Custom,
            ] {
                if ui.selectable_label(current_preset == p, p.label()).clicked() {
                    new_preset = Some(p);
                }
            }
        });

    if let Some(p) = new_preset {
        app.record_undo_snapshot();
        app.doc.setup.preset = p;
        if p != PagePreset::Custom {
            let (w, h) = p.dimensions_pt();
            app.doc.setup.width_pt = w;
            app.doc.setup.height_pt = h;
        }
        layout_dirty = true;
    }

    ui.horizontal(|ui| {
        ui.label("Width:");
        if ui
            .add(
                egui::DragValue::new(&mut app.doc.setup.width_pt)
                    .speed(1.0)
                    .range(144.0..=2000.0)
                    .suffix(" pt"),
            )
            .changed()
        {
            app.doc.setup.preset = PagePreset::Custom;
            layout_dirty = true;
        }
        ui.label("Height:");
        if ui
            .add(
                egui::DragValue::new(&mut app.doc.setup.height_pt)
                    .speed(1.0)
                    .range(144.0..=2000.0)
                    .suffix(" pt"),
            )
            .changed()
        {
            app.doc.setup.preset = PagePreset::Custom;
            layout_dirty = true;
        }
    });

    ui.horizontal(|ui| {
        ui.label("Print Bleed:");
        ui.add(
            egui::DragValue::new(&mut app.doc.setup.bleed_pt)
                .speed(0.5)
                .range(0.0..=36.0)
                .suffix(" pt"),
        );
        ui.label(
            egui::RichText::new(format!("({:.3} in)", app.doc.setup.bleed_pt / 72.0))
                .small()
                .weak(),
        );
    });

    egui::Grid::new("margins_grid")
        .num_columns(4)
        .spacing([6.0, 6.0])
        .show(ui, |ui| {
            ui.label("Top:");
            ui.add(
                egui::DragValue::new(&mut app.doc.setup.margins.top)
                    .speed(1.0)
                    .range(0.0..=144.0)
                    .suffix(" pt"),
            );
            ui.label("Bottom:");
            ui.add(
                egui::DragValue::new(&mut app.doc.setup.margins.bottom)
                    .speed(1.0)
                    .range(0.0..=144.0)
                    .suffix(" pt"),
            );
            ui.end_row();

            ui.label("Inside:");
            ui.add(
                egui::DragValue::new(&mut app.doc.setup.margins.inside)
                    .speed(1.0)
                    .range(0.0..=144.0)
                    .suffix(" pt"),
            );
            ui.label("Outside:");
            ui.add(
                egui::DragValue::new(&mut app.doc.setup.margins.outside)
                    .speed(1.0)
                    .range(0.0..=144.0)
                    .suffix(" pt"),
            );
            ui.end_row();
        });

    if let Some(page) = app.doc.pages.get_mut(app.current_page) {
        ui.separator();
        ui.label(egui::RichText::new("Active Page Settings").strong());
        ui.horizontal(|ui| {
            ui.label("Page Title:");
            ui.text_edit_singleline(&mut page.title);
        });
        ui.horizontal(|ui| {
            ui.label("Paper Color:");
            let mut bg = to_color32(page.background_color);
            if ui.color_edit_button_srgba(&mut bg).changed() {
                page.background_color = from_color32(bg);
            }
        });
    }

    if layout_dirty {
        app.recompute_layout_and_preflight();
    }
}
