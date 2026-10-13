//! IA Darkroom: the Print Sheet dialog (File > Print Sheet…). Paper, size, quality, color or
//! Advanced B&W, the signature strip, then Print (through the native Epson driver) or Save TIFF.
//! It runs the engine's `print.sheet` with these choices.

use egui::Ui;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::LightcraftApp;
use crate::theme::Tokens;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct PrintSheetOptions {
    pub paper: String,
    pub size: String,
    /// draft | high | max
    pub quality: String,
    pub abw: bool,
    pub toning: String,
    pub tone: String,
    pub brightness: i32,
    pub contrast: i32,
    pub shadow_tonality: i32,
    pub highlight_tonality: i32,
    pub max_optical_density: i32,
    pub signature: bool,
    pub caption: String,
    pub margin_in: f32,
    pub strip_in: f32,
    pub ppi: u32,
    pub sharpen: String,
    pub copies: u32,
    /// Save the TIFF instead of printing.
    pub save_only: bool,
    pub save_dir: String,
    /// The papers the engine knows, read once when the dialog opens: (id, name, profile found).
    #[serde(skip)]
    pub papers: Vec<(String, String, bool)>,
}

impl Default for PrintSheetOptions {
    fn default() -> Self {
        Self {
            paper: "ultrapro-luster".into(),
            size: "13x19".into(),
            quality: "max".into(),
            abw: false,
            toning: "neutral".into(),
            tone: "dark".into(),
            brightness: 0,
            contrast: 0,
            shadow_tonality: 0,
            highlight_tonality: 0,
            max_optical_density: 0,
            signature: true,
            caption: "Randy Archambault · The Infinite Arch".into(),
            margin_in: 0.25,
            strip_in: 0.4,
            ppi: 300,
            sharpen: "standard".into(),
            copies: 1,
            save_only: false,
            save_dir: String::new(),
            papers: Vec::new(),
        }
    }
}

impl PrintSheetOptions {
    /// Reads the paper list from the engine.
    pub fn load_papers(&mut self, app: &mut LightcraftApp) {
        if let Ok(v) = app.run("print.papers", json!({}))
            && let Some(items) = v["items"].as_array()
        {
            self.papers = items
                .iter()
                .map(|p| (p["id"].as_str().unwrap_or("").to_string(), p["name"].as_str().unwrap_or("").to_string(), p["profile"].is_string()))
                .collect();
        }
    }

    /// The `print.sheet` parameters.
    pub fn params(&self) -> Value {
        let mut p = json!({
            "paper": self.paper,
            "size": self.size,
            "quality": self.quality,
            "layout": if self.signature { "signature" } else { "plain" },
            "margin": self.margin_in,
            "strip": if self.signature { self.strip_in } else { 0.0 },
            "caption": if self.signature { self.caption.clone() } else { String::new() },
            "ppi": self.ppi,
            "sharpen": self.sharpen,
            "copies": self.copies.max(1),
        });
        if self.abw {
            p["abw"] = json!({
                "toning": self.toning,
                "tone": self.tone,
                "brightness": self.brightness,
                "contrast": self.contrast,
                "shadowTonality": self.shadow_tonality,
                "highlightTonality": self.highlight_tonality,
                "maxOpticalDensity": self.max_optical_density,
            });
        }
        if self.save_only {
            p["print"] = json!(false);
            if !self.save_dir.trim().is_empty() {
                p["out"] = json!(format!("{}/IA Darkroom sheet.tif", self.save_dir.trim_end_matches('/')));
            }
        }
        p
    }
}

fn label_row(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    let t = Tokens::get(ui.ctx());
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(egui::vec2(120.0, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.set_min_width(120.0);
            ui.label(egui::RichText::new(crate::i18n::tr(label)).color(t.text_label));
        });
        add(ui);
    });
}

/// The dialog body.
pub fn draw(app: &mut LightcraftApp, ui: &mut Ui, o: &mut PrintSheetOptions) {
    let t = Tokens::get(ui.ctx());
    if o.papers.is_empty() {
        o.load_papers(app);
    }
    let n = app.session.targets(&json!({})).len();
    let name = app.session.active().and_then(|id| app.session.catalog.photo(id)).map(|p| if p.meta.title.trim().is_empty() { p.file_name.clone() } else { p.meta.title.clone() }).unwrap_or_default();
    ui.label(egui::RichText::new(if n > 1 { format!("{name} (first of {n} selected)") } else { name }).color(t.text_dim));
    ui.add_space(6.0);

    label_row(ui, "Paper", |ui| {
        let current = o.papers.iter().find(|(id, _, _)| *id == o.paper).map(|(_, n, _)| n.clone()).unwrap_or_else(|| o.paper.clone());
        egui::ComboBox::from_id_salt("printPaper").width(300.0).selected_text(current).show_ui(ui, |ui| {
            for (id, name, has_profile) in o.papers.clone() {
                let text = if has_profile { name.clone() } else { format!("{name}  (no P900 profile installed)") };
                ui.selectable_value(&mut o.paper, id, text);
            }
        });
    });
    label_row(ui, "Size", |ui| {
        egui::ComboBox::from_id_salt("printSize").width(140.0).selected_text(o.size.clone()).show_ui(ui, |ui| {
            for s in ["13x19", "17x22", "11x14", "8x10", "letter", "5x7", "a4", "a3", "a2", "12x12", "16x20"] {
                ui.selectable_value(&mut o.size, s.into(), s);
            }
        });
        ui.add_space(8.0);
        ui.label(egui::RichText::new("inches, turned to fit the photo").color(t.text_dim));
    });
    label_row(ui, "Quality", |ui| {
        for (v, l) in [("max", "Max (5760 dpi)"), ("high", "High (1440 dpi)"), ("draft", "Draft (test strips)")] {
            ui.selectable_value(&mut o.quality, v.into(), l);
        }
    });
    label_row(ui, "Sharpen", |ui| {
        for (v, l) in [("low", "Low"), ("standard", "Standard"), ("high", "High")] {
            ui.selectable_value(&mut o.sharpen, v.into(), l);
        }
    });
    ui.add_space(6.0);
    label_row(ui, "Color", |ui| {
        ui.selectable_value(&mut o.abw, false, "Color (IA Darkroom manages color, paper profile)");
        ui.selectable_value(&mut o.abw, true, "Advanced B&W Photo (the printer maps tones)");
    });
    if o.abw {
        ui.indent("abw", |ui| {
            label_row(ui, "Toning", |ui| {
                for v in ["neutral", "cool", "warm", "sepia"] {
                    ui.selectable_value(&mut o.toning, v.into(), v);
                }
            });
            label_row(ui, "Tone", |ui| {
                for v in ["darkest", "darker", "dark", "normal", "light"] {
                    ui.selectable_value(&mut o.tone, v.into(), v);
                }
            });
            label_row(ui, "Brightness", |ui| { ui.add(egui::Slider::new(&mut o.brightness, -25..=25)); });
            label_row(ui, "Contrast", |ui| { ui.add(egui::Slider::new(&mut o.contrast, -25..=25)); });
            label_row(ui, "Shadow tonality", |ui| { ui.add(egui::Slider::new(&mut o.shadow_tonality, -25..=25)); });
            label_row(ui, "Highlight tonality", |ui| { ui.add(egui::Slider::new(&mut o.highlight_tonality, -25..=25)); });
            label_row(ui, "Max optical density", |ui| { ui.add(egui::Slider::new(&mut o.max_optical_density, -50..=0)); });
        });
    }
    ui.add_space(6.0);
    ui.checkbox(&mut o.signature, "Signature strip under the image");
    if o.signature {
        ui.indent("sig", |ui| {
            label_row(ui, "Caption", |ui| { ui.add(egui::TextEdit::singleline(&mut o.caption).desired_width(320.0)); });
            label_row(ui, "Margins (in)", |ui| { ui.add(egui::DragValue::new(&mut o.margin_in).range(0.0..=3.0).speed(0.05)); });
            label_row(ui, "Strip (in)", |ui| { ui.add(egui::DragValue::new(&mut o.strip_in).range(0.1..=2.0).speed(0.05)); });
        });
    } else {
        ui.indent("plain", |ui| {
            label_row(ui, "Margins (in)", |ui| { ui.add(egui::DragValue::new(&mut o.margin_in).range(0.0..=3.0).speed(0.05)); });
        });
    }
    label_row(ui, "Resolution", |ui| {
        for v in [300u32, 360, 720] {
            ui.selectable_value(&mut o.ppi, v, format!("{v} ppi"));
        }
    });
    label_row(ui, "Copies", |ui| { ui.add(egui::DragValue::new(&mut o.copies).range(1..=20)); });
    ui.add_space(6.0);
    ui.checkbox(&mut o.save_only, "Save the 16-bit TIFF instead of printing");
    if o.save_only {
        label_row(ui, "Folder", |ui| {
            ui.add(egui::TextEdit::singleline(&mut o.save_dir).desired_width(320.0).hint_text("default: the print folder in Application Support"));
        });
    }
    ui.add_space(4.0);
    ui.label(egui::RichText::new("Native Epson driver only. The sheet is converted into the paper's profile before it leaves IA Darkroom; the driver's own color is off.").color(t.text_dim));
}
