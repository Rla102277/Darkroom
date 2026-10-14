//! Darkroom printing, the part that does not need macOS: the test target, the conversion of
//! 16-bit RGB into a paper's ICC profile, and the TIFF that goes to the driver.
//!
//! The rule the whole print path follows: Darkroom manages color, the driver does not. The file
//! handed to the Epson driver is already in the paper profile's device RGB, 16 bits per channel,
//! with the profile embedded for anything that looks at the file later; the driver is sent its
//! "no color management" option alongside (see the helper's `print.submit`).
//!
//! The proof target is the first thing printed on a new paper: a 21-step gray wedge (neutrals
//! are where a wrong setting shows first), primaries and secondaries, memory colors, and fine
//! line bars for resolution and sharpening. Printed through the native driver with color off and
//! again in Advanced B&W mode, the two sheets settle milestone 2.

pub mod p900;
pub mod sheet;

use lightcraft_codecs::encode::{EncodeImage, EncodeMeta, Samples, TiffCompression, encode_tiff};
use moxcms::{ColorProfile, DataColorSpace, Layout, RenderingIntent, TransformOptions};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Profile(String),
    #[error("{0}")]
    Encode(String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// An RGB image, 16 bits per channel, interleaved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image16 {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<u16>,
}

impl Image16 {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height, rgb: vec![u16::MAX; (width as usize) * (height as usize) * 3] }
    }

    pub fn fill(&mut self, x0: u32, y0: u32, w: u32, h: u32, rgb: [u16; 3]) {
        let (x1, y1) = ((x0 + w).min(self.width), (y0 + h).min(self.height));
        for y in y0..y1 {
            let row = y as usize * self.width as usize;
            for x in x0..x1 {
                let i = (row + x as usize) * 3;
                self.rgb[i..i + 3].copy_from_slice(&rgb);
            }
        }
    }
}

/// Rendering intent for the paper conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Intent {
    /// Keeps the image's gradation intact inside the paper's range; the usual choice for photos.
    #[default]
    Perceptual,
    /// Exact colors inside the range, clipped outside. Black point compensation is not applied
    /// yet (moxcms has none), so deep shadows can block up; use Perceptual until it is.
    Relative,
}

/// Sizes the target is made in. Inches.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Sheet {
    pub width_in: f32,
    pub height_in: f32,
}

impl Sheet {
    pub const LETTER: Sheet = Sheet { width_in: 8.5, height_in: 11.0 };
    pub const A4: Sheet = Sheet { width_in: 8.27, height_in: 11.69 };
    pub const FIVE_BY_SEVEN: Sheet = Sheet { width_in: 5.0, height_in: 7.0 };
    pub const THIRTEEN_BY_NINETEEN: Sheet = Sheet { width_in: 13.0, height_in: 19.0 };

    pub fn parse(s: &str) -> Option<Sheet> {
        match s.trim().to_ascii_lowercase().as_str() {
            "letter" | "8.5x11" => Some(Self::LETTER),
            "a4" => Some(Self::A4),
            "5x7" => Some(Self::FIVE_BY_SEVEN),
            "13x19" | "a3+" | "super b" => Some(Self::THIRTEEN_BY_NINETEEN),
            other => {
                let (w, h) = other.split_once('x')?;
                Some(Sheet { width_in: w.trim().parse().ok()?, height_in: h.trim().parse().ok()? })
            }
        }
    }
}

fn g(v: f32) -> u16 {
    (v.clamp(0.0, 1.0) * 65535.0).round() as u16
}

/// The proof target in sRGB, at `ppi`, with `margin_in` of white around it (the driver's own
/// margin is on top of that). Everything is placed in inches so the sheet reads the same at any
/// resolution.
pub fn target(sheet: Sheet, ppi: u32, margin_in: f32) -> Image16 {
    let px = |inches: f32| (inches * ppi as f32).round() as u32;
    let mut img = Image16::new(px(sheet.width_in), px(sheet.height_in));
    let left = px(margin_in);
    let top = px(margin_in);
    let inner_w = img.width.saturating_sub(2 * left);
    let mut y = top;

    // 1. gray wedge, 21 steps, white to black, sRGB-encoded values (what a monitor shows)
    let steps = 21u32;
    let step_w = inner_w / steps;
    let wedge_h = px(0.9);
    for i in 0..steps {
        let v = 1.0 - i as f32 / (steps - 1) as f32;
        img.fill(left + i * step_w, y, step_w, wedge_h, [g(v); 3]);
    }
    y += wedge_h + px(0.15);
    // 2. the same wedge as a smooth ramp: banding and tonal breaks show here
    let ramp_h = px(0.5);
    for x in 0..inner_w {
        let v = 1.0 - x as f32 / (inner_w - 1).max(1) as f32;
        img.fill(left + x, y, 1, ramp_h, [g(v); 3]);
    }
    y += ramp_h + px(0.25);
    // 3. primaries, secondaries, and memory colors (sRGB 0..1)
    let patches: [[f32; 3]; 14] = [
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 1.0],
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 0.0],
        [0.0, 0.0, 0.0],
        [0.91, 0.75, 0.62], // light skin
        [0.55, 0.35, 0.24], // dark skin
        [0.36, 0.49, 0.21], // foliage
        [0.47, 0.63, 0.86], // sky
        [0.86, 0.26, 0.19], // red brick
        [0.17, 0.33, 0.55], // deep blue
        [0.95, 0.63, 0.14], // orange
    ];
    let cols = 7u32;
    let patch_w = inner_w / cols;
    let patch_h = px(0.9);
    for (i, p) in patches.iter().enumerate() {
        let (r, c) = ((i as u32) / cols, (i as u32) % cols);
        img.fill(left + c * patch_w + px(0.05), y + r * (patch_h + px(0.1)), patch_w - px(0.1), patch_h, [g(p[0]), g(p[1]), g(p[2])]);
    }
    y += 2 * (patch_h + px(0.1)) + px(0.15);
    // 4. near-black and near-white steps: shadow and highlight separation (1% apart)
    let fine = 10u32;
    let fine_w = inner_w / (2 * fine + 1);
    for i in 0..fine {
        let dark = 0.0 + i as f32 * 0.01;
        let light = 1.0 - i as f32 * 0.01;
        img.fill(left + i * fine_w, y, fine_w, px(0.6), [g(dark); 3]);
        img.fill(left + (fine + 1 + i) * fine_w, y, fine_w, px(0.6), [g(light); 3]);
    }
    y += px(0.6) + px(0.25);
    // 5. resolution bars: 1, 2, 3, 4 px line pairs at this ppi (sharpening and head alignment)
    let bar_h = px(0.6);
    let mut x = left;
    for width in [1u32, 2, 3, 4, 6, 8] {
        let group_w = (inner_w / 6).saturating_sub(px(0.1));
        let mut on = true;
        let mut bx = 0;
        while bx + width <= group_w {
            if on {
                img.fill(x + bx, y, width, bar_h, [0; 3]);
            }
            on = !on;
            bx += width;
        }
        x += inner_w / 6;
    }
    y += bar_h + px(0.25);
    // 6. a mid-gray field across the width: a color cast in neutrals shows here
    let field_h = (img.height.saturating_sub(top + y)).min(px(1.2));
    if field_h > 0 {
        img.fill(left, y, inner_w, field_h, [g(0.5); 3]);
    }
    img
}

/// Converts sRGB 16-bit into the output profile's device RGB. Gray and CMYK profiles are
/// refused (the Epson RGB driver path wants RGB).
pub fn to_profile(img: &Image16, profile: &[u8], intent: Intent) -> Result<Image16> {
    let srgb = lightcraft_codecs::icc::write_named(lightcraft_codecs::space::NamedSpace::Srgb);
    to_profile_from(img, &srgb, profile, intent)
}

/// Converts 16-bit RGB encoded in `source` (an RGB working-space profile) into `profile`.
pub fn to_profile_from(img: &Image16, source: &[u8], profile: &[u8], intent: Intent) -> Result<Image16> {
    let dev = ColorProfile::new_from_slice(profile).map_err(|e| Error::Profile(format!("not a valid ICC profile: {e:?}")))?;
    if dev.color_space != DataColorSpace::Rgb {
        return Err(Error::Profile("the paper profile must be an RGB printer profile".into()));
    }
    let src = ColorProfile::new_from_slice(source).map_err(|e| Error::Profile(format!("source profile: {e:?}")))?;
    let opts = TransformOptions {
        rendering_intent: match intent {
            Intent::Perceptual => RenderingIntent::Perceptual,
            Intent::Relative => RenderingIntent::RelativeColorimetric,
        },
        ..TransformOptions::default()
    };
    let t = src
        .create_transform_16bit(Layout::Rgb, &dev, Layout::Rgb, opts)
        .map_err(|e| Error::Profile(format!("can't convert to this profile: {e:?}")))?;
    let mut out = vec![0u16; img.rgb.len()];
    t.transform(&img.rgb, &mut out).map_err(|e| Error::Profile(format!("conversion failed: {e:?}")))?;
    Ok(Image16 { width: img.width, height: img.height, rgb: out })
}

/// The 16-bit TIFF for the driver: `profile` embedded when given (the file's colors are in it),
/// else sRGB.
pub fn tiff(img: &Image16, profile: Option<&[u8]>, ppi: u32) -> Result<Vec<u8>> {
    let srgb = lightcraft_codecs::icc::write_named(lightcraft_codecs::space::NamedSpace::Srgb);
    let icc = profile.unwrap_or(&srgb);
    let e = EncodeImage::new(img.width, img.height, 3, Samples::U16(&img.rgb));
    let meta = EncodeMeta { icc: Some(icc), exif: None, xmp: None, ppi: Some(ppi.min(u16::MAX as u32) as u16) };
    encode_tiff(&e, TiffCompression::Deflate, &meta).map_err(|e| Error::Encode(e.to_string()))
}

/// The description inside a profile, for naming papers.
pub fn profile_name(profile: &[u8]) -> Option<String> {
    lightcraft_codecs::icc::parse(profile).and_then(|i| i.description)
}

/// Where macOS keeps installed printer profiles, in the order they are searched.
pub fn profile_dirs() -> Vec<std::path::PathBuf> {
    let mut v = vec![std::path::PathBuf::from("/Library/ColorSync/Profiles"), std::path::PathBuf::from("/Library/Printers/EPSON")];
    if let Some(h) = std::env::var_os("HOME") {
        v.insert(0, std::path::PathBuf::from(h).join("Library/ColorSync/Profiles"));
    }
    v
}

/// Installed `.icc` / `.icm` files under [`profile_dirs`] (recursively), with their descriptions.
pub fn installed_profiles() -> Vec<(std::path::PathBuf, String)> {
    fn walk(dir: &std::path::Path, out: &mut Vec<(std::path::PathBuf, String)>, depth: u32) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                if depth < 4 {
                    walk(&p, out, depth + 1);
                }
                continue;
            }
            let ext = p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase());
            if matches!(ext.as_deref(), Some("icc" | "icm"))
                && let Ok(bytes) = std::fs::read(&p)
            {
                let name = profile_name(&bytes).unwrap_or_else(|| p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
                out.push((p, name));
            }
        }
    }
    let mut out = Vec::new();
    for d in profile_dirs() {
        walk(&d, &mut out, 0);
    }
    out.sort_by_key(|a| a.1.to_lowercase());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use lightcraft_codecs::space::NamedSpace;

    #[test]
    fn the_target_has_a_white_border_and_a_full_wedge() {
        let t = target(Sheet::LETTER, 100, 0.5);
        assert_eq!((t.width, t.height), (850, 1100));
        // border is white
        assert_eq!(&t.rgb[0..3], &[u16::MAX; 3]);
        // first wedge step is white, last is black
        let px = |x: u32, y: u32| {
            let i = (y as usize * t.width as usize + x as usize) * 3;
            [t.rgb[i], t.rgb[i + 1], t.rgb[i + 2]]
        };
        assert_eq!(px(55, 60), [u16::MAX; 3]);
        let step_w = (850 - 100) / 21;
        assert_eq!(px(50 + 20 * step_w + 2, 60), [0; 3]);
    }

    #[test]
    fn converting_to_srgb_is_the_identity_and_the_tiff_embeds_the_profile() {
        let t = target(Sheet::FIVE_BY_SEVEN, 50, 0.25);
        let srgb = lightcraft_codecs::icc::write_named(NamedSpace::Srgb);
        let out = to_profile(&t, &srgb, Intent::Relative).unwrap();
        let diff = t.rgb.iter().zip(&out.rgb).map(|(a, b)| (*a as i32 - *b as i32).unsigned_abs()).max().unwrap();
        assert!(diff <= 600, "max difference {diff}");
        let bytes = tiff(&out, Some(&srgb), 50).unwrap();
        assert!(bytes.len() > 1000);
        assert!(&bytes[0..2] == b"II" || &bytes[0..2] == b"MM");
        assert!(Sheet::parse("13x19").is_some());
        assert_eq!(Sheet::parse("17 x 22").map(|s| s.height_in), Some(22.0));
        assert!(to_profile(&t, b"junk", Intent::Perceptual).is_err());
    }
}
