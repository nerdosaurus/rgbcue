//! Local-only artwork loader for the ported UI.
//!
//! The web UI ships product art in Qt `.rcc` files that belong to Corsair.
//! We never commit those; instead the app reads normalised PNGs from a local
//! asset root and falls back to the drawn icons when they are absent.
//!
//! Root resolution:
//!   `RGB_CUE_ASSETS` env var, else `~/assets/icue/normalized`.

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct ArtCache {
    textures: HashMap<String, Option<TextureHandle>>,
}

impl ArtCache {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
        }
    }

    /// Top-down line art for a Scimitar-class mouse, if present.
    pub fn mouse_top(&mut self, ctx: &egui::Context, product: &str) -> Option<TextureHandle> {
        self.mouse(ctx, product, "top.png")
    }

    /// Side line art for a Scimitar-class mouse, if present.
    pub fn mouse_side(&mut self, ctx: &egui::Context, product: &str) -> Option<TextureHandle> {
        self.mouse(ctx, product, "side.png")
    }

    fn mouse(
        &mut self,
        ctx: &egui::Context,
        product: &str,
        file: &str,
    ) -> Option<TextureHandle> {
        let name = product.to_uppercase();
        if !(name.contains("SCIMITAR") && name.contains("WIRELESS")) {
            return None;
        }
        self.load(ctx, &asset_root().join("scimitar_wireless").join(file))
    }

    fn load(&mut self, ctx: &egui::Context, path: &Path) -> Option<TextureHandle> {
        let key = path.to_string_lossy().to_string();
        if let Some(cached) = self.textures.get(&key) {
            return cached.clone();
        }
        let texture = load_texture(ctx, &key, path);
        self.textures.insert(key, texture.clone());
        texture
    }
}

pub fn asset_root() -> PathBuf {
    if let Ok(root) = std::env::var("RGB_CUE_ASSETS") {
        return PathBuf::from(root);
    }
    dirs::home_dir()
        .unwrap_or_default()
        .join("assets/icue/normalized")
}

fn load_texture(ctx: &egui::Context, key: &str, path: &Path) -> Option<TextureHandle> {
    let image = image::open(path).ok()?.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let pixels = ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    Some(ctx.load_texture(key.to_string(), pixels, TextureOptions::LINEAR))
}

/// Fit `size` inside `container` without distorting it.
pub fn fit_rect(container: egui::Rect, size: egui::Vec2) -> egui::Rect {
    if size.x <= 0.0 || size.y <= 0.0 {
        return container;
    }
    let scale = (container.width() / size.x)
        .min(container.height() / size.y)
        .max(0.0);
    egui::Rect::from_center_size(container.center(), size * scale)
}

/// Draw line art tinted with the current effect colour.
pub fn draw_line_art(
    painter: &egui::Painter,
    container: egui::Rect,
    texture: &TextureHandle,
    tint: eframe::egui::Color32,
) {
    let target = fit_rect(container, texture.size_vec2());
    painter.image(
        texture.id(),
        target,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        tint,
    );
}
