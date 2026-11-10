//! The Displays page's arrangement maths, in logical pixels: output sizes after scale and
//! rotation, snapping a dragged output to its neighbours, and the scales that give whole sizes.

/// One output on the arrangement, in logical pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct Rect {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    fn overlaps(&self, o: &Rect) -> bool {
        self.x < o.x + o.w && o.x < self.x + self.w && self.y < o.y + o.h && o.y < self.y + self.h
    }

    fn touches(&self, o: &Rect) -> bool {
        let x_meet = self.x <= o.x + o.w && o.x <= self.x + self.w;
        let y_meet = self.y <= o.y + o.h && o.y <= self.y + self.h;
        x_meet && y_meet
    }

    fn at(&self, x: i32, y: i32) -> Rect {
        Rect {
            x,
            y,
            ..self.clone()
        }
    }
}

/// How close (in logical pixels) an edge must be to snap.
const SNAP: i32 = 32;

/// The scales the page offers, before [`scale_choices`] keeps the ones that fit.
const SCALES: [f64; 9] = [1.0, 1.25, 1.333333, 1.5, 1.6, 1.75, 2.0, 2.5, 3.0];

/// The mode's size divided by `scale` and rounded, with width and height swapped for a
/// quarter-turn (odd transforms).
pub fn logical_size(mode_w: u32, mode_h: u32, scale: f64, transform: u8) -> (i32, i32) {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let w = (f64::from(mode_w) / scale).round() as i32;
    let h = (f64::from(mode_h) / scale).round() as i32;
    if transform % 2 == 1 { (h, w) } else { (w, h) }
}

/// The nearest value within [`SNAP`] of `v` among `targets`, else `v`.
fn snap(v: i32, targets: impl Iterator<Item = i32>) -> i32 {
    targets
        .map(|t| (t, (t - v).abs()))
        .filter(|(_, d)| *d <= SNAP)
        .min_by_key(|(_, d)| *d)
        .map_or(v, |(t, _)| t)
}

/// Where `moving` lands when dropped at `(x, y)`: each edge snaps to a neighbour's edge within
/// 32 px; an overlap moves it to the first free side (right, then below); and it always ends
/// touching an output when there are others.
pub fn place(others: &[Rect], moving: &Rect, x: i32, y: i32) -> (i32, i32) {
    if others.is_empty() {
        return (x, y);
    }
    // Left edge to their left or right edges, right edge likewise (expressed as a left edge).
    let xs = others
        .iter()
        .flat_map(|o| [o.x, o.x + o.w, o.x - moving.w, o.x + o.w - moving.w]);
    let ys = others
        .iter()
        .flat_map(|o| [o.y, o.y + o.h, o.y - moving.h, o.y + o.h - moving.h]);
    let mut c = moving.at(snap(x, xs), snap(y, ys));
    if let Some(hit) = others.iter().find(|o| c.overlaps(o)) {
        // The first free side of the output it overlaps: right, then below, then left, above.
        let candidates = [
            c.at(hit.x + hit.w, c.y),
            c.at(c.x, hit.y + hit.h),
            c.at(hit.x - c.w, c.y),
            c.at(c.x, hit.y - c.h),
        ];
        let free = candidates
            .iter()
            .find(|r| !others.iter().any(|o| r.overlaps(o)));
        c = match free {
            Some(r) => r.clone(),
            // Boxed in: to the right of everything.
            None => c.at(others.iter().map(|o| o.x + o.w).max().unwrap_or(0), c.y),
        };
    }
    if !others.iter().any(|o| c.touches(o)) {
        // Nearest output: join it on the side the drop is closest to.
        let near = others
            .iter()
            .min_by_key(|o| {
                let dx = (c.x - (o.x + o.w)).max(o.x - (c.x + c.w)).max(0);
                let dy = (c.y - (o.y + o.h)).max(o.y - (c.y + c.h)).max(0);
                dx + dy
            })
            .expect("others is not empty");
        let clamp_y = c.y.clamp(near.y - c.h, near.y + near.h);
        let clamp_x = c.x.clamp(near.x - c.w, near.x + near.w);
        c = if c.x >= near.x + near.w {
            c.at(near.x + near.w, clamp_y)
        } else if c.x + c.w <= near.x {
            c.at(near.x - c.w, clamp_y)
        } else if c.y >= near.y + near.h {
            c.at(clamp_x, near.y + near.h)
        } else {
            c.at(clamp_x, near.y - c.h)
        };
    }
    (c.x, c.y)
}

/// Shift every rect so the smallest x and y are 0.
pub fn normalise(rects: &mut [Rect]) {
    let (Some(min_x), Some(min_y)) = (
        rects.iter().map(|r| r.x).min(),
        rects.iter().map(|r| r.y).min(),
    ) else {
        return;
    };
    for r in rects {
        r.x -= min_x;
        r.y -= min_y;
    }
}

/// The scales whose logical width and height are whole numbers (within 0.01) for this mode;
/// 1.0 is always offered.
pub fn scale_choices(mode_w: u32, mode_h: u32) -> Vec<f64> {
    let whole = |n: u32, s: f64| {
        let v = f64::from(n) / s;
        (v - v.round()).abs() < 0.01
    };
    SCALES
        .into_iter()
        .filter(|&s| s == 1.0 || (whole(mode_w, s) && whole(mode_h, s)))
        .collect()
}

/// Hyprland's `"2560x1440@143.91Hz"` (the rate and `Hz` optional) as `(w, h, rate)`.
pub fn parse_mode(s: &str) -> Option<(u32, u32, Option<f64>)> {
    let s = s.trim().trim_end_matches("Hz");
    let (size, rate) = match s.split_once('@') {
        Some((size, r)) => (size, Some(r.parse::<f64>().ok().filter(|r| r.is_finite())?)),
        None => (s, None),
    };
    let (w, h) = size.split_once('x')?;
    Some((w.parse().ok()?, h.parse().ok()?, rate))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(name: &str, x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect {
            name: name.into(),
            x,
            y,
            w,
            h,
        }
    }

    #[test]
    fn snaps_to_right_edge() {
        let a = rect("A", 0, 0, 1920, 1080);
        let b = rect("B", 0, 0, 1280, 1024);
        assert_eq!(place(&[a.clone()], &b, 1940, 20), (1920, 0));
        assert_eq!(place(&[a.clone()], &b, 1900, 500), (1920, 500));
        // Below, with its right edge lined up with A's.
        assert_eq!(place(&[a], &b, 650, 1100), (640, 1080));
    }

    #[test]
    fn overlap_pushed_out() {
        let a = rect("A", 0, 0, 1920, 1080);
        let b = rect("B", 0, 0, 1280, 1024);
        let (x, y) = place(&[a.clone()], &b, 900, 300);
        let placed = b.at(x, y);
        assert!(!placed.overlaps(&a), "{placed:?}");
        assert!(placed.touches(&a));
        assert_eq!((x, y), (1920, 300));
        // Far away comes back to touch.
        let (x, y) = place(&[a.clone()], &b, 5000, 4000);
        assert!(b.at(x, y).touches(&a), "{x},{y}");
        assert!(!b.at(x, y).overlaps(&a));
    }

    #[test]
    fn rotated_size_swaps() {
        assert_eq!(logical_size(3840, 2160, 2.0, 0), (1920, 1080));
        assert_eq!(logical_size(3840, 2160, 2.0, 1), (1080, 1920));
        assert_eq!(logical_size(2560, 1440, 1.25, 3), (1152, 2048));
        assert_eq!(logical_size(1920, 1080, 1.0, 2), (1920, 1080));
        assert_eq!(logical_size(1920, 1080, 0.0, 0), (1920, 1080));
    }

    #[test]
    fn normalise_to_origin() {
        let mut rs = vec![rect("A", -1920, 100, 10, 10), rect("B", 0, 40, 10, 10)];
        normalise(&mut rs);
        assert_eq!((rs[0].x, rs[0].y), (0, 60));
        assert_eq!((rs[1].x, rs[1].y), (1920, 0));
        normalise(&mut []);
    }

    #[test]
    fn scale_choices_for_4k_and_1080p() {
        let k4 = scale_choices(3840, 2160);
        assert!(
            k4.contains(&1.5) && k4.contains(&2.0) && k4.contains(&1.0),
            "{k4:?}"
        );
        let hd = scale_choices(1920, 1080);
        assert!(hd.contains(&1.25) && hd.contains(&1.5), "{hd:?}");
        assert!(!hd.contains(&1.75), "1920 / 1.75 isn't whole: {hd:?}");
        assert_eq!(scale_choices(1366, 768), [1.0, 2.0]);
        assert_eq!(scale_choices(1366, 767), [1.0]);
    }

    #[test]
    fn parse_hyprland_modes() {
        assert_eq!(
            parse_mode("2560x1440@143.91Hz"),
            Some((2560, 1440, Some(143.91)))
        );
        assert_eq!(parse_mode("1920x1080"), Some((1920, 1080, None)));
        assert_eq!(parse_mode("1920x1080@60"), Some((1920, 1080, Some(60.0))));
        for bad in ["", "x", "1920", "axb@60Hz", "1920x1080@fastHz"] {
            assert_eq!(parse_mode(bad), None, "{bad}");
        }
    }
}
