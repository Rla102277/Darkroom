//! Darkroom: printing commands (`print.*`), milestone 2, the proof.
//!
//! Two hard rules, enforced here and in the helper: the file goes to the NATIVE Epson driver only
//! (an AirPrint queue is refused), and Darkroom manages color: the file is converted into the
//! paper profile before it leaves the app and the driver gets its color-off option. Advanced B&W
//! is the exception by design: in that mode the printer maps tones itself, so the file is sent
//! as sRGB with the driver's B&W options instead.
//!
//! The option names differ by driver version, so nothing is hard-coded: `print.options` dumps
//! what the driver on this Mac exposes, and `print.proof` takes the options to send by name.

use serde_json::{Value, json};

use super::{CommandSpec, always, bad, cmd, str_param};
use crate::{Result, Session};

fn helper(s: &Session, cmd: &str) -> Result<darkroom_photos::Photos> {
    s.photos.clone().ok_or_else(|| bad(cmd, super::photos::unavailable()))
}

fn print_enabled(s: &Session) -> std::result::Result<(), String> {
    match &s.photos {
        Some(_) => Ok(()),
        None => Err(super::photos::unavailable()),
    }
}

/// Where prepared print files go: `<config dir>/print/`.
pub fn print_dir() -> std::path::PathBuf {
    let base = crate::config::config_dir().unwrap_or_else(std::env::temp_dir);
    base.join("print")
}

/// Builds the proof target into a TIFF: in `profile` (a path) when given, else sRGB (for
/// Advanced B&W). Returns the file written.
pub fn make_target(dir: &std::path::Path, sheet: &str, ppi: u32, profile: Option<&str>, intent: darkroom_print::Intent, name: &str) -> Result<(std::path::PathBuf, Option<String>)> {
    const C: &str = "print.target";
    let sheet = darkroom_print::Sheet::parse(sheet).ok_or_else(|| bad(C, format!("unknown sheet `{sheet}` (letter, a4, 5x7, 13x19, or WxH in inches)")))?;
    let img = darkroom_print::target(sheet, ppi, 0.5);
    let (img, icc, pname) = match profile {
        Some(p) => {
            let bytes = std::fs::read(p).map_err(|e| bad(C, format!("{p}: {e}")))?;
            let converted = darkroom_print::to_profile(&img, &bytes, intent).map_err(|e| bad(C, format!("{p}: {e}")))?;
            let pname = darkroom_print::profile_name(&bytes);
            (converted, Some(bytes), pname)
        }
        None => (img, None, None),
    };
    let tiff = darkroom_print::tiff(&img, icc.as_deref(), ppi).map_err(|e| bad(C, e.to_string()))?;
    std::fs::create_dir_all(dir).map_err(|e| bad(C, format!("{}: {e}", dir.display())))?;
    let safe: String = name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    let path = dir.join(format!("{safe}.tif"));
    std::fs::write(&path, tiff).map_err(|e| bad(C, format!("{}: {e}", path.display())))?;
    Ok((path, pname))
}

fn intent_param(p: &Value) -> Result<darkroom_print::Intent> {
    match str_param(p, "intent").unwrap_or("perceptual") {
        "perceptual" => Ok(darkroom_print::Intent::Perceptual),
        "relative" => Ok(darkroom_print::Intent::Relative),
        other => Err(bad("print", format!("unknown intent `{other}` (perceptual|relative)"))),
    }
}


/// `print.sheet`: one catalog photo on a page (the signature sheet by default), rendered at the
/// print's density through LightCraft's own pipeline, composed, converted into the paper's
/// profile, and either saved as a 16-bit TIFF or sent to the P900.
fn sheet(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "print.sheet";
    use darkroom_print::sheet::Layout;
    // the photo
    let id = p.get("id").and_then(Value::as_u64).map(lightcraft_catalog::PhotoId).or_else(|| s.targets(p).first().copied()).ok_or_else(|| bad(C, "id required (or a selection)"))?;
    let photo = s.catalog.photo(id).cloned().ok_or_else(|| bad(C, "no such photo"))?;
    // the paper and mode
    let paper_id = str_param(p, "paper").filter(|x| !x.is_empty()).ok_or_else(|| bad(C, "paper required (see print.papers)"))?;
    let paper = darkroom_print::p900::paper(paper_id).ok_or_else(|| bad(C, format!("unknown paper `{paper_id}` (see print.papers)")))?;
    let abw: Option<darkroom_print::p900::Abw> = match p.get("abw") {
        None | Some(Value::Null) => None,
        Some(v) => Some(serde_json::from_value(v.clone()).map_err(|e| bad(C, format!("abw: {e}")))?),
    };
    let quality = match str_param(p, "quality").unwrap_or("max") {
        "draft" => darkroom_print::p900::Quality::Draft,
        "high" => darkroom_print::p900::Quality::High,
        "max" => darkroom_print::p900::Quality::Max,
        other => return Err(bad(C, format!("unknown quality `{other}` (draft|high|max)"))),
    };
    // the layout: signature by default, every number overridable
    let mut layout = match str_param(p, "layout").unwrap_or("signature") {
        "signature" => Layout::signature(),
        "plain" => Layout::plain(13.0, 19.0, 0.25, 300),
        other => return Err(bad(C, format!("unknown layout `{other}` (signature|plain)"))),
    };
    if let Some(sz) = str_param(p, "size").filter(|x| !x.is_empty()) {
        let size = darkroom_print::p900::Size::parse(sz).ok_or_else(|| bad(C, format!("bad size `{sz}`")))?;
        let (w, h) = size.inches();
        layout.width_in = w;
        layout.height_in = h;
    }
    if let Some(m) = p.get("margin").and_then(Value::as_f64) { layout.margin_in = m as f32; }
    if let Some(st) = p.get("strip").and_then(Value::as_f64) { layout.strip_in = st as f32; }
    if let Some(c) = str_param(p, "caption") { layout.caption = c.to_string(); }
    if let Some(pt) = p.get("captionPt").and_then(Value::as_f64) { layout.caption_pt = pt as f32; }
    if let Some(a) = str_param(p, "captionAlign") { layout.caption_align = a.to_string(); }
    if let Some(ppi) = p.get("ppi").and_then(Value::as_u64) { layout.ppi = ppi.clamp(72, 1440) as u32; }
    if let Some(r) = p.get("autoRotate").and_then(Value::as_bool) { layout.auto_rotate = r; }
    let font: Option<Vec<u8>> = match str_param(p, "font").filter(|f| !f.is_empty()) {
        Some(f) => Some(std::fs::read(f).map_err(|e| bad(C, format!("font {f}: {e}")))?),
        None => None,
    };
    // where the photo lands, so it is rendered at exactly that size
    let (pw, ph) = crate::export::output_size(&photo, &crate::export::ExportOptions::default());
    let place = layout.place(pw as u32, ph as u32);
    let working = if abw.is_some() { lightcraft_pipeline::OutputSpace::Srgb } else { lightcraft_pipeline::OutputSpace::AdobeRgb };
    let opts = crate::export::ExportOptions {
        format: crate::export::ExportFormat::Tiff,
        bit_depth: Some(16),
        color_space: working,
        resize: Some(crate::export::Resize { mode: crate::export::ResizeMode::LongEdge, value: place.w.max(place.h) as f32, height: place.h, dont_enlarge: false }),
        ppi: layout.ppi.min(u16::MAX as u32) as u16,
        sharpen: if paper.matte { crate::export::SharpenFor::Matte } else { crate::export::SharpenFor::Glossy },
        sharpen_amount: match str_param(p, "sharpen").unwrap_or("standard") {
            "low" => crate::export::SharpenAmount::Low,
            "high" => crate::export::SharpenAmount::High,
            _ => crate::export::SharpenAmount::Standard,
        },
        metadata: crate::export::MetadataPolicy::All,
        tiff_compression: crate::export::TiffCompression::None,
        ..Default::default()
    };
    let exported = crate::export::export_photo(s, id, &opts, 1).map_err(|e| bad(C, format!("render: {e}")))?;
    // back to 16-bit samples in the working space
    let decoded = lightcraft_codecs::decode(&exported.bytes, lightcraft_codecs::DecodeOptions::default()).map_err(|e| bad(C, format!("decode: {e}")))?;
    let trc = working.trc();
    let img = decoded.image;
    let mut rgb = Vec::with_capacity(img.width * img.height * 3);
    for px in &img.data {
        for k in 0..3 {
            rgb.push((trc.encode(px[k]) * 65535.0).round() as u16);
        }
    }
    let photo16 = darkroom_print::Image16 { width: img.width as u32, height: img.height as u32, rgb };
    let page = darkroom_print::sheet::compose(&layout, &photo16, font.as_deref());
    // into the paper's profile (color), or left as sRGB for Advanced B&W
    let (page, icc, pname) = if abw.is_some() {
        let srgb = lightcraft_codecs::icc::write_named(lightcraft_codecs::space::NamedSpace::Srgb);
        (page, srgb, None)
    } else {
        let profile = match str_param(p, "profile").filter(|x| !x.is_empty()) {
            Some(x) => std::path::PathBuf::from(x),
            None => darkroom_print::p900::find_profile(paper, &darkroom_print::installed_profiles())
                .ok_or_else(|| bad(C, format!("no P900 profile for {} is installed; pass profile=", paper.name)))?,
        };
        let bytes = std::fs::read(&profile).map_err(|e| bad(C, format!("{}: {e}", profile.display())))?;
        let src = lightcraft_codecs::icc::write_named(lightcraft_codecs::space::NamedSpace::AdobeRgb);
        let converted = darkroom_print::to_profile_from(&page, &src, &bytes, intent_param(p)?).map_err(|e| bad(C, e.to_string()))?;
        let name = darkroom_print::profile_name(&bytes);
        (converted, bytes, name)
    };
    let tiff = darkroom_print::tiff(&page, Some(&icc), layout.ppi).map_err(|e| bad(C, e.to_string()))?;
    // write
    let title = if photo.meta.title.trim().is_empty() { photo.file_name.clone() } else { photo.meta.title.clone() };
    let safe: String = title.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' { c } else { '_' }).collect();
    let out = match str_param(p, "out").filter(|x| !x.is_empty()) {
        Some(o) => std::path::PathBuf::from(o),
        None => {
            let dir = print_dir();
            std::fs::create_dir_all(&dir).map_err(|e| bad(C, format!("{}: {e}", dir.display())))?;
            dir.join(format!("{} {} {}.tif", safe.trim(), paper.id, if abw.is_some() { "abw" } else { "color" }))
        }
    };
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| bad(C, format!("{}: {e}", parent.display())))?;
    }
    std::fs::write(&out, &tiff).map_err(|e| bad(C, format!("{}: {e}", out.display())))?;
    let mut result = json!({
        "file": out.to_string_lossy(),
        "page": {"width": page.width, "height": page.height, "widthIn": page.width as f32 / layout.ppi as f32, "heightIn": page.height as f32 / layout.ppi as f32},
        "image": {"x": place.x, "y": place.y, "width": photo16.width, "height": photo16.height, "ppi": layout.ppi},
        "profile": pname,
        "paper": paper.id,
    });
    // print
    let do_print = p.get("print").and_then(Value::as_bool).unwrap_or(str_param(p, "out").is_none());
    if do_print {
        let h = helper(s, C)?;
        let queue = str_param(p, "queue").filter(|q| !q.is_empty()).unwrap_or(darkroom_print::p900::QUEUE);
        let (w_in, h_in) = (page.width as f32 / layout.ppi as f32, page.height as f32 / layout.ppi as f32);
        // the driver wants the page as the printer sees it: width across the feed
        let (sw, sh) = if w_in > h_in { (h_in, w_in) } else { (w_in, h_in) };
        let size = darkroom_print::p900::Size::parse(&format!("{sw}x{sh}")).unwrap_or(darkroom_print::p900::Size::Custom { width_in: sw, height_in: sh });
        let roll = p.get("rollWidth").and_then(Value::as_u64).map(|w| w as u32);
        let job = darkroom_print::p900::Job { paper: paper.id.to_string(), size, quality, abw: abw.clone(), borderless: false, roll_width_in: roll, banner: false };
        let mut options = darkroom_print::p900::options(&job).map_err(|e| bad(C, e))?;
        if let Some(extra) = p.get("options").and_then(Value::as_object) {
            for (k, v) in extra {
                options.insert(k.clone(), match v { Value::String(s) => s.clone(), other => other.to_string() });
            }
        }
        let opts_json = serde_json::to_value(&options).unwrap_or_default();
        let copies = p.get("copies").and_then(Value::as_u64).unwrap_or(1).clamp(1, 99) as u32;
        let job_title = format!("Darkroom: {title} on {}", paper.name);
        let r = h.print_submit(queue, &out, &job_title, copies, &opts_json).map_err(|e| bad(C, e.to_string()))?;
        result["jobId"] = r["jobId"].clone();
        result["request"] = r["request"].clone();
        result["options"] = opts_json;
    }
    Ok(result)
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "print.queues", "Print Queues", [], None,
            "{} → {items: [{name, model?, isDefault, airPrint, epsonDriver, nickName?}], default} — the Mac's print queues; Darkroom prints only to one with epsonDriver",
            print_enabled, |s, _| {
                let h = helper(s, "print.queues")?;
                h.print_queues().map_err(|e| bad("print.queues", e.to_string()))
            }),
        cmd!(query "print.options", "Print Driver Options", [], None,
            "{queue} → {queue, options: [{key, label, choices: [{value, label?, current}]}]} — everything the queue's driver exposes; the keys here are what print.proof and print.submit send",
            print_enabled, |s, p| {
                let h = helper(s, "print.options")?;
                let q = str_param(p, "queue").filter(|q| !q.is_empty()).ok_or_else(|| bad("print.options", "queue required"))?;
                h.print_options(q).map_err(|e| bad("print.options", e.to_string()))
            }),
        cmd!(query "print.profiles", "Installed Paper Profiles", [], None,
            "{} → {items: [{path, name}]} — ICC profiles installed on this Mac (user and system ColorSync folders, Epson's folder)",
            always, |_, _| {
                let items: Vec<Value> = darkroom_print::installed_profiles().into_iter().map(|(p, n)| json!({"path": p.to_string_lossy(), "name": n})).collect();
                Ok(json!({"items": items}))
            }),
        cmd!(query "print.target", "Make Proof Target", [], None,
            "{sheet?: letter|a4|5x7|13x19|WxH, ppi?: 360, profile?: path to the paper's ICC (omit for the Advanced B&W sheet, which stays sRGB), intent?: perceptual|relative, name?} → {file, profile?} — writes the 16-bit TIFF proof target",
            always, |_, p| {
                let sheet = str_param(p, "sheet").unwrap_or("letter");
                let ppi = p.get("ppi").and_then(Value::as_u64).unwrap_or(360).clamp(72, 1440) as u32;
                let profile = str_param(p, "profile").filter(|x| !x.is_empty());
                let name = str_param(p, "name").unwrap_or(if profile.is_some() { "proof-target-color" } else { "proof-target-bw" });
                let (file, pname) = make_target(&print_dir(), sheet, ppi, profile, intent_param(p)?, name)?;
                Ok(json!({"file": file.to_string_lossy(), "profile": pname}))
            }),
        cmd!(query "print.submit", "Send File to Printer", [], None,
            "{queue, file, title?, copies?: 1, options?: {driverKey: value}} → {jobId, request, options} — hands a prepared file to the native driver with the options named (keys from print.options). An AirPrint queue is refused.",
            print_enabled, |s, p| {
                const C: &str = "print.submit";
                let h = helper(s, C)?;
                let q = str_param(p, "queue").filter(|q| !q.is_empty()).ok_or_else(|| bad(C, "queue required"))?;
                let file = str_param(p, "file").filter(|f| !f.is_empty()).ok_or_else(|| bad(C, "file required"))?;
                let title = str_param(p, "title").unwrap_or("Darkroom");
                let copies = p.get("copies").and_then(Value::as_u64).unwrap_or(1).clamp(1, 99) as u32;
                let options = p.get("options").cloned().unwrap_or_else(|| json!({}));
                h.print_submit(q, std::path::Path::new(file), title, copies, &options).map_err(|e| bad(C, e.to_string()))
            }),
        cmd!(query "print.papers", "Papers", [], None,
            "{} → {items: [{id, name, media, feed, overcoat, glossSmoothing, matte, profile?: path found on this Mac}]} — the papers Darkroom can print on the P900, with each one's ICC profile when it is installed",
            always, |_, _| {
                let installed = darkroom_print::installed_profiles();
                let items: Vec<Value> = darkroom_print::p900::PAPERS.iter().map(|p| {
                    let mut v = serde_json::to_value(p).unwrap_or_default();
                    v["profile"] = json!(darkroom_print::p900::find_profile(p, &installed).map(|x| x.to_string_lossy().to_string()));
                    v
                }).collect();
                Ok(json!({"items": items, "queue": darkroom_print::p900::QUEUE}))
            }),
        cmd!(query "print.proof", "Print Proof Target", [], None,
            "{queue?: the P900 queue, paper: id from print.papers, size?: letter|a4|5x7|8x10|13x19|17x22|WxH (default letter), quality?: draft|high|max, abw?: {toning?: neutral|cool|warm|sepia, tone?: darkest|darker|dark|normal|light, brightness?, contrast?, shadowTonality?, highlightTonality?, maxOpticalDensity?, highlightPointShift?: off|on|fullPage, wheelX?, wheelY?} (present = the Advanced B&W sheet; absent = the color sheet in the paper's profile), profile?: ICC path to override the one found, intent?, ppi?: 360, options?: {driverKey: value} overrides} → {file, jobId, request, options, profile} — makes the proof target and sends it through the native driver in one step",
            print_enabled, |s, p| {
                const C: &str = "print.proof";
                let h = helper(s, C)?;
                let queue = str_param(p, "queue").filter(|q| !q.is_empty()).unwrap_or(darkroom_print::p900::QUEUE);
                let paper_id = str_param(p, "paper").filter(|x| !x.is_empty()).ok_or_else(|| bad(C, "paper required (see print.papers)"))?;
                let paper = darkroom_print::p900::paper(paper_id).ok_or_else(|| bad(C, format!("unknown paper `{paper_id}` (see print.papers)")))?;
                let size = darkroom_print::p900::Size::parse(str_param(p, "size").unwrap_or("letter")).ok_or_else(|| bad(C, "bad size"))?;
                let quality = match str_param(p, "quality").unwrap_or("max") {
                    "draft" => darkroom_print::p900::Quality::Draft,
                    "high" => darkroom_print::p900::Quality::High,
                    "max" => darkroom_print::p900::Quality::Max,
                    other => return Err(bad(C, format!("unknown quality `{other}` (draft|high|max)"))),
                };
                let abw: Option<darkroom_print::p900::Abw> = match p.get("abw") {
                    None | Some(Value::Null) => None,
                    Some(v) => Some(serde_json::from_value(v.clone()).map_err(|e| bad(C, format!("abw: {e}")))?),
                };
                let ppi = p.get("ppi").and_then(Value::as_u64).unwrap_or(360).clamp(72, 1440) as u32;
                // the file: in the paper profile for color, sRGB for ABW
                let profile: Option<String> = if abw.is_some() {
                    None
                } else {
                    match str_param(p, "profile").filter(|x| !x.is_empty()) {
                        Some(x) => Some(x.to_string()),
                        None => Some(darkroom_print::p900::find_profile(paper, &darkroom_print::installed_profiles())
                            .ok_or_else(|| bad(C, format!("no P900 profile for {} is installed; install Red River's P900 profiles or pass profile=", paper.name)))?
                            .to_string_lossy().to_string()),
                    }
                };
                let (w, hgt) = size.inches();
                let sheet = format!("{w}x{hgt}");
                let name = if abw.is_some() { format!("proof-{}-abw", paper.id) } else { format!("proof-{}-color", paper.id) };
                let (file, pname) = make_target(&print_dir(), &sheet, ppi, profile.as_deref(), intent_param(p)?, &name)?;
                let job = darkroom_print::p900::Job { paper: paper.id.to_string(), size, quality, abw: abw.clone(), borderless: false, roll_width_in: None, banner: false };
                let mut options = darkroom_print::p900::options(&job).map_err(|e| bad(C, e))?;
                if let Some(extra) = p.get("options").and_then(Value::as_object) {
                    for (k, v) in extra {
                        options.insert(k.clone(), match v { Value::String(s) => s.clone(), other => other.to_string() });
                    }
                }
                let opts_json = serde_json::to_value(&options).unwrap_or_default();
                let title = format!("Darkroom proof: {} {}", paper.name, if abw.is_some() { "(Advanced B&W)" } else { "(color)" });
                let mut r = h.print_submit(queue, &file, &title, 1, &opts_json).map_err(|e| bad(C, e.to_string()))?;
                r["file"] = json!(file.to_string_lossy());
                r["profile"] = json!(pname.or(profile));
                r["options"] = opts_json;
                Ok(r)
            }),
        cmd!(query "print.sheet", "Print Signature Sheet", ["File"], None,
            "{id?: photo (default: the selection), paper: id from print.papers, layout?: signature|plain, size?: 13x19 (default) | 17x22 | letter | WxH, margin?: 0.25, strip?: 0.4 (0 = none), caption?, captionPt?: 7, captionAlign?: left|center|right, ppi?: 300, autoRotate?: true, font?: path to a .ttf/.otf (default Inter), quality?: draft|high|max, sharpen?: low|standard|high, abw?: {…} (Advanced B&W), profile?: ICC override, intent?, out?: path (write the TIFF only), print?: bool (default: print unless out is given), copies?, rollWidth?: inches, options?: driver overrides} → {file, page, image, profile, jobId?, request?, options?} — renders the photo at print density, lays it on the page with the signature strip, converts into the paper profile and prints through the native driver (or saves the 16-bit TIFF)",
            always, sheet),
        cmd!(query "print.jobs", "Print Jobs", [], None, "{queue?} → {items: [lines from lpstat -o]}", print_enabled, |s, p| {
            let h = helper(s, "print.jobs")?;
            h.print_jobs(str_param(p, "queue").filter(|q| !q.is_empty())).map_err(|e| bad("print.jobs", e.to_string()))
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_target_is_written_and_named_by_mode() {
        let dir = std::env::temp_dir().join(format!("darkroom-print-test-{}", std::process::id()));
        let (file, pname) = make_target(&dir, "5x7", 72, None, darkroom_print::Intent::Perceptual, "proof-target-bw").unwrap();
        assert!(file.ends_with("proof-target-bw.tif"), "{}", file.display());
        assert_eq!(pname, None);
        assert!(std::fs::metadata(&file).unwrap().len() > 1000);
        assert!(make_target(&dir, "nope", 72, None, darkroom_print::Intent::Perceptual, "x").is_err());
        assert!(make_target(&dir, "5x7", 72, Some("/nonexistent.icc"), darkroom_print::Intent::Perceptual, "x").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn submit_goes_through_the_helper_with_the_options() {
        let mut s = Session::new();
        s.photos = Some(darkroom_photos::Photos::with_runner(darkroom_photos::fake_runner(|cmd, p| match cmd {
            "print.submit" => {
                assert_eq!(p["queue"], "P900");
                assert_eq!(p["options"]["ColorModel"], "RGB");
                Ok(json!({"jobId": "P900-7", "request": "request id is P900-7 (1 file(s))"}))
            }
            "print.queues" => Ok(json!({"items": [{"name": "P900", "epsonDriver": true, "airPrint": false}], "default": "P900"})),
            _ => Err(darkroom_photos::Error::Helper("unexpected".into())),
        })));
        let v = s.execute("print.submit", &json!({"queue": "P900", "file": "/tmp/x.tif", "options": {"ColorModel": "RGB"}})).unwrap();
        assert_eq!(v["jobId"], "P900-7");
        assert_eq!(s.execute("print.queues", &Value::Null).unwrap()["default"], "P900");
        assert!(s.execute("print.submit", &json!({"queue": "P900"})).is_err());
    }
}
