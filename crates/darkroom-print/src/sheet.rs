//! The signature sheet, and every other sheet layout: a photo placed on a page with margins,
//! optionally a signature strip under it, composed at the print's pixel density in a working RGB
//! space, then converted into the paper's profile ([`crate::to_profile_from`]).
//!
//! The signature sheet is Randy's Photoshop layout (JSX v1.4): 13x19, 0.25 inch margins, a
//! 0.4 inch strip under the image with "Randy Archambault · The Infinite Arch" in a light face at
//! 7 pt, charcoal, 300 ppi. The numbers are the template's defaults; everything is a parameter.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use serde::{Deserialize, Serialize};

use crate::Image16;

/// A page layout in inches and points.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Layout {
    pub width_in: f32,
    pub height_in: f32,
    pub margin_in: f32,
    /// Height of the strip under the image; 0 = none.
    pub strip_in: f32,
    /// Text in the strip (empty = none).
    pub caption: String,
    /// Caption size in points (1/72 inch).
    pub caption_pt: f32,
    /// Caption color, sRGB 0..255.
    pub caption_rgb: [u8; 3],
    /// left | center | right
    pub caption_align: String,
    pub ppi: u32,
    /// Turn the page so a landscape photo gets the long side (the printer gets the file as is).
    pub auto_rotate: bool,
}

impl Default for Layout {
    fn default() -> Self {
        Self::signature()
    }
}

impl Layout {
    /// The Infinite Arch signature sheet.
    pub fn signature() -> Self {
        Self {
            width_in: 13.0,
            height_in: 19.0,
            margin_in: 0.25,
            strip_in: 0.4,
            caption: "Randy Archambault · The Infinite Arch".into(),
            caption_pt: 7.0,
            caption_rgb: [0x33, 0x33, 0x33],
            caption_align: "left".into(),
            ppi: 300,
            auto_rotate: true,
        }
    }

    /// A plain sheet with even margins and no strip.
    pub fn plain(width_in: f32, height_in: f32, margin_in: f32, ppi: u32) -> Self {
        Self { width_in, height_in, margin_in, strip_in: 0.0, caption: String::new(), ppi, ..Self::signature() }
    }

    fn px(&self, inches: f32) -> u32 {
        (inches * self.ppi as f32).round().max(0.0) as u32
    }

    /// Page size in pixels, turned for the photo when `auto_rotate` says so.
    pub fn page_px(&self, photo_w: u32, photo_h: u32) -> (u32, u32) {
        let (w, h) = (self.px(self.width_in), self.px(self.height_in));
        let landscape_photo = photo_w > photo_h;
        if self.auto_rotate && landscape_photo != (w > h) { (h, w) } else { (w, h) }
    }

    /// Where the photo goes on the page: `(x, y, w, h)` in pixels, fitted inside the margins and
    /// above the strip, centered. Also the page size it was fitted for.
    pub fn place(&self, photo_w: u32, photo_h: u32) -> Placement {
        let (pw, ph) = self.page_px(photo_w, photo_h);
        let m = self.px(self.margin_in);
        let strip = self.px(self.strip_in);
        let area_w = pw.saturating_sub(2 * m).max(1) as f64;
        let area_h = ph.saturating_sub(2 * m + strip).max(1) as f64;
        let s = (area_w / photo_w.max(1) as f64).min(area_h / photo_h.max(1) as f64);
        let w = ((photo_w as f64 * s).round() as u32).max(1);
        let h = ((photo_h as f64 * s).round() as u32).max(1);
        let x = m + ((area_w as u32).saturating_sub(w)) / 2;
        let y = m + ((area_h as u32).saturating_sub(h)) / 2;
        Placement { page_w: pw, page_h: ph, x, y, w, h, strip_top: y + h, strip_h: strip, margin: m }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub page_w: u32,
    pub page_h: u32,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub strip_top: u32,
    pub strip_h: u32,
    pub margin: u32,
}

/// Puts `photo` (already sized to about `placement.w` x `placement.h`; a few pixels off are
/// clipped or left white) on a white page and draws the caption in the strip. Pixel values are
/// copied as they are: the photo and the page share one working space.
pub fn compose(layout: &Layout, photo: &Image16, font: Option<&[u8]>) -> Image16 {
    let p = layout.place(photo.width, photo.height);
    let mut page = Image16::new(p.page_w, p.page_h);
    // the photo, centered in its slot when the sizes differ slightly
    let dx = p.x as i64 + (p.w as i64 - photo.width as i64) / 2;
    let dy = p.y as i64 + (p.h as i64 - photo.height as i64) / 2;
    for y in 0..photo.height {
        let py = dy + y as i64;
        if py < 0 || py >= page.height as i64 {
            continue;
        }
        let src = (y as usize) * photo.width as usize * 3;
        let x0 = dx.max(0);
        let x1 = (dx + photo.width as i64).min(page.width as i64);
        if x1 <= x0 {
            continue;
        }
        let sx0 = (x0 - dx) as usize;
        let n = (x1 - x0) as usize;
        let dst = (py as usize * page.width as usize + x0 as usize) * 3;
        page.rgb[dst..dst + n * 3].copy_from_slice(&photo.rgb[src + sx0 * 3..src + (sx0 + n) * 3]);
    }
    if p.strip_h > 0 && !layout.caption.trim().is_empty() {
        let font_bytes = font.unwrap_or(DEFAULT_FONT);
        if let Ok(f) = FontRef::try_from_slice(font_bytes) {
            let px = layout.caption_pt / 72.0 * layout.ppi as f32;
            draw_text(&mut page, &f, &layout.caption, px, layout.caption_rgb, &p, &layout.caption_align);
        }
    }
    page
}

/// Inter Regular, the face used when no other is given (Helvetica Neue Light is not bundled with
/// anything we may redistribute; pass its bytes to use it on the Mac).
pub static DEFAULT_FONT: &[u8] = include_bytes!("../../../assets/fonts/Inter-Regular.ttf");

fn draw_text(page: &mut Image16, font: &FontRef<'_>, text: &str, px: f32, rgb: [u8; 3], p: &Placement, align: &str) {
    let scaled = font.as_scaled(PxScale::from(px));
    // measure
    let mut width = 0.0f32;
    let mut glyphs = Vec::new();
    let mut prev: Option<ab_glyph::GlyphId> = None;
    for c in text.chars() {
        let id = font.glyph_id(c);
        if let Some(pg) = prev {
            width += scaled.kern(pg, id);
        }
        glyphs.push((id, width));
        width += scaled.h_advance(id);
        prev = Some(id);
    }
    let ascent = scaled.ascent();
    let descent = scaled.descent();
    let text_h = ascent - descent;
    // the strip: text baseline so the glyph box is vertically centered in the strip
    let strip_bottom = (p.strip_top + p.strip_h) as f32;
    let baseline = p.strip_top as f32 + (p.strip_h as f32 - text_h) / 2.0 + ascent;
    let baseline = baseline.min(strip_bottom - 1.0);
    let x0 = match align {
        "center" => (p.page_w as f32 - width) / 2.0,
        "right" => (p.page_w - p.margin) as f32 - width,
        _ => p.x as f32,
    };
    let color = rgb.map(|c| {
        // sRGB 8-bit to the page's 16-bit value: neutral grays carry across working spaces
        ((c as f32 / 255.0) * 65535.0).round() as u16
    });
    for (id, off) in glyphs {
        let g = id.with_scale_and_position(PxScale::from(px), ab_glyph::point(x0 + off, baseline));
        let Some(outlined) = font.outline_glyph(g) else { continue };
        let bounds = outlined.px_bounds();
        outlined.draw(|gx, gy, cov| {
            let x = bounds.min.x as i64 + gx as i64;
            let y = bounds.min.y as i64 + gy as i64;
            if x < 0 || y < 0 || x >= page.width as i64 || y >= page.height as i64 {
                return;
            }
            let i = (y as usize * page.width as usize + x as usize) * 3;
            let a = cov.clamp(0.0, 1.0);
            for k in 0..3 {
                let bg = page.rgb[i + k] as f32;
                page.rgb[i + k] = (bg + (color[k] as f32 - bg) * a).round() as u16;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_signature_sheet_places_a_landscape_photo_on_a_turned_page() {
        let l = Layout::signature();
        let p = l.place(6000, 4000);
        assert_eq!((p.page_w, p.page_h), (5700, 3900));
        assert_eq!(p.margin, 75);
        assert_eq!(p.strip_h, 120);
        // fits the height: 3900 - 150 - 120 = 3630 tall, 5445 wide
        assert_eq!((p.w, p.h), (5445, 3630));
        assert_eq!(p.y, 75);
        assert_eq!(p.x, 75 + (5550 - 5445) / 2);
        assert_eq!(p.strip_top, 75 + 3630);
    }

    #[test]
    fn compose_puts_the_photo_on_white_and_writes_the_caption() {
        let mut l = Layout::signature();
        l.ppi = 150;
        let p = l.place(300, 200);
        let mut photo = Image16::new(p.w, p.h);
        photo.rgb.fill(1000);
        let page = compose(&l, &photo, None);
        assert_eq!((page.width, page.height), (p.page_w, p.page_h));
        let at = |x: u32, y: u32| page.rgb[(y as usize * page.width as usize + x as usize) * 3];
        assert_eq!(at(0, 0), u16::MAX);
        assert_eq!(at(p.x + 5, p.y + 5), 1000);
        // something dark was drawn in the strip near its left edge
        let strip: Vec<u16> = (p.strip_top..p.strip_top + p.strip_h).flat_map(|y| (p.x..p.x + 600).map(move |x| (x, y))).map(|(x, y)| at(x, y)).collect();
        assert!(strip.iter().any(|v| *v < 50000), "caption not drawn");
    }
}
