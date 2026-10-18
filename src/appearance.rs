use eframe::egui::{self, ColorImage, IconData, TextureHandle, TextureOptions};

pub fn window_icon() -> IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/app-icon-window.png"))
        .expect("The embedded app icon must be a valid PNG")
}

pub fn load_mark(context: &egui::Context) -> TextureHandle {
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/logo-mark-ui.png"))
        .expect("The embedded logo mark must be a valid PNG");
    context.load_texture(
        "brand-mark",
        ColorImage::from_rgba_unmultiplied([icon.width as usize, icon.height as usize], &icon.rgba),
        TextureOptions::LINEAR,
    )
}
