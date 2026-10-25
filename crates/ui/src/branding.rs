//! Brand artwork painted from pre-processed alpha masks.
//!
//! The supplied brand PNGs (white artwork on black, and the inverse) are
//! reduced to raw 8-bit coverage masks under `assets/branding/`: each file is
//! a little-endian `u32` width, a little-endian `u32` height, then one
//! coverage byte per pixel. A mask carries no colour, so the same texture is
//! tinted with the active palette token (`Palette::accent`, near-black in
//! light mode and paper white in dark mode) through mesh vertex colours.
//!
//! Keeping the artwork as coverage masks avoids a runtime image decoder: the
//! masks are embedded with `include_bytes!` and uploaded to the GPU once per
//! egui context, cached in context temp memory.

use egui::{Color32, ColorImage, Pos2, Rect, Sense, Shape, TextureHandle, TextureOptions, Ui, Vec2};

use crate::theme::Palette;

/// Coverage mask of the sunburst symbol alone, square (256 px).
const MARK_MASK: &[u8] = include_bytes!("../assets/branding/atlas-mark.mask");
/// Coverage mask of the full lockup (sunburst + the “Atlas” wordmark),
/// 768 × 248 px.
const WORDMARK_MASK: &[u8] = include_bytes!("../assets/branding/atlas-wordmark.mask");

const CACHE_ID: &str = "atlas-brand-textures";

/// GPU textures for the two brand masks, created once per egui context.
#[derive(Default, Clone)]
struct BrandTextures {
    mark: Option<TextureHandle>,
    wordmark: Option<TextureHandle>,
}

fn which(cache: &mut BrandTextures, wordmark: bool) -> &mut Option<TextureHandle> {
    if wordmark {
        &mut cache.wordmark
    } else {
        &mut cache.mark
    }
}

/// Decodes a coverage mask (`u32` width, `u32` height, then coverage bytes)
/// into a white RGBA image whose alpha channel is the coverage.
fn coverage_image(bytes: &[u8]) -> ColorImage {
    let width = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    let height = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    let mask = &bytes[8..];
    debug_assert_eq!(mask.len(), width * height, "brand mask size mismatch");
    let pixels = mask
        .iter()
        .map(|&coverage| Color32::from_rgba_unmultiplied(255, 255, 255, coverage))
        .collect();
    ColorImage::new([width, height], pixels)
}

/// Returns a cloned (cheap, `Arc`-backed) texture handle for a mask,
/// uploading it on first use.
fn texture(ui: &Ui, wordmark: bool) -> TextureHandle {
    let bytes = if wordmark { WORDMARK_MASK } else { MARK_MASK };
    let name = if wordmark { "atlas-wordmark" } else { "atlas-mark" };
    ui.ctx().data_mut(|data| {
        let cache = data.get_temp_mut_or_default::<BrandTextures>(egui::Id::new(CACHE_ID));
        let slot = which(cache, wordmark);
        if slot.is_none() {
            *slot = Some(ui.ctx().load_texture(
                name,
                coverage_image(bytes),
                TextureOptions::LINEAR,
            ));
        }
        slot.clone().expect("brand texture was just created")
    })
}

/// Paints a tinted texture over `rect`, mapping the whole texture onto it.
fn paint_tinted(ui: &Ui, rect: Rect, texture: &TextureHandle, tint: Color32) {
    let mut mesh = egui::epaint::Mesh::with_texture(texture.id());
    mesh.vertices = vec![
        egui::epaint::Vertex {
            pos: rect.left_top(),
            uv: Pos2::new(0.0, 0.0),
            color: tint,
        },
        egui::epaint::Vertex {
            pos: rect.right_top(),
            uv: Pos2::new(1.0, 0.0),
            color: tint,
        },
        egui::epaint::Vertex {
            pos: rect.left_bottom(),
            uv: Pos2::new(0.0, 1.0),
            color: tint,
        },
        egui::epaint::Vertex {
            pos: rect.right_bottom(),
            uv: Pos2::new(1.0, 1.0),
            color: tint,
        },
    ];
    mesh.indices = vec![0, 1, 2, 2, 1, 3];
    ui.painter().add(Shape::Mesh(mesh.into()));
}

/// The square sunburst symbol, `size` pixels wide, in the palette accent
/// (paper white in dark mode, near-black in light mode).
pub fn mark(ui: &mut Ui, palette: &Palette, size: f32) {
    let texture = texture(ui, false);
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    paint_tinted(ui, rect, &texture, palette.accent);
}

/// The full horizontal lockup (sunburst + “Atlas”) at a given height; the
/// width follows the mask aspect ratio. Tinted with the palette accent.
pub fn lockup(ui: &mut Ui, palette: &Palette, height: f32) {
    let texture = texture(ui, true);
    let [tex_w, tex_h] = texture.size();
    let width = height * tex_w as f32 / tex_h as f32;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    paint_tinted(ui, rect, &texture, palette.accent);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opaque_pixels(image: &ColorImage) -> usize {
        image.pixels.iter().filter(|pixel| pixel.a() > 200).count()
    }

    #[test]
    fn masks_decode_with_their_committed_geometry() {
        let mark = coverage_image(MARK_MASK);
        assert_eq!(mark.size, [256, 256]);
        let wordmark = coverage_image(WORDMARK_MASK);
        assert_eq!(wordmark.size, [768, 248]);
        // Both masks carry real artwork while keeping most pixels empty.
        assert!(opaque_pixels(&mark) > 1_000);
        assert!(opaque_pixels(&mark) < mark.size[0] * mark.size[1] / 2);
        assert!(opaque_pixels(&wordmark) > 1_000);
        // The sunburst is an open ring: its exact centre stays transparent.
        assert_eq!(mark.pixels[128 * 256 + 128].a(), 0);
        // Decoded pixels are white, tinted only at paint time.
        let sample = mark.pixels.iter().find(|pixel| pixel.a() > 200).unwrap();
        assert_eq!((sample.r(), sample.g(), sample.b()), (255, 255, 255));
    }
}
