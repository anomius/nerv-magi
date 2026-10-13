//! Headless render to PNG: an EGL device display (no window, no Wayland connection), a context
//! made current without a surface, and a femtovg image as the render target. Used by agents and
//! tests to look at the TV screen. There is deliberately no hidden-window fallback: it would flash
//! on the user's screen.

use std::fs::File;
use std::io::BufWriter;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context as _};
use femtovg::renderer::OpenGl;
use femtovg::{Canvas, ImageFlags, PixelFormat, RenderTarget};
use glutin::api::egl::context::PossiblyCurrentContext;
use glutin::api::egl::device::Device;
use glutin::api::egl::display::Display;
use glutin::config::{ConfigSurfaceTypes, ConfigTemplateBuilder};
use glutin::context::{ContextApi, ContextAttributesBuilder};
use glutin::prelude::*;

use crate::cli::TvArgs;
use crate::config::Config;

use super::View;

/// Keeps the EGL display and context alive for as long as the canvas renders.
pub(super) struct Headless {
    _context: PossiblyCurrentContext,
    display: Display,
}

fn headless_on(device: &Device) -> anyhow::Result<Headless> {
    let display = unsafe { Display::with_device(device, None) }.map_err(|e| anyhow!("EGL display: {e}"))?;
    let template =
        ConfigTemplateBuilder::new().with_surface_type(ConfigSurfaceTypes::empty()).with_alpha_size(8).build();
    let config = unsafe { display.find_configs(template) }
        .map_err(|e| anyhow!("EGL configs: {e}"))?
        .next()
        .ok_or_else(|| anyhow!("no surfaceless EGL config"))?;
    let mut last = None;
    for api in [ContextApi::OpenGl(None), ContextApi::Gles(None)] {
        let attrs = ContextAttributesBuilder::new().with_context_api(api).build(None);
        match unsafe { display.create_context(&config, &attrs) } {
            Ok(ctx) => {
                let ctx = ctx.make_current_surfaceless().map_err(|e| anyhow!("make current (surfaceless): {e}"))?;
                return Ok(Headless { _context: ctx, display });
            }
            Err(e) => last = Some(e),
        }
    }
    Err(anyhow!("EGL context: {}", last.map(|e| e.to_string()).unwrap_or_default()))
}

/// Hardware devices (with a DRM render node) first, then software ones.
fn headless() -> anyhow::Result<Headless> {
    let mut devices: Vec<Device> = Device::query_devices().map_err(|e| anyhow!("EGL device query: {e}"))?.collect();
    devices.sort_by_key(|d| d.drm_render_device_node_path().is_none());
    let mut errors = Vec::new();
    for d in &devices {
        match headless_on(d) {
            Ok(h) => return Ok(h),
            Err(e) => errors.push(format!("{}: {e}", d.name().unwrap_or("?"))),
        }
    }
    bail!("no headless EGL device worked ({} tried): {}", devices.len(), errors.join("; "))
}

/// A headless GL context, a canvas of `w`×`h` and an offscreen image to render into.
pub(super) fn offscreen(w: u32, h: u32) -> anyhow::Result<(Headless, Canvas<OpenGl>, femtovg::ImageId)> {
    let gl = headless()?;
    let renderer = OpenGl::new_from_glutin_display(&gl.display).map_err(|e| anyhow!("femtovg: {e:?}"))?;
    let mut canvas = Canvas::new(renderer).map_err(|e| anyhow!("canvas: {e:?}"))?;
    canvas.set_size(w, h, 1.0);
    let target = canvas
        .create_image_empty(w as usize, h as usize, PixelFormat::Rgba8, ImageFlags::empty())
        .map_err(|e| anyhow!("offscreen image {w}x{h}: {e:?}"))?;
    Ok((gl, canvas, target))
}

pub fn render_png(args: &TvArgs, config: Config) -> anyhow::Result<()> {
    let path = args.snapshot.as_ref().ok_or_else(|| anyhow!("--snapshot path missing"))?;
    let (w, h) = args.size;
    let (_gl, canvas, target) = offscreen(w, h)?;

    let mut view = View::new(canvas, config, args)?;
    // Frames are paced in real time so the data feed (which runs on wall-clock time) and the
    // animations agree: --frames 1500 shows the demo at ~25 s.
    let start = Instant::now();
    let frames = args.frames.max(1);
    let mut drawn = 0u32;
    for i in 0..frames {
        let due = start + Duration::from_secs_f64(i as f64 / 60.0);
        let now = Instant::now();
        if let Some(wait) = due.checked_duration_since(now) {
            std::thread::sleep(wait);
        }
        let t = i as f32 / 60.0;
        // behind schedule by more than a frame: advance the state but skip drawing (the last
        // frame is always drawn)
        let late = now.saturating_duration_since(due) > Duration::from_millis(17);
        if late && i + 1 < frames {
            view.update(t);
            continue;
        }
        view.frame(w, h, t, RenderTarget::Image(target));
        drawn += 1;
    }
    let img = view.canvas.screenshot().map_err(|e| anyhow!("read pixels: {e:?}"))?;
    if img.width() != w as usize || img.height() != h as usize {
        bail!("readback is {}x{}, expected {w}x{h}", img.width(), img.height());
    }

    let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
    let mut enc = png::Encoder::new(BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::Fast);
    let rgb: Vec<u8> = img.buf().iter().flat_map(|p| [p.r, p.g, p.b]).collect();
    enc.write_header()?.write_image_data(&rgb)?;
    eprintln!(
        "nerv: wrote {} ({w}x{h}, {} frames, {drawn} drawn, {:.1} s)",
        path.display(),
        frames,
        start.elapsed().as_secs_f32()
    );
    Ok(())
}
