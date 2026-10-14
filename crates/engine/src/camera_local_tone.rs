//! IA Darkroom: the camera's local tone mapping for every raw, not only Apple ProRAW.
//!
//! A ProRAW carries the per-region gains its camera applied as a `ProfileGainTableMap`, and the
//! pipeline renders them when "Camera local tone mapping" is on (`lightcraft_pipeline::local_tone`).
//! Every other raw carries the camera's own JPEG instead, and that JPEG already has the camera's
//! local tone in it (Fujifilm's D-Range Priority, Leica's, Sony's DRO, Canon's Auto Lighting
//! Optimizer, Nikon's Active D-Lighting). The global camera look fitted to that JPEG
//! (`camera_preview`) explains the tone curve and the colour; what it cannot explain, at low spatial
//! frequency, is the local part. This module measures that residual and writes it as a gain table
//! map of the same shape as Apple's, so the render path is one and the same.
//!
//! How: at proxy resolution, for every unclipped pixel, the pipeline's source colour is the fitted
//! look's matrix on the white-balanced sensor value; the camera tone curve predicts its display
//! luminance; the JPEG says what the camera displayed. The gain that makes the curve land on the
//! JPEG is `tone⁻¹(jpeg) / scene`. Gains are pooled per grid cell (16 × 12 over the frame) and per
//! input brightness (17 points), trimmed, filled where a cell has no sample at a brightness, and
//! smoothed across cells, so fine detail (demosaic, sharpening, noise) does not reach the map and
//! only the camera's broad choices do. A map whose gains are all within a few percent of 1 is not
//! kept: the camera did no local work on that frame, and the option then does nothing.

use std::sync::Arc;

use lightcraft_color::{D50, D65, PROPHOTO, REC2020, bradford};
use lightcraft_pipeline::local_tone::LocalTone;
use lightcraft_pipeline::tone::CameraTone;
use lightcraft_raw::RawImage;
use lightcraft_raw::color::CameraTransform;
use lightcraft_raw::gaintable::{GainTableMap, SourcePlacement};

use crate::camera_preview::{CameraLook, luma, proxies};

/// Grid of gain tables over the frame.
const CELLS_H: usize = 16;
const CELLS_V: usize = 12;
/// Brightness points per table (the map's input runs 0..1 in linear ProPhoto luminance).
const POINTS_N: usize = 17;
/// Proxy width in pixels: 256 gives ~16 × 16 samples per cell.
const PROXY: usize = 256;
/// Gains outside this are measurement trouble, not tone mapping.
const GAIN_MIN: f32 = 0.25;
const GAIN_MAX: f32 = 4.0;
/// A map is kept when the gains spread at least this much (log2), i.e. the camera did something.
const MIN_SPREAD_LOG2: f32 = 0.08;
/// Minimum usable samples for a map at all.
const MIN_SAMPLES: usize = 2000;

/// Linear ProPhoto luminance weights (Y row of the RIMM matrix).
const PROPHOTO_Y: [f32; 3] = [0.288_040, 0.711_874, 0.000_086];

/// The raw's camera local tone as a gain table map synthesized from its JPEG, with where the
/// developed picture sits in the active area. `None` when there is no JPEG to learn from, no camera
/// tone curve to invert, too few clean samples, or no local work worth keeping.
pub(crate) fn synthesize(
    raw: &RawImage,
    bytes: &[u8],
    transform: &CameraTransform,
    look: Option<&CameraLook>,
    tone: Option<&CameraTone>,
) -> Option<Arc<LocalTone>> {
    // no curve from a fitted look or the file's tags (a Leica DNG): learn one from the JPEG
    let fitted = if tone.is_none() { crate::camera_preview::tone_from_preview(raw, bytes, transform) } else { None };
    let tone = tone.or(fitted.as_ref())?;
    let map = synthesize_map(raw, bytes, transform, look, tone)?;
    let a = raw.active_area;
    let (aw, ah) = (a.width.max(1) as f64, a.height.max(1) as f64);
    let c = raw.develop_crop(a.width, a.height);
    let rect = [c.x as f64 / aw, c.y as f64 / ah, c.width as f64 / aw, c.height as f64 / ah];
    let placement = SourcePlacement { rect, orientation: raw.orientation };
    Some(Arc::new(LocalTone { map, placement }))
}

fn synthesize_map(raw: &RawImage, bytes: &[u8], transform: &CameraTransform, look: Option<&CameraLook>, tone: &CameraTone) -> Option<GainTableMap> {
    let (sensor, reference, clipped) = proxies(raw, bytes, transform, PROXY)?;
    if sensor.width < 32 || sensor.height < 32 || reference.width != sensor.width || reference.height != sensor.height {
        return None;
    }
    let look_matrix = look.map(|l| l.matrix.to_f32());
    let to_prophoto = PROPHOTO.from_xyz().mul(&bradford(D65, D50)).mul(&REC2020.to_xyz()).to_f32();
    let inv = ToneInverse::new(tone);
    let (w, h) = (sensor.width, sensor.height);
    // sums of log2 gain and counts, per cell and brightness point
    let tables = CELLS_V * CELLS_H;
    let mut sum = vec![0f32; tables * POINTS_N];
    let mut cnt = vec![0u32; tables * POINTS_N];
    // the proxy's frame is the developed crop in the raw's orientation; the map is addressed in the
    // active area, oriented like the sensor. The proxy is unoriented sensor (except CR3, oriented
    // both ways by `proxies`); the cell grid below is over the proxy, and the map's grid is placed
    // over the developed crop by `placement`, so the two agree.
    let mut samples = 0usize;
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if clipped.get(i).copied().unwrap_or(true) {
                continue;
            }
            let s = sensor.data[i];
            if !s.iter().all(|c| c.is_finite()) {
                continue;
            }
            let scene = match &look_matrix {
                Some(m) => mat_apply(m, s),
                None => s,
            };
            let y_scene = luma(scene.map(f64::from)) as f32;
            let r = reference.data[i];
            let y_ref = luma(r.map(f64::from)) as f32;
            // nothing to learn from black, and the display white is where the JPEG clips
            if y_scene < 2e-4 || y_ref < 1e-3 || y_ref > 0.97 {
                continue;
            }
            let Some(y_want) = inv.apply(y_ref) else { continue };
            let g = (y_want / y_scene).clamp(GAIN_MIN, GAIN_MAX);
            // the map's input: linear ProPhoto luminance of the pipeline's source pixel
            let q = mat_apply(&to_prophoto, scene);
            let input = (PROPHOTO_Y[0] * q[0] + PROPHOTO_Y[1] * q[1] + PROPHOTO_Y[2] * q[2]).clamp(0.0, 1.0);
            let p = ((input * POINTS_N as f32).round() as usize).min(POINTS_N - 1);
            let cx = (x * CELLS_H / w).min(CELLS_H - 1);
            let cy = (y * CELLS_V / h).min(CELLS_V - 1);
            let k = (cy * CELLS_H + cx) * POINTS_N + p;
            sum[k] += g.log2();
            cnt[k] += 1;
            samples += 1;
        }
    }
    if samples < MIN_SAMPLES {
        return None;
    }
    // mean log gain per (cell, point); empty points filled from the nearest filled point of the
    // same cell, empty cells from their neighbours
    let mut log_g = vec![f32::NAN; tables * POINTS_N];
    for k in 0..log_g.len() {
        if cnt[k] >= 3 {
            log_g[k] = sum[k] / cnt[k] as f32;
        }
    }
    for t in 0..tables {
        let row = &mut log_g[t * POINTS_N..(t + 1) * POINTS_N];
        fill_row(row);
    }
    fill_cells(&mut log_g);
    // smooth across cells (3 × 3, twice) and along brightness (3-tap)
    for _ in 0..2 {
        log_g = smooth_cells(&log_g);
    }
    for t in 0..tables {
        let row = &mut log_g[t * POINTS_N..(t + 1) * POINTS_N];
        smooth_row(row);
    }
    // keep only maps that do something
    let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
    for &v in &log_g {
        if v.is_finite() {
            lo = lo.min(v);
            hi = hi.max(v);
        }
    }
    if !lo.is_finite() || hi - lo < MIN_SPREAD_LOG2 {
        return None;
    }
    let gains: Vec<f32> = log_g.iter().map(|v| if v.is_finite() { v.exp2().clamp(GAIN_MIN, GAIN_MAX) } else { 1.0 }).collect();
    Some(GainTableMap {
        points_v: CELLS_V,
        points_h: CELLS_H,
        points_n: POINTS_N,
        // cell centres from 1/(2n) to 1 - 1/(2n) of the active area
        spacing_v: 1.0 / CELLS_V as f64,
        spacing_h: 1.0 / CELLS_H as f64,
        origin_v: 0.5 / CELLS_V as f64,
        origin_h: 0.5 / CELLS_H as f64,
        weights: [PROPHOTO_Y[0], PROPHOTO_Y[1], PROPHOTO_Y[2], 0.0, 0.0],
        gamma: 1.0,
        gains,
    })
}

fn mat_apply(m: &[[f32; 3]; 3], p: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| m[i][0] * p[0] + m[i][1] * p[1] + m[i][2] * p[2])
}

/// Fill NaN points of a brightness row from the nearest filled point on either side (linear
/// between two, flat beyond the ends). A row with nothing stays NaN.
fn fill_row(row: &mut [f32]) {
    let n = row.len();
    let filled: Vec<usize> = (0..n).filter(|&i| row[i].is_finite()).collect();
    if filled.is_empty() {
        return;
    }
    for i in 0..n {
        if row[i].is_finite() {
            continue;
        }
        let prev = filled.iter().rev().find(|&&f| f < i);
        let next = filled.iter().find(|&&f| f > i);
        row[i] = match (prev, next) {
            (Some(&a), Some(&b)) => {
                let t = (i - a) as f32 / (b - a) as f32;
                row[a] + (row[b] - row[a]) * t
            }
            (Some(&a), None) => row[a],
            (None, Some(&b)) => row[b],
            (None, None) => f32::NAN,
        };
    }
}

/// Fill cells with no samples at all from the mean of their filled neighbours, repeated until
/// every cell has values (a frame with samples somewhere always converges).
fn fill_cells(log_g: &mut [f32]) {
    let empty = |g: &[f32], t: usize| g[t * POINTS_N..(t + 1) * POINTS_N].iter().all(|v| !v.is_finite());
    for _ in 0..(CELLS_H + CELLS_V) {
        let before = log_g.to_vec();
        let mut changed = false;
        for cy in 0..CELLS_V {
            for cx in 0..CELLS_H {
                let t = cy * CELLS_H + cx;
                if !empty(&before, t) {
                    continue;
                }
                let mut acc = [0f32; POINTS_N];
                let mut n = 0;
                for (dy, dx) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                    let (ny, nx) = (cy as i32 + dy, cx as i32 + dx);
                    if ny < 0 || nx < 0 || ny >= CELLS_V as i32 || nx >= CELLS_H as i32 {
                        continue;
                    }
                    let u = ny as usize * CELLS_H + nx as usize;
                    if empty(&before, u) {
                        continue;
                    }
                    for p in 0..POINTS_N {
                        acc[p] += before[u * POINTS_N + p];
                    }
                    n += 1;
                }
                if n > 0 {
                    for p in 0..POINTS_N {
                        log_g[t * POINTS_N + p] = acc[p] / n as f32;
                    }
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
}

/// 3 × 3 box over cells, per brightness point (edges replicate).
fn smooth_cells(log_g: &[f32]) -> Vec<f32> {
    let mut out = vec![0f32; log_g.len()];
    for cy in 0..CELLS_V {
        for cx in 0..CELLS_H {
            for p in 0..POINTS_N {
                let mut acc = 0f32;
                let mut n = 0f32;
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let ny = (cy as i32 + dy).clamp(0, CELLS_V as i32 - 1) as usize;
                        let nx = (cx as i32 + dx).clamp(0, CELLS_H as i32 - 1) as usize;
                        let v = log_g[(ny * CELLS_H + nx) * POINTS_N + p];
                        if v.is_finite() {
                            acc += v;
                            n += 1.0;
                        }
                    }
                }
                out[(cy * CELLS_H + cx) * POINTS_N + p] = if n > 0.0 { acc / n } else { f32::NAN };
            }
        }
    }
    out
}

fn smooth_row(row: &mut [f32]) {
    let src = row.to_vec();
    let n = src.len();
    for i in 0..n {
        let a = src[i.saturating_sub(1)];
        let b = src[i];
        let c = src[(i + 1).min(n - 1)];
        if a.is_finite() && b.is_finite() && c.is_finite() {
            row[i] = 0.25 * a + 0.5 * b + 0.25 * c;
        }
    }
}

/// The camera tone curve inverted on its knots (scene luminance for a display luminance), with
/// the shoulder beyond the last knot left out: the JPEG's top end is where the curve is least
/// trustworthy.
struct ToneInverse {
    knots: Vec<[f32; 2]>,
}

impl ToneInverse {
    fn new(tone: &CameraTone) -> Self {
        let mut knots: Vec<[f32; 2]> = vec![[0.0, 0.0]];
        for k in tone.knots() {
            if k[0].is_finite() && k[1].is_finite() && k[0] > knots.last().unwrap()[0] && k[1] > knots.last().unwrap()[1] {
                knots.push(*k);
            }
        }
        Self { knots }
    }

    fn apply(&self, display: f32) -> Option<f32> {
        let last = self.knots.last()?;
        if display > last[1] || self.knots.len() < 2 {
            return None;
        }
        for w in self.knots.windows(2) {
            let (a, b) = (w[0], w[1]);
            if display <= b[1] {
                let t = (display - a[1]) / (b[1] - a[1]);
                return Some(a[0] + (b[0] - a[0]) * t);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dng_without_a_tone_curve_or_a_jpeg_gets_no_map_and_no_error() {
        // the JPEG-fitted fallback curve needs a JPEG: none here, so no map (the Q3 path with one is
        // verified on real files)
        let bytes = crate::tests_xmp::synthetic_dng_with(None, Default::default());
        let raw = lightcraft_raw::decode(&bytes).unwrap();
        let xy = lightcraft_raw::color::as_shot_white_xy(&raw);
        let t = lightcraft_raw::color::camera_transform(&raw, xy);
        assert!(raw.color.profile.tone_curve.is_none());
        assert!(crate::camera_preview::tone_from_preview(&raw, &bytes, &t).is_none());
        assert!(synthesize(&raw, &bytes, &t, None, None).is_none());
    }

    #[test]
    fn fill_row_interpolates_between_known_points_and_holds_the_ends() {
        let mut row = [f32::NAN, 0.0, f32::NAN, f32::NAN, 0.6, f32::NAN];
        fill_row(&mut row);
        for (a, b) in row.iter().zip([0.0, 0.0, 0.2, 0.4, 0.6, 0.6]) {
            assert!((a - b).abs() < 1e-5, "{row:?}");
        }
    }

    #[test]
    fn tone_inverse_undoes_the_curve_inside_the_knots() {
        // strictly increasing, inside (0, 1): the constructor's rules
        let knots: [[f32; 2]; 32] = std::array::from_fn(|i| {
            let x = 0.01 + 0.95 * i as f32 / 31.0;
            [x, x.powf(0.45) * 0.98]
        });
        let tone = CameraTone::new(knots).unwrap();
        let inv = ToneInverse::new(&tone);
        for y in [0.05f32, 0.2, 0.5, 0.8] {
            let x = inv.apply(tone.apply(y)).unwrap();
            assert!((x - y).abs() < 0.02, "{y} -> {x}");
        }
        assert!(inv.apply(1.5).is_none());
    }
}
