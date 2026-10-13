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
