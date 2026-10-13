//! The 3840×2160 design grid. Panels draw in design units; the canvas transform scales them
//! uniformly to the window and centers the result (letterbox / pillarbox when not 16:9).

pub const DESIGN_W: f32 = 3840.0;
pub const DESIGN_H: f32 = 2160.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    /// True when the interiors intersect (shared edges do not count).
    #[allow(dead_code)] // layout and card tests
    pub fn overlaps(&self, o: &Rect) -> bool {
        self.x < o.x + o.w && o.x < self.x + self.w && self.y < o.y + o.h && o.y < self.y + self.h
    }

    /// Shrink by `d` on every side.
    pub fn inset(&self, d: f32) -> Rect {
        Rect::new(self.x + d, self.y + d, (self.w - 2.0 * d).max(0.0), (self.h - 2.0 * d).max(0.0))
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    pub scale: f32,
    pub origin: (f32, f32),
    pub header: Rect,
    pub alert: Rect,
    /// PILOT ROSTER.
    pub units: Rect,
    /// Vertical chevron band on the roster's right edge.
    pub band: Rect,
    pub sectors: Rect,
    pub jobs: Rect,
    pub magi: Rect,
    pub timer: Rect,
    pub sdat: Rect,
    pub telemetry: Rect,
    pub procs: Rect,
    pub ticker: Rect,
}

impl Layout {
    /// Uniform scale `min(w/3840, h/2160)`, centered in the window.
    pub fn new(w: f32, h: f32) -> Self {
        let scale = (w / DESIGN_W).min(h / DESIGN_H);
        let origin = ((w - DESIGN_W * scale) / 2.0, (h - DESIGN_H * scale) / 2.0);
        // plan REVISION 2 MAIN anchors: content boxes are disjoint; plates, traces and rulers
        // drawn around them may cross the gaps.
        Self {
            scale,
            origin,
            header: Rect::new(0.0, 24.0, 3840.0, 96.0),
            alert: Rect::new(24.0, 136.0, 3792.0, 56.0),
            units: Rect::new(24.0, 210.0, 1306.0, 1290.0),
            band: Rect::new(1330.0, 210.0, 70.0, 1290.0),
            magi: Rect::new(1440.0, 210.0, 980.0, 670.0),
            sectors: Rect::new(2460.0, 210.0, 1356.0, 490.0),
            timer: Rect::new(1440.0, 900.0, 980.0, 520.0),
            telemetry: Rect::new(2460.0, 720.0, 1356.0, 640.0),
            jobs: Rect::new(24.0, 1520.0, 1306.0, 490.0),
            sdat: Rect::new(1440.0, 1440.0, 980.0, 570.0),
            procs: Rect::new(2460.0, 1380.0, 1356.0, 630.0),
            ticker: Rect::new(0.0, 2030.0, 3840.0, 106.0),
        }
    }

    /// Every box that holds text or data (must stay disjoint and on screen).
    #[allow(dead_code)] // layout tests
    pub fn content_boxes(&self) -> Vec<Rect> {
        vec![
            self.header,
            self.alert,
            self.units,
            self.band,
            self.magi,
            self.sectors,
            self.timer,
            self.telemetry,
            self.jobs,
            self.sdat,
            self.procs,
            self.ticker,
        ]
    }

    /// Design units → window pixels.
    #[allow(dead_code)] // layout tests
    pub fn to_px(&self, r: Rect) -> Rect {
        Rect::new(
            r.x * self.scale + self.origin.0,
            r.y * self.scale + self.origin.1,
            r.w * self.scale,
            r.h * self.scale,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_boxes_inside_screen_and_disjoint() {
        let l = Layout::new(3840.0, 2160.0);
        let rs = l.content_boxes();
        assert_eq!(rs.len(), 12);
        for r in &rs {
            assert!(r.x >= 0.0 && r.y >= 0.0 && r.x + r.w <= 3840.0 && r.y + r.h <= 2160.0, "{r:?}");
        }
        for (i, a) in rs.iter().enumerate() {
            for b in &rs[i + 1..] {
                assert!(!a.overlaps(b), "{a:?} {b:?}");
            }
        }
    }

    #[test]
    fn revision_2_main_anchors() {
        let l = Layout::new(3840.0, 2160.0);
        assert_eq!(l.header, Rect::new(0.0, 24.0, 3840.0, 96.0));
        assert_eq!(l.alert, Rect::new(24.0, 136.0, 3792.0, 56.0));
        assert_eq!(l.units, Rect::new(24.0, 210.0, 1306.0, 1290.0));
        assert_eq!(l.band, Rect::new(1330.0, 210.0, 70.0, 1290.0));
        assert_eq!(l.magi, Rect::new(1440.0, 210.0, 980.0, 670.0));
        assert_eq!(l.sectors, Rect::new(2460.0, 210.0, 1356.0, 490.0));
        assert_eq!(l.timer, Rect::new(1440.0, 900.0, 980.0, 520.0));
        assert_eq!(l.telemetry, Rect::new(2460.0, 720.0, 1356.0, 640.0));
        assert_eq!(l.jobs, Rect::new(24.0, 1520.0, 1306.0, 490.0));
        assert_eq!(l.sdat, Rect::new(1440.0, 1440.0, 980.0, 570.0));
        assert_eq!(l.procs, Rect::new(2460.0, 1380.0, 1356.0, 630.0));
        assert_eq!(l.ticker, Rect::new(0.0, 2030.0, 3840.0, 106.0));
    }

    #[test]
    fn scale_and_letterbox_for_other_sizes() {
        for (w, h) in [(1920.0, 1080.0), (2560.0, 1080.0), (1280.0, 800.0), (3840.0, 2160.0)] {
            let l = Layout::new(w, h);
            for r in l.content_boxes() {
                let px = l.to_px(r);
                assert!(
                    px.x >= -0.5 && px.y >= -0.5 && px.x + px.w <= w + 0.5 && px.y + px.h <= h + 0.5,
                    "{w}x{h} {px:?}"
                );
            }
        }
        assert_eq!(Layout::new(1920.0, 1080.0).scale, 0.5);
        assert_eq!(Layout::new(2560.0, 1080.0).origin.0, (2560.0 - 1920.0) / 2.0);
    }
}
