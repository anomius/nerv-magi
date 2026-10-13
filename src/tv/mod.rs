//! `nerv tv`: fullscreen GPU window (winit + glutin + femtovg), or a headless PNG snapshot.

pub mod anim;
pub mod bench;
pub mod fonts;
pub mod fx;
pub mod gfx;
pub mod input;
pub mod layers;
pub mod layout;
pub mod panels;
pub mod record;
pub mod scene;
pub mod snapshot;
pub mod sprites;
pub mod usage;
pub mod views;

use std::fs;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant, SystemTime};

use anyhow::anyhow;
use femtovg::renderer::OpenGl;
use femtovg::{Canvas, ImageFlags, PixelFormat, RenderTarget};
use glutin::config::{Config as GlutinConfig, ConfigTemplateBuilder};
use glutin::context::{ContextAttributesBuilder, PossiblyCurrentContext};
use glutin::display::GetGlDisplay;
use glutin::prelude::*;
use glutin::surface::{Surface, SwapInterval, WindowSurface};
use glutin_winit::{DisplayBuilder, GlWindow};
use raw_window_handle::HasWindowHandle;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::wayland::WindowAttributesExtWayland;
use winit::window::{Fullscreen, Window, WindowId};

use crate::cli::TvArgs;
use crate::collect::Which;
use crate::config::{asset_dir, Config, ScanlineMode};
use crate::control::{self, TvCommand, ViewMode, ViewName};
use crate::state::{drain_into, PlayStatus, State, Update};

use self::anim::Anim;
use self::fonts::Fonts;
use self::gfx::{Cv, Palette};
use self::layers::Layer;
use self::layout::Layout;
use self::panels::{Ctx, SceneInfo};
use self::scene::{Director, Scene};

/// Everything a frame needs, independent of where it is presented (window or offscreen image).
pub struct View {
    pub canvas: Cv,
    fonts: Fonts,
    pub state: State,
    rx: Receiver<Update>,
    config: Config,
    anim: Anim,
    director: Director,
    /// Scanline look (starts from --scanlines or the config; cycled at runtime).
    scanlines: ScanlineMode,
    toast: Option<(String, f32)>,
    help: input::Help,
    grain: fx::Grain,
    fx: fx::Images,
    sprites: sprites::Cache,
    /// Cached base layer (MAIN / LOAD / SYNC): image, its size, when and for which scene it was drawn.
    base: Option<BaseImage>,
    /// Per-phase render CPU (only `--bench` sets it).
    pub prof: Option<bench::Prof>,
    t: f32,
}

struct BaseImage {
    image: femtovg::ImageId,
    size: (u32, u32),
    at: f32,
    scene: Scene,
    dirty: bool,
    /// Data the base was drawn from (a change forces a re-render, see layers::Epoch).
    epoch: Option<layers::Epoch>,
}

impl View {
    pub fn new(mut canvas: Canvas<OpenGl>, config: Config, args: &TvArgs) -> anyhow::Result<Self> {
        let fonts = fonts::load(&mut canvas, &config.fonts, &asset_dir().join("fonts"))?;
        let rx = crate::start_feed(args.demo, Which { spectrum: true }, &config);
        let mut director = Director::new(0.0);
        // --scene <view> starts pinned on it; BOOT is not a pickable view, so it stays forced
        match args.scene.as_deref() {
            Some("boot") => director.forced = Some(Scene::Boot { reconnecting: false }),
            Some(name) => {
                if let Some(v) = ViewName::parse(name) {
                    director.set_mode(ViewMode::Pinned(v));
                }
            }
            None => {}
        }
        let scanlines = fx::effective_mode(args.scanlines, &config.tv);
        let grain = fx::Grain::new(config.tv.grain);
        let mut view = Self {
            canvas,
            fonts,
            state: State::new(SystemTime::now()),
            rx,
            config,
            anim: Anim::new(0.0),
            director,
            scanlines,
            toast: None,
            help: input::Help::default(),
            grain,
            fx: fx::Images::default(),
            sprites: sprites::Cache::default(),
            base: None,
            prof: None,
            t: 0.0,
        };
        for cmd in args.cmds.iter().filter_map(|w| TvCommand::parse(w)) {
            view.command(cmd);
        }
        Ok(view)
    }

    /// Next scanline look, with a 2 s toast.
    pub fn cycle_scanlines(&mut self) {
        self.scanlines = self.scanlines.next();
        self.show_toast(fx::toast_text(self.scanlines));
    }

    fn show_toast(&mut self, text: String) {
        self.toast = Some((text, self.t + TOAST_S));
        if let Some(b) = self.base.as_mut() {
            b.dirty = true;
        }
    }

    /// A key or remote command that changes what the view shows. Fullscreen and quit belong to
    /// the window and are ignored here.
    pub fn command(&mut self, cmd: TvCommand) {
        match cmd {
            TvCommand::Scanlines => self.cycle_scanlines(),
            TvCommand::ToggleHelp => self.help.toggle(self.t),
            TvCommand::ToggleGrain => {
                let text = self.grain.toggle();
                self.show_toast(text);
            }
            TvCommand::ToggleFullscreen | TvCommand::Quit => {}
            TvCommand::View(_) | TvCommand::Next | TvCommand::Prev | TvCommand::Auto | TvCommand::TogglePin => {
                let mode = self.director.mode().apply(cmd, self.director.current().view_name());
                self.director.set_mode(mode);
                self.show_toast(input::view_toast(mode));
            }
        }
    }

    /// Apply pending updates without drawing (a snapshot catching up with the wall clock).
    pub fn update(&mut self, t: f32) {
        self.t = t;
        drain_into(&self.rx, &mut self.state);
        self.state.tick(SystemTime::now());
    }

    /// Apply pending updates and draw one frame of `w`×`h` physical pixels at `t` seconds into
    /// `target` (the window, or the snapshot image). MAIN / LOAD / SYNC reuse a cached base layer.
    pub fn frame(&mut self, w: u32, h: u32, t: f32, target: RenderTarget) {
        self.update(t);
        let layout = Layout::new(w as f32, h as f32);
        let scene = self.director.scene(&self.state, &self.config, t);
        let info = SceneInfo {
            scene,
            since: t - self.director.entered_at(),
            cut: self.director.transition(t),
            countdown: self.director.countdown(&self.config, t),
            lost_for: self.director.lost_since().map(|s| t - s),
            mode: self.director.mode(),
            preview: self.director.preview(),
        };
        if self.toast.as_ref().is_some_and(|(_, until)| t >= *until) {
            self.toast = None;
        }
        gfx::set_bloom(if self.scanlines == ScanlineMode::Crt { 1.3 } else { 1.0 });
        let layered = layers::layered(scene, info.cut, self.anim.busy(t));

        if layered {
            let size_changed = self.base.as_ref().is_none_or(|b| b.size != (w, h));
            if size_changed {
                if let Some(old) = self.base.take() {
                    self.canvas.delete_image(old.image);
                }
                match self.canvas.create_image_empty(w as usize, h as usize, PixelFormat::Rgba8, ImageFlags::FLIP_Y) {
                    Ok(image) => {
                        self.base = Some(BaseImage {
                            image,
                            size: (w, h),
                            at: f32::NEG_INFINITY,
                            scene,
                            dirty: true,
                            epoch: None,
                        })
                    }
                    Err(e) => eprintln!("nerv: base layer image {w}x{h}: {e:?}"),
                }
            }
        }
        let epoch = layers::epoch(&self.state, info.preview);
        match self.base.as_ref().filter(|_| layered).map(|b| {
            (b.image, layers::base_stale(t - b.at, b.scene != scene, false, b.dirty || b.epoch != Some(epoch)))
        }) {
            Some((image, stale)) => {
                if stale {
                    self.canvas.set_render_target(RenderTarget::Image(image));
                    self.canvas.reset_transform();
                    self.canvas.clear_rect(0, 0, w, h, Palette::BLACK);
                    let c0 = bench::thread_cpu();
                    self.draw_pass(&layout, Layer::Base, info, RenderTarget::Image(image));
                    if let Some(p) = self.prof.as_mut() {
                        p.add("base pass", c0);
                    }
                    if let Some(b) = self.base.as_mut() {
                        b.at = t;
                        b.scene = scene;
                        b.dirty = false;
                        b.epoch = Some(epoch);
                    }
                }
                self.canvas.set_render_target(target);
                self.canvas.reset_transform();
                let mut all = femtovg::Path::new();
                all.rect(0.0, 0.0, w as f32, h as f32);
                self.canvas.fill_path(&all, &femtovg::Paint::image(image, 0.0, 0.0, w as f32, h as f32, 0.0, 1.0));
                let c0 = bench::thread_cpu();
                self.draw_pass(&layout, Layer::Live, info, target);
                if let Some(p) = self.prof.as_mut() {
                    p.add("live pass", c0);
                }
            }
            None => {
                self.canvas.set_render_target(target);
                self.canvas.reset_transform();
                self.canvas.clear_rect(0, 0, w, h, Palette::BLACK);
                let c0 = bench::thread_cpu();
                self.draw_pass(&layout, Layer::All, info, target);
                if let Some(p) = self.prof.as_mut() {
                    p.add("full pass", c0);
                }
            }
        }
        let c0 = bench::thread_cpu();
        self.canvas.flush();
        if let Some(p) = self.prof.as_mut() {
            p.add("flush", c0);
        }
    }

    /// One pass of the view in design units.
    fn draw_pass(&mut self, layout: &Layout, layer: Layer, info: SceneInfo, target: RenderTarget) {
        let toast = self.toast.as_ref().map(|(s, _)| s.as_str());
        self.sprites.scale = layout.scale;
        self.sprites.target = target;
        let canvas = &mut self.canvas;
        canvas.save();
        canvas.translate(layout.origin.0, layout.origin.1);
        canvas.scale(layout.scale, layout.scale);
        let mut ctx = Ctx {
            canvas,
            fonts: &self.fonts,
            state: &self.state,
            config: &self.config,
            t: self.t,
            anim: &mut self.anim,
            scene: info,
            scanlines: self.scanlines,
            toast,
            layer,
            fx: &mut self.fx,
            sprites: &mut self.sprites,
            help: self.help.visible(self.t),
            grain: self.grain.on,
        };
        views::draw(&mut ctx, layout);
        self.canvas.restore();
    }

    /// Name of the scene on screen (bench reports).
    pub fn scene_name(&self) -> &'static str {
        self.director.current().name()
    }

    /// Fast motion that needs full frame rate: cuts, the blocked flash, MAGI flicker and trace
    /// pulses, the BOOT view.
    fn fast(&self) -> bool {
        matches!(self.director.current(), Scene::Boot { .. })
            || self.director.transition(self.t) < 1.0
            || self.anim.busy(self.t)
    }

    /// Ambient motion: anything not MAIN, a blocked agent, music, the toast, rolling scanlines.
    fn ambient(&self) -> bool {
        !matches!(self.director.current(), Scene::Main)
            || self.toast.is_some()
            || self.grain.on
            || self.state.counts().blocked > 0
            || self.state.media.as_ref().is_some_and(|m| m.status == PlayStatus::Playing)
            || (self.scanlines != ScanlineMode::Off && (self.config.tv.scanline_roll || self.config.tv.crt_flicker))
    }

    fn frame_interval(&self) -> Duration {
        let fps = pace(self.fast(), self.ambient(), &self.config.tv);
        Duration::from_secs_f64(1.0 / fps.max(1) as f64)
    }
}

pub const AMBIENT_FPS: u32 = 24;

/// Frames per second for this frame: `fast` motion → fps_active; ambient → min(fps_active,
/// AMBIENT_FPS); otherwise fps_calm.
pub fn pace(fast: bool, ambient: bool, tv: &crate::config::TvConfig) -> u32 {
    if fast {
        tv.fps_active
    } else if ambient {
        tv.fps_active.min(AMBIENT_FPS)
    } else {
        tv.fps_calm
    }
}

/// Large allocations (≥ 1 MiB) straight from mmap, so they go back to the system when freed.
/// femtovg uploads every new texture from a zero-filled CPU buffer (33 MB for a 4K base layer);
/// with glibc's dynamic threshold those land on the heap and stay resident after `free`
/// (measured: 4K RSS 124 → 85 MB).
fn return_large_frees() {
    #[cfg(target_env = "gnu")]
    unsafe {
        libc::mallopt(libc::M_MMAP_THRESHOLD, 1 << 20);
        libc::mallopt(libc::M_TRIM_THRESHOLD, 1 << 20);
    }
}

pub fn run(args: TvArgs) -> anyhow::Result<()> {
    return_large_frees();
    let config = Config::load()?;
    if let Some(secs) = args.bench {
        return bench::run(&args, config, secs);
    }
    if args.record.is_some() {
        return record::run(&args, config);
    }
    if args.snapshot.is_some() {
        return snapshot::render_png(&args, config);
    }
    // Handlers first: once the pidfile exists, tv-ctl may signal us at any moment.
    install_signal_handlers();
    let Some(_pid) = PidFile::acquire()? else { return Ok(()) };
    let event_loop = EventLoop::new()?;
    let mut app = App {
        args,
        config: Some(config),
        gl: None,
        next_frame: Instant::now(),
        start: Instant::now(),
        error: None,
        shift: false,
        last_move: Instant::now(),
        cursor: input::CursorLatch::default(),
        keys: input::KeyGate::default(),
    };
    event_loop.run_app(&mut app)?;
    match app.error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

// ---------------------------------------------------------------------------------------------
// single instance + clean shutdown

static TERMINATE: AtomicBool = AtomicBool::new(false);
/// SIGUSR1 (herdr action `tv-scanlines`): cycle the scanline look.
static CYCLE_SCANLINES: AtomicBool = AtomicBool::new(false);
/// SIGUSR2 (`scripts/tv-ctl.sh`): commands are waiting in `control::cmd_path()`.
static REMOTE: AtomicBool = AtomicBool::new(false);

/// How long the SCANLINES toast shows.
const TOAST_S: f32 = 2.0;

extern "C" fn on_signal(_: libc::c_int) {
    TERMINATE.store(true, Ordering::Relaxed);
}

extern "C" fn on_usr1(_: libc::c_int) {
    CYCLE_SCANLINES.store(true, Ordering::Relaxed);
}

extern "C" fn on_usr2(_: libc::c_int) {
    REMOTE.store(true, Ordering::Relaxed);
}

fn install_signal_handlers() {
    let handler = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
    let usr1 = on_usr1 as extern "C" fn(libc::c_int) as libc::sighandler_t;
    let usr2 = on_usr2 as extern "C" fn(libc::c_int) as libc::sighandler_t;
    unsafe {
        libc::signal(libc::SIGTERM, handler);
        libc::signal(libc::SIGINT, handler);
        libc::signal(libc::SIGUSR1, usr1);
        libc::signal(libc::SIGUSR2, usr2);
    }
}

/// `<control::runtime_dir()>/nerv-tv.pid`, removed on drop.
struct PidFile(PathBuf);

impl PidFile {
    fn path() -> PathBuf {
        control::runtime_dir().join("nerv-tv.pid")
    }

    /// `None` when another live `nerv tv` holds the pidfile.
    fn acquire() -> anyhow::Result<Option<Self>> {
        let path = Self::path();
        if let Some(pid) = fs::read_to_string(&path).ok().and_then(|s| s.trim().parse::<i32>().ok()) {
            if pid_is_nerv(pid) && pid != std::process::id() as i32 {
                println!("nerv tv already running (pid {pid})");
                return Ok(None);
            }
        }
        fs::write(&path, format!("{}\n", std::process::id())).map_err(|e| anyhow!("{}: {e}", path.display()))?;
        Ok(Some(Self(path)))
    }
}

impl Drop for PidFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn pid_is_nerv(pid: i32) -> bool {
    pid > 0
        && unsafe { libc::kill(pid, 0) } == 0
        && fs::read_to_string(format!("/proc/{pid}/comm")).is_ok_and(|c| c.trim() == "nerv")
}

// ---------------------------------------------------------------------------------------------
// window

struct Gl {
    // drop order: the canvas (inside view) before the context and surface
    view: View,
    surface: Surface<WindowSurface>,
    context: PossiblyCurrentContext,
    window: Window,
}

struct App {
    args: TvArgs,
    config: Option<Config>,
    gl: Option<Gl>,
    next_frame: Instant,
    start: Instant,
    error: Option<anyhow::Error>,
    /// Shift held (Shift+Tab = previous view).
    shift: bool,
    /// Last pointer movement over the window (pointer auto-hide when fullscreen).
    last_move: Instant,
    cursor: input::CursorLatch,
    /// Drops key presses right after the window appears or regains focus.
    keys: input::KeyGate,
}

/// Window size when leaving fullscreen.
const WINDOWED_SIZE: winit::dpi::LogicalSize<f64> = winit::dpi::LogicalSize::new(1600.0, 900.0);

/// The window: always with the windowed size (on Wayland, winit drops a resize requested while
/// still fullscreen and un-fullscreens to the creation size, else its 800×600 default when
/// floating), fullscreen on top when starting fullscreen.
fn window_attributes(start_fullscreen: bool) -> winit::window::WindowAttributes {
    let attrs = Window::default_attributes()
        .with_title("NERV · MAGI")
        .with_name("nerv-tv", "nerv-tv")
        .with_inner_size(WINDOWED_SIZE);
    if start_fullscreen {
        attrs.with_fullscreen(Some(Fullscreen::Borderless(None)))
    } else {
        attrs
    }
}

impl App {
    fn create(&mut self, el: &ActiveEventLoop) -> anyhow::Result<Gl> {
        let start_fullscreen = !self.args.windowed && self.config.as_ref().is_none_or(|c| c.tv.start_fullscreen);
        let attrs = window_attributes(start_fullscreen);
        let template = ConfigTemplateBuilder::new().with_alpha_size(8).with_stencil_size(8);
        let (window, gl_config) = DisplayBuilder::new()
            .with_window_attributes(Some(attrs))
            .build(el, template, pick_config)
            .map_err(|e| anyhow!("GL display: {e}"))?;
        let window = window.ok_or_else(|| anyhow!("window was not created"))?;
        let raw = window.window_handle()?.as_raw();
        let display = gl_config.display();
        let ctx_attrs = ContextAttributesBuilder::new().build(Some(raw));
        let not_current = unsafe { display.create_context(&gl_config, &ctx_attrs) }?;
        let surface_attrs = window.build_surface_attributes(Default::default())?;
        let surface = unsafe { display.create_window_surface(&gl_config, &surface_attrs) }?;
        let context = not_current.make_current(&surface)?;
        // never block in swap_buffers: frames are paced by about_to_wait's WaitUntil, and a vsync
        // wait stalls the event loop (SIGTERM / SIGUSR2 handling included) for as long as the
        // compositor sends no frame callbacks, e.g. while the window sits on a hidden workspace
        let _ = surface.set_swap_interval(&context, SwapInterval::DontWait);
        let renderer = OpenGl::new_from_glutin_display(&display).map_err(|e| anyhow!("femtovg: {e:?}"))?;
        let mut canvas = Canvas::new(renderer).map_err(|e| anyhow!("canvas: {e:?}"))?;
        let size = window.inner_size();
        canvas.set_size(size.width.max(1), size.height.max(1), 1.0);
        let config = self.config.take().unwrap_or_default();
        let view = View::new(canvas, config, &self.args)?;
        self.start = Instant::now();
        self.keys.focused(0.0);
        Ok(Gl { view, surface, context, window })
    }

    fn fail(&mut self, el: &ActiveEventLoop, e: anyhow::Error) {
        self.error = Some(e);
        el.exit();
    }

    /// One key or remote command: the window handles fullscreen and quit, the view the rest.
    fn dispatch(&mut self, el: &ActiveEventLoop, cmd: TvCommand) {
        let Some(gl) = self.gl.as_mut() else { return };
        match cmd {
            TvCommand::Quit => el.exit(),
            TvCommand::ToggleFullscreen => {
                if gl.window.fullscreen().is_some() {
                    gl.window.set_fullscreen(None);
                    let _ = gl.window.request_inner_size(WINDOWED_SIZE);
                } else {
                    gl.window.set_fullscreen(Some(Fullscreen::Borderless(None)));
                }
            }
            _ => gl.view.command(cmd),
        }
        gl.window.request_redraw();
    }
}

fn pick_config(configs: Box<dyn Iterator<Item = GlutinConfig> + '_>) -> GlutinConfig {
    configs
        .reduce(|best, c| {
            let better = c.stencil_size() >= 8 && (best.stencil_size() < 8 || c.num_samples() < best.num_samples());
            if better {
                c
            } else {
                best
            }
        })
        .expect("no GL config")
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gl.is_none() {
            match self.create(el) {
                Ok(gl) => {
                    gl.window.request_redraw();
                    self.gl = Some(gl);
                }
                Err(e) => self.fail(el, e),
            }
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(gl) = self.gl.as_mut() else { return };
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::ModifiersChanged(m) => self.shift = m.state().shift_key(),
            WindowEvent::CursorMoved { .. } => self.last_move = Instant::now(),
            WindowEvent::Focused(true) => self.keys.focused(self.start.elapsed().as_secs_f32()),
            // synthetic presses (X11: keys already held at focus) are never meant for us; on Wayland
            // stray typing arrives as real presses, which the focus grace period drops
            WindowEvent::KeyboardInput {
                event: KeyEvent { logical_key, state: ElementState::Pressed, repeat, .. },
                is_synthetic: false,
                ..
            } => {
                if !self.keys.allows(self.start.elapsed().as_secs_f32()) {
                    return;
                }
                let cmd = input::key_name(&logical_key).and_then(|k| control::key_command(&k, self.shift));
                if let Some(cmd) = cmd.filter(|c| !(repeat && input::repeat_ignored(*c))) {
                    self.dispatch(el, cmd);
                }
            }
            WindowEvent::Resized(size) => {
                if let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) {
                    gl.surface.resize(&gl.context, w, h);
                    gl.view.canvas.set_size(w.get(), h.get(), 1.0);
                    gl.window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                let size = gl.window.inner_size();
                gl.view.frame(
                    size.width.max(1),
                    size.height.max(1),
                    self.start.elapsed().as_secs_f32(),
                    RenderTarget::Screen,
                );
                if let Err(e) = gl.surface.swap_buffers(&gl.context) {
                    self.fail(el, anyhow!("swap buffers: {e}"));
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if TERMINATE.load(Ordering::Relaxed) {
            el.exit();
            return;
        }
        if REMOTE.swap(false, Ordering::Relaxed) {
            for cmd in control::take_commands(&control::cmd_path()) {
                self.dispatch(el, cmd);
            }
        }
        let Some(gl) = self.gl.as_mut() else { return };
        if CYCLE_SCANLINES.swap(false, Ordering::Relaxed) {
            gl.view.cycle_scanlines();
            gl.window.request_redraw();
        }
        let tv = &gl.view.config.tv;
        let want = tv.cursor.visible(
            gl.window.fullscreen().is_some(),
            self.last_move.elapsed().as_secs_f32(),
            tv.cursor_hide_secs,
        );
        if let Some(v) = self.cursor.update(want) {
            gl.window.set_cursor_visible(v);
        }
        let now = Instant::now();
        if now >= self.next_frame {
            gl.window.request_redraw();
            self.next_frame = now + gl.view.frame_interval();
        }
        el.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_always_carries_the_windowed_size() {
        use winit::dpi::Size;
        for fs in [true, false] {
            let a = window_attributes(fs);
            assert_eq!(
                a.inner_size,
                Some(Size::Logical(WINDOWED_SIZE)),
                "fullscreen start {fs}: un-fullscreen needs a size to come back to"
            );
            assert_eq!(a.fullscreen.is_some(), fs);
        }
    }

    #[test]
    fn pacing_policy() {
        let tv = crate::config::TvConfig::default(); // 60 active, 10 calm
        assert_eq!(pace(true, true, &tv), 60);
        assert_eq!(pace(false, true, &tv), 24); // ambient motion: 24 fps (perf budget)
        assert_eq!(pace(false, false, &tv), 10);
        let slow = crate::config::TvConfig { fps_active: 24, ..Default::default() };
        assert_eq!(pace(false, true, &slow), 24);
    }
}
