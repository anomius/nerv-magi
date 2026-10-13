//! Live data collectors. Each `spawn` starts a thread that sends `Update`s until the receiver is
//! dropped. Collectors must never panic on missing files/tools: they send `SourceOffline` instead.

pub mod audio;
pub mod herdr;
pub mod media;
pub mod power;
pub mod proc;
pub mod procs;
pub mod spectrum;
pub mod sys;
pub mod usage;

use std::sync::mpsc::Sender;

use crate::config::Config;
use crate::state::Update;

/// Which optional collectors to run. The hub view skips the spectrum (cava) to save CPU.
#[derive(Debug, Clone, Copy)]
pub struct Which {
    pub spectrum: bool,
}

pub fn spawn_all(tx: Sender<Update>, which: Which, config: &Config) {
    herdr::spawn(tx.clone());
    sys::spawn(tx.clone());
    power::spawn(tx.clone());
    procs::spawn(tx.clone());
    usage::spawn(tx.clone());
    audio::spawn(tx.clone());
    media::spawn(tx.clone());
    if which.spectrum {
        spectrum::spawn(tx, config.spectrum.bars, config.spectrum.framerate);
    }
}
