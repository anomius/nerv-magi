//! The TV panels. Each draws inside its layout rect in design units (3840×2160).

pub mod alert;
pub mod header;
pub mod jobs;
pub mod magi;
pub mod plate;
pub mod procs;
pub mod sdat;
pub mod sectors;
pub mod telemetry;
pub mod ticker;
pub mod timer;
pub mod units;

use crate::config::Config;
use crate::state::{AgentStatus, State};

use super::anim::Anim;
use super::fonts::Fonts;
use super::gfx::{Cv, Palette};
use super::scene::Scene;

pub struct Ctx<'a> {
    pub canvas: &'a mut Cv,
    pub fonts: &'a Fonts,
    pub state: &'a State,
    pub config: &'a Config,
    /// Seconds since the view started.
    pub t: f32,
    pub anim: &'a mut Anim,
    pub scene: SceneInfo,
    /// Current scanline look (runtime state: `s` / SIGUSR1 cycle it).
    pub scanlines: crate::config::ScanlineMode,
    /// Text of the transient toast, while it shows.
    pub toast: Option<&'a str>,
    /// Which layer this pass draws (see tv::layers).
    pub layer: crate::tv::layers::Layer,
    pub fx: &'a mut crate::tv::fx::Images,
    /// Pre-rendered glyphs for live 7-segment readouts.
    pub sprites: &'a mut crate::tv::sprites::Cache,
    /// The `?` help overlay is up.
    pub help: bool,
    /// Film grain on (runtime switch).
    pub grain: bool,
}

/// What the Director decided for this frame, for views that show it.
#[derive(Debug, Clone, Copy)]
pub struct SceneInfo {
    pub scene: Scene,
    /// Seconds since this scene was cut to.
    pub since: f32,
    /// 0..1 progress of the cut into it.
    pub cut: f32,
    /// While rotating: next scene and seconds until it.
    pub countdown: Option<(Scene, f32)>,
    /// Seconds herdr has been unreachable.
    pub lost_for: Option<f32>,
    /// Rotation or pinned view (status strip label).
    pub mode: crate::control::ViewMode,
    /// EMERGENCY pinned with no real emergency: draw it as a preview.
    pub preview: bool,
}

pub fn status_color(s: AgentStatus) -> femtovg::Color {
    match s {
        AgentStatus::Blocked => Palette::RED,
        AgentStatus::Working => Palette::GREEN,
        AgentStatus::Done => Palette::CYAN,
        AgentStatus::Idle => Palette::DIM,
        AgentStatus::Unknown => Palette::AMBER,
    }
}
