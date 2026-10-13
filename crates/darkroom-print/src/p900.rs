//! The Epson SureColor P900 driver, as it exposes itself on Randy's Mac (`print.options` dump of
//! queue `EPSON_SC_P900_Series__IP_`, Oct 13 2026, Epson driver with Red River media files
//! installed). Every option name and value here came from that dump, not from a manual.
//!
//! Two modes, both through the native driver:
//! - **Color, app managed.** Darkroom converts into the paper profile; the driver is told
//!   `EPIJ_CMat=3` (Off, No Color Adjustment) and vendor matching, so it adds nothing.
//! - **Advanced B&W Photo.** `EPIJ_Ink_=3` + `ColorModel=ABWP`: the printer maps the tones; the
//!   file goes as sRGB and the B&W controls ride along as options.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The CUPS queue the P900 is installed as on this Mac.
pub const QUEUE: &str = "EPSON_SC_P900_Series__IP_";

/// Where the sheet is fed from. The P900 has a top feed for photo and fine art paper and a
/// front path for thick board (up to 1.5 mm); rolls need the Roll Media Adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Feed {
    /// `EPIJ_PSrc=2` Standard (top feed).
    Top,
    /// `EPIJ_PSrc=31` Front - Thick.
    FrontThick,
    /// `EPIJ_PSrc=33` Front - Poster Board.
    FrontBoard,
    /// `EPIJ_PSrc=49` Roll Paper.
    Roll,
}

impl Feed {
    pub fn value(self) -> &'static str {
        match self {
            Feed::Top => "2",
            Feed::FrontThick => "31",
            Feed::FrontBoard => "33",
            Feed::Roll => "49",
        }
    }
    pub fn borderless_value(self, retain_size: bool) -> &'static str {
        match (self, retain_size) {
            (Feed::Roll, false) => "50",
            (Feed::Roll, true) => "52",
            (Feed::FrontThick, false) => "32",
            (Feed::FrontThick, true) => "41",
            (_, false) => "3",
            (_, true) => "40",
        }
    }
}

/// A paper Darkroom knows how to print on: the driver's media type, the feed path, and the
/// finishing options that suit its surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Paper {
    /// Short id used in commands (`ultrapro-luster`).
    pub id: &'static str,
    pub name: &'static str,
    /// `EPIJ_Medi` value (the Red River media files register as 19xx).
    pub media: &'static str,
    pub feed: Feed,
    /// Black Enhance Overcoat (`EPIJ_OvCt`): deepens blacks on gloss and luster surfaces.
    pub overcoat: bool,
    /// Gloss Smoothing (`EPIJ_AnGl`): evens the gloss differential on glossy papers.
    pub gloss_smoothing: bool,
    /// Substring that identifies the paper's ICC profile among the installed ones.
    pub profile_hint: &'static str,
    /// Matte papers print with matte black; the driver picks it from the media type.
    pub matte: bool,
}

/// The papers Randy stocks, plus the Epson and Red River media the driver lists, so a new paper
/// is one line here.
pub const PAPERS: &[Paper] = &[
    Paper { id: "ultrapro-luster", name: "Red River UltraPro Luster 300", media: "1925", feed: Feed::Top, overcoat: true, gloss_smoothing: false, profile_hint: "UltraPro Luster", matte: false },
    Paper { id: "big-bend-baryta", name: "Red River Big Bend Baryta 310", media: "1916", feed: Feed::Top, overcoat: true, gloss_smoothing: false, profile_hint: "Big Bend Baryta", matte: false },
    Paper { id: "polar-gloss-metallic", name: "Red River Polar Gloss Metallic 255", media: "1918", feed: Feed::Top, overcoat: true, gloss_smoothing: true, profile_hint: "Polar Gloss Metallic", matte: false },
    Paper { id: "polar-luster-metallic", name: "Red River Polar Luster Metallic", media: "1923", feed: Feed::Top, overcoat: true, gloss_smoothing: false, profile_hint: "Polar Luster Metallic", matte: false },
    Paper { id: "palo-duro-baryta", name: "Red River Palo Duro Baryta v2", media: "1926", feed: Feed::Top, overcoat: true, gloss_smoothing: false, profile_hint: "Palo Duro Baryta", matte: false },
    Paper { id: "palo-duro-satin", name: "Red River Palo Duro Satin", media: "1927", feed: Feed::Top, overcoat: true, gloss_smoothing: false, profile_hint: "Palo Duro Satin", matte: false },
    Paper { id: "palo-duro-softgloss-rag", name: "Red River Palo Duro Softgloss Rag 315", media: "1924", feed: Feed::Top, overcoat: true, gloss_smoothing: false, profile_hint: "Palo Duro Softgloss", matte: false },
    Paper { id: "palo-duro-etching", name: "Red River Palo Duro Etching", media: "1901", feed: Feed::Top, overcoat: false, gloss_smoothing: false, profile_hint: "Palo Duro Etching", matte: true },
    Paper { id: "aurora-white", name: "Red River Aurora White 300", media: "1900", feed: Feed::Top, overcoat: false, gloss_smoothing: false, profile_hint: "Aurora White", matte: true },
    Paper { id: "trinity-river-velvet", name: "Red River Trinity River Velvet 315", media: "1907", feed: Feed::Top, overcoat: false, gloss_smoothing: false, profile_hint: "Trinity River Velvet", matte: true },
    Paper { id: "river-linen", name: "Red River River Linen", media: "1909", feed: Feed::Top, overcoat: false, gloss_smoothing: false, profile_hint: "River Linen", matte: true },
    Paper { id: "polar-matte", name: "Red River Polar Matte 60lb", media: "1915", feed: Feed::Top, overcoat: false, gloss_smoothing: false, profile_hint: "Polar Matte", matte: true },
    Paper { id: "ultrapro-satin", name: "Red River UltraPro Satin", media: "1928", feed: Feed::Top, overcoat: true, gloss_smoothing: false, profile_hint: "UltraPro Satin", matte: false },
    Paper { id: "ultrapro-gloss", name: "Red River UltraPro Gloss", media: "1921", feed: Feed::Top, overcoat: true, gloss_smoothing: true, profile_hint: "UltraPro Gloss", matte: false },
    Paper { id: "epson-premium-luster", name: "Epson Premium Luster Photo Paper", media: "27", feed: Feed::Top, overcoat: true, gloss_smoothing: false, profile_hint: "Premium Luster", matte: false },
    Paper { id: "epson-premium-glossy", name: "Epson Premium Glossy", media: "13", feed: Feed::Top, overcoat: true, gloss_smoothing: true, profile_hint: "Premium Glossy", matte: false },
    Paper { id: "epson-legacy-baryta", name: "Epson Legacy Baryta", media: "157", feed: Feed::Top, overcoat: true, gloss_smoothing: false, profile_hint: "Legacy Baryta", matte: false },
    Paper { id: "epson-legacy-platine", name: "Epson Legacy Platine", media: "1910", feed: Feed::Top, overcoat: true, gloss_smoothing: false, profile_hint: "Legacy Platine", matte: false },
    Paper { id: "epson-legacy-fibre", name: "Epson Legacy Fibre", media: "1905", feed: Feed::Top, overcoat: false, gloss_smoothing: false, profile_hint: "Legacy Fibre", matte: true },
    Paper { id: "epson-exhibition-fiber", name: "Epson Exhibition Fiber", media: "1914", feed: Feed::Top, overcoat: true, gloss_smoothing: false, profile_hint: "Exhibition Fiber", matte: false },
    Paper { id: "epson-hot-press-bright", name: "Epson Hot Press Bright", media: "1912", feed: Feed::Top, overcoat: false, gloss_smoothing: false, profile_hint: "Hot Press Bright", matte: true },
    Paper { id: "epson-cold-press-bright", name: "Epson Cold Press Bright", media: "1908", feed: Feed::Top, overcoat: false, gloss_smoothing: false, profile_hint: "Cold Press Bright", matte: true },
    Paper { id: "epson-velvet-fine-art", name: "Epson Velvet Fine Art Paper", media: "53", feed: Feed::Top, overcoat: false, gloss_smoothing: false, profile_hint: "Velvet Fine Art", matte: true },
    Paper { id: "epson-ultrasmooth", name: "Epson UltraSmooth Fine Art Paper", media: "88", feed: Feed::Top, overcoat: false, gloss_smoothing: false, profile_hint: "UltraSmooth", matte: true },
    Paper { id: "epson-archival-matte", name: "Epson Archival Matte Paper", media: "14", feed: Feed::Top, overcoat: false, gloss_smoothing: false, profile_hint: "Archival Matte", matte: true },
    Paper { id: "epson-metallic-gloss", name: "Epson Metallic Photo Gloss", media: "1903", feed: Feed::Top, overcoat: true, gloss_smoothing: true, profile_hint: "Metallic Photo Paper Glossy", matte: false },
    Paper { id: "epson-metallic-luster", name: "Epson Metallic Photo Luster", media: "1904", feed: Feed::Top, overcoat: true, gloss_smoothing: false, profile_hint: "Metallic Photo Paper Luster", matte: false },
    Paper { id: "canvas-satin", name: "Epson Canvas Satin", media: "153", feed: Feed::FrontThick, overcoat: false, gloss_smoothing: false, profile_hint: "Canvas Satin", matte: false },
    Paper { id: "canvas-matte", name: "Epson Canvas Matte", media: "155", feed: Feed::FrontThick, overcoat: false, gloss_smoothing: false, profile_hint: "Canvas Matte", matte: true },
];

pub fn paper(id: &str) -> Option<&'static Paper> {
    let id = id.trim().to_ascii_lowercase();
    PAPERS.iter().find(|p| p.id == id || p.name.eq_ignore_ascii_case(&id))
}

/// Driver quality. `Max` is Quality Options with SuperPhoto 5760 dpi, High Speed off and Finest
/// Detail on: the slowest and the one for anything that leaves the house.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Quality {
    /// Test strips and packages: Quality (1440 dpi), High Speed on.
    Draft,
    /// 1440 dpi SuperFine, High Speed off, Finest Detail on.
    High,
    #[default]
    Max,
}

/// Paper sizes the driver names (`EPIJ_Size`), plus a custom size for rolls and odd sheets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Size {
    Letter,
    A4,
    A3,
    A3Plus,
    A2,
    FiveBySeven,
    EightByTen,
    ElevenBySeventeen,
    ElevenByFourteen,
    TwelveByTwelve,
    SixteenByTwenty,
    SeventeenByTwentyTwo,
    /// Width and height in inches: `PageSize=Custom.WxH` in points.
    Custom { width_in: f32, height_in: f32 },
}

impl Size {
    pub fn parse(s: &str) -> Option<Size> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "letter" | "8.5x11" => Size::Letter,
            "a4" => Size::A4,
            "a3" => Size::A3,
            "a3+" | "13x19" | "super b" => Size::A3Plus,
            "a2" => Size::A2,
            "5x7" => Size::FiveBySeven,
            "8x10" => Size::EightByTen,
            "11x17" => Size::ElevenBySeventeen,
            "11x14" => Size::ElevenByFourteen,
            "12x12" => Size::TwelveByTwelve,
            "16x20" => Size::SixteenByTwenty,
            "17x22" => Size::SeventeenByTwentyTwo,
            other => {
                let (w, h) = other.split_once('x')?;
                Size::Custom { width_in: w.trim().parse().ok()?, height_in: h.trim().parse().ok()? }
            }
        })
    }

    /// (`EPIJ_Size`, `PageSize`) values.
    pub fn values(&self) -> (Option<&'static str>, String) {
        match self {
            Size::Letter => (Some("4"), "Letter".into()),
            Size::A4 => (Some("1"), "A4".into()),
            Size::A3 => (Some("0"), "A3".into()),
            Size::A3Plus => (Some("10"), "A3+".into()),
            Size::A2 => (Some("13"), "A2".into()),
            Size::FiveBySeven => (Some("76"), "5x7".into()),
            Size::EightByTen => (Some("29"), "8x10".into()),
            Size::ElevenBySeventeen => (Some("35"), "Tabloid".into()),
            Size::ElevenByFourteen => (Some("77"), "11x14".into()),
            Size::TwelveByTwelve => (Some("96"), "12x12".into()),
            Size::SixteenByTwenty => (Some("93"), "16x20".into()),
            Size::SeventeenByTwentyTwo => (Some("37"), "17x22".into()),
            Size::Custom { width_in, height_in } => (None, format!("Custom.{}x{}", (width_in * 72.0).round() as u32, (height_in * 72.0).round() as u32)),
        }
    }

    pub fn inches(&self) -> (f32, f32) {
        match self {
            Size::Letter => (8.5, 11.0),
            Size::A4 => (8.27, 11.69),
            Size::A3 => (11.69, 16.54),
            Size::A3Plus => (13.0, 19.0),
            Size::A2 => (16.54, 23.39),
            Size::FiveBySeven => (5.0, 7.0),
            Size::EightByTen => (8.0, 10.0),
            Size::ElevenBySeventeen => (11.0, 17.0),
            Size::ElevenByFourteen => (11.0, 14.0),
            Size::TwelveByTwelve => (12.0, 12.0),
            Size::SixteenByTwenty => (16.0, 20.0),
            Size::SeventeenByTwentyTwo => (17.0, 22.0),
            Size::Custom { width_in, height_in } => (*width_in, *height_in),
        }
    }
}

/// Advanced B&W Photo controls, exactly the driver's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Abw {
    /// neutral | cool | warm | sepia
    pub toning: String,
    /// darkest | darker | dark | normal | light (the driver's default is Darker)
    pub tone: String,
    /// -25..25
    pub brightness: i32,
    pub contrast: i32,
    pub shadow_tonality: i32,
    pub highlight_tonality: i32,
    /// -50..0
    pub max_optical_density: i32,
    /// off | on | fullPage
    pub highlight_point_shift: String,
    /// Tone wheel, -75..75 each.
    pub wheel_x: i32,
    pub wheel_y: i32,
}

impl Default for Abw {
    fn default() -> Self {
        Self {
            toning: "neutral".into(),
            tone: "dark".into(),
            brightness: 0,
            contrast: 0,
            shadow_tonality: 0,
            highlight_tonality: 0,
            max_optical_density: 0,
            highlight_point_shift: "off".into(),
            wheel_x: 0,
            wheel_y: 0,
        }
    }
}

impl Abw {
    fn options(&self, into: &mut BTreeMap<String, String>) {
        let toning = match self.toning.to_ascii_lowercase().as_str() {
            "cool" => "1",
            "warm" => "2",
            "sepia" => "3",
            _ => "0",
        };
        let tone = match self.tone.to_ascii_lowercase().as_str() {
            "darkest" => "0",
            "darker" => "1",
            "dark" => "2",
            "light" => "4",
            _ => "3",
        };
        let hps = match self.highlight_point_shift.to_ascii_lowercase().as_str() {
            "on" => "2",
            "fullpage" | "full" | "full page" => "1",
            _ => "0",
        };
        let c = |v: i32, lo: i32, hi: i32| v.clamp(lo, hi).to_string();
        into.insert("EPIJ_ClTn".into(), toning.into());
        into.insert("EPIJ_MoGm".into(), tone.into());
        into.insert("EPIJ_Mobr".into(), c(self.brightness, -25, 25));
        into.insert("EPIJ_MoCo".into(), c(self.contrast, -25, 25));
        into.insert("EPIJ_ShTo".into(), c(self.shadow_tonality, -25, 25));
        into.insert("EPIJ_HiTo".into(), c(self.highlight_tonality, -25, 25));
        into.insert("EPIJ_MxOD".into(), c(self.max_optical_density, -50, 0));
        into.insert("EPIJ_HiPS".into(), hps.into());
        into.insert("EPIJ_CirX".into(), c(self.wheel_x, -75, 75));
        into.insert("EPIJ_CirY".into(), c(self.wheel_y, -75, 75));
    }
}

/// What a job asks the driver for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub paper: String,
    pub size: Size,
    #[serde(default)]
    pub quality: Quality,
    /// `None`: color, app managed. `Some`: Advanced B&W Photo.
    #[serde(default)]
    pub abw: Option<Abw>,
    #[serde(default)]
    pub borderless: bool,
    /// Roll: the loaded width in inches (`EPIJ_RoWd`), and whether the job ends with a cut line
    /// (`Cut Sheet`) or runs on as a banner.
    #[serde(default)]
    pub roll_width_in: Option<u32>,
    #[serde(default)]
    pub banner: bool,
}

/// The driver options for a job, as `lp -o key=value` pairs. Errors name the unknown paper.
pub fn options(job: &Job) -> Result<BTreeMap<String, String>, String> {
    let paper = paper(&job.paper).ok_or_else(|| format!("unknown paper `{}` (one of {})", job.paper, PAPERS.iter().map(|p| p.id).collect::<Vec<_>>().join(", ")))?;
    let mut o = BTreeMap::new();
    // media and feed
    o.insert("EPIJ_Medi".into(), paper.media.into());
    o.insert("MediaType".into(), paper.media.into());
    let feed = if job.roll_width_in.is_some() { Feed::Roll } else { paper.feed };
    o.insert("EPIJ_PSrc".into(), if job.borderless { feed.borderless_value(false) } else { feed.value() }.into());
    if let Some(w) = job.roll_width_in {
        o.insert("EPIJ_RoWd".into(), w.clamp(11, 20).to_string());
        o.insert("EPIJ_CutS".into(), if job.banner { "0" } else { "1" }.into());
        o.insert("EPIJ_SRol".into(), "1".into());
    }
    // size
    let (eps, page) = job.size.values();
    if let Some(e) = eps {
        o.insert("EPIJ_Size".into(), e.into());
    }
    o.insert("PageSize".into(), page);
    // quality
    o.insert("EPIJ_Mode".into(), "3".into()); // Advanced
    o.insert("EPIJ_PrStSlID".into(), "1".into()); // Advanced Color Settings panel
    o.insert("EPIJ_PrntMode".into(), "255".into()); // Quality Options
    o.insert("EPIJ_Manu".into(), "0".into());
    match job.quality {
        Quality::Draft => {
            o.insert("EPIJ_Qual".into(), "36".into());
            o.insert("EPIJ_Bi_D".into(), "1".into());
            o.insert("EPIJ_FDet".into(), "0".into());
            o.insert("Resolution".into(), "720x720dpi".into());
        }
        Quality::High => {
            o.insert("EPIJ_Qual".into(), "35".into());
            o.insert("EPIJ_Bi_D".into(), "0".into());
            o.insert("EPIJ_FDet".into(), "1".into());
            o.insert("Resolution".into(), "720x720dpi".into());
        }
        Quality::Max => {
            o.insert("EPIJ_Qual".into(), "52".into());
            o.insert("EPIJ_Bi_D".into(), "0".into());
            o.insert("EPIJ_FDet".into(), "1".into());
            o.insert("Resolution".into(), "720x720dpi".into());
        }
    }
    o.insert("EPIJ_OvCt".into(), if paper.overcoat { "1" } else { "0" }.into());
    o.insert("EPIJ_AnGl".into(), if paper.gloss_smoothing { "1" } else { "0" }.into());
    o.insert("EPIJ_Hori".into(), "0".into());
    o.insert("EPIJ_PPrv".into(), "0".into());
    // color path
    match &job.abw {
        None => {
            o.insert("EPIJ_Ink_".into(), "1".into());
            o.insert("ColorModel".into(), "RGB".into());
            o.insert("EPIJ_CMat".into(), "3".into()); // Off (No Color Adjustment)
            o.insert("EPIJ_OSColMat".into(), "1".into()); // vendor, not ColorSync
            o.insert("EPIJ_OSCMProf".into(), "0".into());
            o.insert("EPIJ_HdofClSp".into(), "0".into());
            o.insert("EPIJ_AFil".into(), "0".into()); // driver sharpening off
            o.insert("EPIJ_ACam".into(), "0".into());
        }
        Some(abw) => {
            o.insert("EPIJ_Ink_".into(), "3".into());
            o.insert("ColorModel".into(), "ABWP".into());
            o.insert("EPIJ_OSColMat".into(), "1".into());
            abw.options(&mut o);
        }
    }
    Ok(o)
}

/// Finds the paper's ICC profile among installed profiles by its hint and "P900" / "SC-P900".
pub fn find_profile(paper: &Paper, installed: &[(std::path::PathBuf, String)]) -> Option<std::path::PathBuf> {
    let hint = paper.profile_hint.to_ascii_lowercase().replace(' ', "");
    let mut candidates: Vec<&(std::path::PathBuf, String)> = installed
        .iter()
        .filter(|(p, n)| {
            let hay = format!("{} {}", n, p.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default()).to_ascii_lowercase().replace(' ', "");
            hay.contains(&hint) && (hay.contains("p900") || hay.contains("sc-p900") || hay.contains("scp900"))
        })
        .collect();
    candidates.sort_by_key(|(p, _)| p.to_string_lossy().len());
    candidates.first().map(|(p, _)| p.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_jobs_turn_the_drivers_color_off() {
        let o = options(&Job { paper: "ultrapro-luster".into(), size: Size::Letter, quality: Quality::Max, abw: None, borderless: false, roll_width_in: None, banner: false }).unwrap();
        assert_eq!(o["EPIJ_Medi"], "1925");
        assert_eq!(o["EPIJ_CMat"], "3");
        assert_eq!(o["ColorModel"], "RGB");
        assert_eq!(o["EPIJ_Qual"], "52");
        assert_eq!(o["EPIJ_Bi_D"], "0");
        assert_eq!(o["EPIJ_OvCt"], "1");
        assert_eq!(o["EPIJ_PSrc"], "2");
        assert_eq!(o["EPIJ_Size"], "4");
        assert!(!o.contains_key("EPIJ_ClTn"));
    }

    #[test]
    fn abw_jobs_send_every_bw_control() {
        let abw = Abw { toning: "warm".into(), tone: "normal".into(), contrast: 5, max_optical_density: -10, highlight_point_shift: "on".into(), ..Default::default() };
        let o = options(&Job { paper: "big-bend-baryta".into(), size: Size::A3Plus, quality: Quality::High, abw: Some(abw), borderless: false, roll_width_in: None, banner: false }).unwrap();
        assert_eq!(o["EPIJ_Ink_"], "3");
        assert_eq!(o["ColorModel"], "ABWP");
        assert_eq!(o["EPIJ_ClTn"], "2");
        assert_eq!(o["EPIJ_MoGm"], "3");
        assert_eq!(o["EPIJ_MoCo"], "5");
        assert_eq!(o["EPIJ_MxOD"], "-10");
        assert_eq!(o["EPIJ_HiPS"], "2");
        assert_eq!(o["EPIJ_Size"], "10");
        assert!(!o.contains_key("EPIJ_CMat"));
    }

    #[test]
    fn rolls_and_custom_sizes() {
        let o = options(&Job { paper: "ultrapro-luster".into(), size: Size::Custom { width_in: 17.0, height_in: 46.0 }, quality: Quality::Max, abw: None, borderless: false, roll_width_in: Some(17), banner: false }).unwrap();
        assert_eq!(o["EPIJ_PSrc"], "49");
        assert_eq!(o["EPIJ_RoWd"], "17");
        assert_eq!(o["EPIJ_CutS"], "1");
        assert_eq!(o["PageSize"], "Custom.1224x3312");
        assert!(options(&Job { paper: "nope".into(), size: Size::Letter, quality: Quality::Max, abw: None, borderless: false, roll_width_in: None, banner: false }).is_err());
        assert_eq!(Size::parse("13x19"), Some(Size::A3Plus));
        assert_eq!(Size::parse("17 x 46").map(|s| s.inches()), Some((17.0, 46.0)));
    }

    #[test]
    fn profiles_are_found_by_hint_and_printer() {
        let installed = vec![
            (std::path::PathBuf::from("/x/RR UltraPro Luster SC-P900.icc"), "RR UltraPro Luster 300 SC-P900".to_string()),
            (std::path::PathBuf::from("/x/RR UltraPro Luster PRO-1000.icc"), "RR UltraPro Luster 300 PRO-1000".to_string()),
        ];
        let p = paper("ultrapro-luster").unwrap();
        assert_eq!(find_profile(p, &installed).unwrap().to_string_lossy(), "/x/RR UltraPro Luster SC-P900.icc");
        assert!(find_profile(paper("big-bend-baryta").unwrap(), &installed).is_none());
    }
}
