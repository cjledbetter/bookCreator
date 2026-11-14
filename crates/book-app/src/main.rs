//! Desktop application entry point (`book-creator`) built with `eframe` / `egui`.

use book_core::BookDocument;
use book_help::create_tutorial_book;
use book_layout::ComputedBookLayout;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Book Creator — Visual Print PDF & EPUB 3 Studio")
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([960.0, 640.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Book Creator",
        native_options,
        Box::new(|_cc| Ok(Box::new(BookCreatorApp::new()))),
    )
}

pub struct BookCreatorApp {
    pub doc: BookDocument,
    pub layout: ComputedBookLayout,
    pub current_page: usize,
    pub show_help: bool,
}

impl BookCreatorApp {
    pub fn new() -> Self {
        let doc = create_tutorial_book();
        let layout = ComputedBookLayout::compute(&doc);
        Self {
            doc,
            layout,
            current_page: 0,
            show_help: false,
        }
    }
}

impl eframe::App for BookCreatorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.key_pressed(egui::Key::F1)) {
            self.show_help = !self.show_help;
        }

        egui::TopBottomPanel::top("top_toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading(&self.doc.metadata.title);
                ui.separator();
                if ui.button("? Help Manual (F1)").clicked() {
                    self.show_help = !self.show_help;
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label(format!(
                "Loaded {} pages. Ready for Phase 2 /swarm implementation.",
                self.doc.pages.len()
            ));
        });
    }
}
