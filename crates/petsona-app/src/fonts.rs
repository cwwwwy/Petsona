//! CJK font fallback.
//!
//! egui's bundled fonts cover Latin and Cyrillic, but not Chinese. Add the
//! platform's best available CJK font as a fallback for both proportional and
//! monospace text.

use std::sync::Arc;

pub fn install_cjk_font(ctx: &egui::Context, candidates: &[std::path::PathBuf]) {
    let Some((bytes, index)) = load_cjk_font(candidates) else {
        tracing::warn!("no CJK font found; Chinese text may render as boxes");
        return;
    };

    let mut fonts = egui::FontDefinitions::default();
    let mut data = egui::FontData::from_owned(bytes);
    data.index = index;
    fonts.font_data.insert("cjk".to_string(), Arc::new(data));

    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .push("cjk".to_string());
    }
    ctx.set_fonts(fonts);
}

fn load_cjk_font(candidates: &[std::path::PathBuf]) -> Option<(Vec<u8>, u32)> {
    if let Some(path) = std::env::var_os("PETSONA_FONT") {
        if let Some(font) = read_font(std::path::PathBuf::from(path)) {
            return Some((font, 0));
        }
    }

    for path in candidates {
        if let Some(font) = read_font(path) {
            tracing::info!(path = %path.display(), "loaded CJK font");
            return Some((font, 0));
        }
    }

    None
}

fn read_font(path: impl AsRef<std::path::Path>) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}
