//! Font loading: a configured override path, else the bundled file in `assets/fonts`, else a
//! system family via fontdb. A slot with none of the three is an error that names the file.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context};
use femtovg::renderer::OpenGl;
use femtovg::{Canvas, FontId};

use crate::config::FontOverrides;

#[derive(Debug, Clone, Copy)]
pub struct Fonts {
    /// Oswald Medium: display caps.
    pub display: FontId,
    /// Oswald Bold: headers.
    pub display_bold: FontId,
    /// Barlow Condensed SemiBold: secondary labels.
    pub label: FontId,
    /// Fira Code Medium: data readouts.
    pub mono: FontId,
    /// Noto Serif JP Black (subset): Mincho kanji, also the glyph fallback for the others.
    pub mincho: FontId,
    /// DSEG7 Classic Bold Italic: 7-segment timer and clocks.
    pub segment: FontId,
}

impl Fonts {
    /// Font lists with the Mincho fallback appended, so kanji inside Latin strings render.
    pub fn display(&self) -> [FontId; 2] {
        [self.display, self.mincho]
    }
    pub fn bold(&self) -> [FontId; 2] {
        [self.display_bold, self.mincho]
    }
    pub fn label(&self) -> [FontId; 2] {
        [self.label, self.mincho]
    }
    pub fn mono(&self) -> [FontId; 2] {
        [self.mono, self.mincho]
    }
    pub fn mincho(&self) -> [FontId; 2] {
        [self.mincho, self.display]
    }
    pub fn segment(&self) -> [FontId; 1] {
        [self.segment]
    }
}

struct Slot<'a> {
    file: &'static str,
    override_path: Option<&'a PathBuf>,
    families: &'static [&'static str],
}

pub fn load(canvas: &mut Canvas<OpenGl>, cfg: &FontOverrides, dir: &Path) -> anyhow::Result<Fonts> {
    let mut system: Option<fontdb::Database> = None;
    let mut one = |slot: Slot| -> anyhow::Result<FontId> {
        if let Some(p) = slot.override_path {
            return canvas.add_font(p).with_context(|| format!("font override {}", p.display()));
        }
        let bundled = dir.join(slot.file);
        if bundled.is_file() {
            return canvas.add_font(&bundled).with_context(|| format!("bundled font {}", bundled.display()));
        }
        let db = system.get_or_insert_with(|| {
            let mut db = fontdb::Database::new();
            db.load_system_fonts();
            db
        });
        for family in slot.families {
            let q = fontdb::Query { families: &[fontdb::Family::Name(family)], ..Default::default() };
            if let Some(id) = db.query(&q) {
                if let Some(Ok(font)) = db.with_face_data(id, |data, _index| canvas.add_font_mem(data)) {
                    eprintln!("nerv: {} missing, using system font {family}", bundled.display());
                    return Ok(font);
                }
            }
        }
        Err(anyhow!("font {} not found and no system fallback ({})", bundled.display(), slot.families.join(", ")))
    };
    const SANS: &[&str] = &["Nimbus Sans Narrow", "Liberation Sans Narrow", "DejaVu Sans Condensed"];
    const MONO: &[&str] = &["FiraCode Nerd Font", "Fira Code", "JetBrainsMono Nerd Font", "DejaVu Sans Mono"];
    Ok(Fonts {
        display: one(Slot { file: "Oswald-Medium.ttf", override_path: cfg.display.as_ref(), families: SANS })?,
        display_bold: one(Slot { file: "Oswald-Bold.ttf", override_path: cfg.display_bold.as_ref(), families: SANS })?,
        label: one(Slot { file: "BarlowCondensed-SemiBold.ttf", override_path: cfg.label.as_ref(), families: SANS })?,
        mono: one(Slot { file: "FiraCode-Medium.ttf", override_path: cfg.mono.as_ref(), families: MONO })?,
        mincho: one(Slot {
            file: "NotoSerifJP-Black-subset.ttf",
            override_path: cfg.mincho.as_ref(),
            families: &["Noto Serif CJK JP", "Noto Serif JP", "Noto Sans CJK JP"],
        })?,
        segment: one(Slot {
            file: "DSEG7Classic-BoldItalic.ttf",
            override_path: cfg.segment.as_ref(),
            families: MONO,
        })?,
    })
}
