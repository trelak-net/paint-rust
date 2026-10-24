use eframe::egui::{self, Button, Image, ImageSource};

#[derive(Clone, Copy)]
pub enum Icon {
    New,
    Open,
    Save,
    SaveAs,
    Undo,
    Redo,
    Invert,
    Line,
    Rectangle,
    Ellipse,
    Brush,
    Eraser,
    Fill,
    Picker,
    Fit,
    ActualSize,
}

impl Icon {
    fn source(self) -> ImageSource<'static> {
        match self {
            Self::New => egui::include_image!("../assets/icons/new.svg"),
            Self::Open => egui::include_image!("../assets/icons/open.svg"),
            Self::Save => egui::include_image!("../assets/icons/save.svg"),
            Self::SaveAs => egui::include_image!("../assets/icons/save-as.svg"),
            Self::Undo => egui::include_image!("../assets/icons/undo.svg"),
            Self::Redo => egui::include_image!("../assets/icons/redo.svg"),
            Self::Invert => egui::include_image!("../assets/icons/invert.svg"),
            Self::Line => egui::include_image!("../assets/icons/line.svg"),
            Self::Rectangle => egui::include_image!("../assets/icons/rectangle.svg"),
            Self::Ellipse => egui::include_image!("../assets/icons/ellipse.svg"),
            Self::Brush => egui::include_image!("../assets/icons/brush.svg"),
            Self::Eraser => egui::include_image!("../assets/icons/eraser.svg"),
            Self::Fill => egui::include_image!("../assets/icons/fill.svg"),
            Self::Picker => egui::include_image!("../assets/icons/picker.svg"),
            Self::Fit => egui::include_image!("../assets/icons/fit.svg"),
            Self::ActualSize => egui::include_image!("../assets/icons/actual-size.svg"),
        }
    }

    pub fn button(self, text: &str) -> Button<'_> {
        Button::image_and_text(
            Image::new(self.source()).fit_to_exact_size(egui::vec2(20.0, 20.0)),
            text,
        )
        .image_tint_follows_text_color(true)
    }
}
