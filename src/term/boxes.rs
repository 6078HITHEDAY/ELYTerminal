/// One step of a path, in device pixels from the cell's top-left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    Move(f32, f32),
    Line(f32, f32),
    /// A cubic curve: two control points, then the end.
    Curve([f32; 6]),
}

/// A piece of a drawn character, in device pixels from the cell's top-left.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// A rectangle filled at `alpha` of the text color.
    Rect {
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        alpha: f32,
    },
    /// A closed outline, filled.
    Fill(Vec<Step>),
    /// An open path, stroked this wide with butt ends.
    Stroke(Vec<Step>, f32),
}

/// Whether the terminal draws `ch` as shapes instead of through the font.
pub fn drawn(ch: char) -> bool {
    matches!(ch, '\u{2500}'..='\u{259F}' | '\u{E0B0}'..='\u{E0B7}')
}

const NONE: u8 = 0;
const LIGHT: u8 = 1;
const HEAVY: u8 = 2;
const DOUBLE: u8 = 3;

/// Each line character's arms, up, right, down, left, from U+2500.
#[rustfmt::skip]
const LINES: [[u8; 4]; 128] = [
    [0, 1, 0, 1], [0, 2, 0, 2], [1, 0, 1, 0], [2, 0, 2, 0], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0],
    [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [0, 1, 1, 0], [0, 2, 1, 0], [0, 1, 2, 0], [0, 2, 2, 0],
    [0, 0, 1, 1], [0, 0, 1, 2], [0, 0, 2, 1], [0, 0, 2, 2], [1, 1, 0, 0], [1, 2, 0, 0], [2, 1, 0, 0], [2, 2, 0, 0],
    [1, 0, 0, 1], [1, 0, 0, 2], [2, 0, 0, 1], [2, 0, 0, 2], [1, 1, 1, 0], [1, 2, 1, 0], [2, 1, 1, 0], [1, 1, 2, 0],
    [2, 1, 2, 0], [2, 2, 1, 0], [1, 2, 2, 0], [2, 2, 2, 0], [1, 0, 1, 1], [1, 0, 1, 2], [2, 0, 1, 1], [1, 0, 2, 1],
    [2, 0, 2, 1], [2, 0, 1, 2], [1, 0, 2, 2], [2, 0, 2, 2], [0, 1, 1, 1], [0, 1, 1, 2], [0, 2, 1, 1], [0, 2, 1, 2],
    [0, 1, 2, 1], [0, 1, 2, 2], [0, 2, 2, 1], [0, 2, 2, 2], [1, 1, 0, 1], [1, 1, 0, 2], [1, 2, 0, 1], [1, 2, 0, 2],
    [2, 1, 0, 1], [2, 1, 0, 2], [2, 2, 0, 1], [2, 2, 0, 2], [1, 1, 1, 1], [1, 1, 1, 2], [1, 2, 1, 1], [1, 2, 1, 2],
    [2, 1, 1, 1], [1, 1, 2, 1], [2, 1, 2, 1], [2, 1, 1, 2], [2, 2, 1, 1], [1, 1, 2, 2], [1, 2, 2, 1], [2, 2, 1, 2],
    [1, 2, 2, 2], [2, 1, 2, 2], [2, 2, 2, 1], [2, 2, 2, 2], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0],
    [0, 3, 0, 3], [3, 0, 3, 0], [0, 3, 1, 0], [0, 1, 3, 0], [0, 3, 3, 0], [0, 0, 1, 3], [0, 0, 3, 1], [0, 0, 3, 3],
    [1, 3, 0, 0], [3, 1, 0, 0], [3, 3, 0, 0], [1, 0, 0, 3], [3, 0, 0, 1], [3, 0, 0, 3], [1, 3, 1, 0], [3, 1, 3, 0],
    [3, 3, 3, 0], [1, 0, 1, 3], [3, 0, 3, 1], [3, 0, 3, 3], [0, 3, 1, 3], [0, 1, 3, 1], [0, 3, 3, 3], [1, 3, 0, 3],
    [3, 1, 0, 1], [3, 3, 0, 3], [1, 3, 1, 3], [3, 1, 3, 1], [3, 3, 3, 3], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0],
    [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 1], [1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0],
    [0, 0, 0, 2], [2, 0, 0, 0], [0, 2, 0, 0], [0, 0, 2, 0], [0, 2, 0, 1], [1, 0, 2, 0], [0, 1, 0, 2], [2, 0, 1, 0],
];

/// Quadrant blocks from U+2596: top-left, top-right, bottom-left, bottom-right bits.
const QUADRANTS: [u8; 10] = [
    0b0010, 0b0001, 0b1000, 0b1011, 0b1001, 0b1110, 0b1101, 0b0100, 0b0110, 0b0111,
];

/// The shapes of a `drawn` char in a `w` by `h` cell whose light lines are `t` thick.
pub fn shapes(ch: char, w: u32, h: u32, t: u32) -> Vec<Shape> {
    let code = ch as u32;
    match code {
        0x2504..=0x250B | 0x254C..=0x254F => dashes(code, w, h, t),
        0x256D..=0x2570 => arc(code, w, h, t),
        0x2571..=0x2573 => diagonals(code, w, h, t),
        0x2500..=0x257F => lines(LINES[(code - 0x2500) as usize], w, h, t),
        0x2580..=0x259F => blocks(code, w, h),
        0xE0B0..=0xE0B7 => powerline(code, w, h, t),
        _ => panic!("U+{code:04X} is not a drawn character"),
    }
}

fn rect(x0: u32, y0: u32, x1: u32, y1: u32) -> Shape {
    Shape::Rect {
        x: x0,
        y: y0,
        w: x1.saturating_sub(x0),
        h: y1.saturating_sub(y0),
        alpha: 1.,
    }
}

/// Where an arm ends at the center, by the arms crossing it: `[heavy, double, light, short]`.
fn stop((a, b): (u8, u8), straight: bool, [heavy, double, light, short]: [u32; 4]) -> u32 {
    if a == HEAVY || b == HEAVY {
        heavy
    } else if a != b || straight {
        if a == DOUBLE || b == DOUBLE {
            double
        } else {
            light
        }
    } else if a == NONE && b == NONE {
        light
    } else {
        short
    }
}

/// Arms from the center to the edges, joined as Ghostty joins them.
fn lines([up, right, down, left]: [u8; 4], w: u32, h: u32, t: u32) -> Vec<Shape> {
    let heavy = 2 * t;
    let h_light_top = h.saturating_sub(t) / 2;
    let h_light_bottom = h_light_top + t;
    let h_heavy_top = h.saturating_sub(heavy) / 2;
    let h_heavy_bottom = h_heavy_top + heavy;
    let h_double_top = h_light_top.saturating_sub(t);
    let h_double_bottom = h_light_bottom + t;
    let v_light_left = w.saturating_sub(t) / 2;
    let v_light_right = v_light_left + t;
    let v_heavy_left = w.saturating_sub(heavy) / 2;
    let v_heavy_right = v_heavy_left + heavy;
    let v_double_left = v_light_left.saturating_sub(t);
    let v_double_right = v_light_right + t;
    let up_bottom = stop(
        (left, right),
        down == up,
        [h_heavy_bottom, h_double_bottom, h_light_bottom, h_light_top],
    );
    let down_top = stop(
        (left, right),
        up == down,
        [h_heavy_top, h_double_top, h_light_top, h_light_bottom],
    );
    let left_right = stop(
        (up, down),
        left == right,
        [v_heavy_right, v_double_right, v_light_right, v_light_left],
    );
    let right_left = stop(
        (up, down),
        right == left,
        [v_heavy_left, v_double_left, v_light_left, v_light_right],
    );
    let (top_double, bottom_double) = (up == DOUBLE, down == DOUBLE);
    let mut out = Vec::new();
    match up {
        LIGHT => out.push(rect(v_light_left, 0, v_light_right, up_bottom)),
        HEAVY => out.push(rect(v_heavy_left, 0, v_heavy_right, up_bottom)),
        DOUBLE => {
            let stop = |side| {
                if side == DOUBLE {
                    h_light_top
                } else {
                    up_bottom
                }
            };
            out.push(rect(v_double_left, 0, v_light_left, stop(left)));
            out.push(rect(v_light_right, 0, v_double_right, stop(right)));
        }
        _ => {}
    }
    match right {
        LIGHT => out.push(rect(right_left, h_light_top, w, h_light_bottom)),
        HEAVY => out.push(rect(right_left, h_heavy_top, w, h_heavy_bottom)),
        DOUBLE => {
            let start = |double| if double { v_light_right } else { right_left };
            out.push(rect(start(top_double), h_double_top, w, h_light_top));
            out.push(rect(
                start(bottom_double),
                h_light_bottom,
                w,
                h_double_bottom,
            ));
        }
        _ => {}
    }
    match down {
        LIGHT => out.push(rect(v_light_left, down_top, v_light_right, h)),
        HEAVY => out.push(rect(v_heavy_left, down_top, v_heavy_right, h)),
        DOUBLE => {
            let start = |side| {
                if side == DOUBLE {
                    h_light_bottom
                } else {
                    down_top
                }
            };
            out.push(rect(v_double_left, start(left), v_light_left, h));
            out.push(rect(v_light_right, start(right), v_double_right, h));
        }
        _ => {}
    }
    match left {
        LIGHT => out.push(rect(0, h_light_top, left_right, h_light_bottom)),
        HEAVY => out.push(rect(0, h_heavy_top, left_right, h_heavy_bottom)),
        DOUBLE => {
            let end = |double| if double { v_light_left } else { left_right };
            out.push(rect(0, h_double_top, end(top_double), h_light_top));
            out.push(rect(0, h_light_bottom, end(bottom_double), h_double_bottom));
        }
        _ => {}
    }
    out
}

/// Dashes that tile across cells; a cell too small for them takes a solid light line.
fn dashes(code: u32, w: u32, h: u32, t: u32) -> Vec<Shape> {
    let wide = t.max(4);
    let (count, thick, gap, vertical) = match code {
        0x2504 => (3, t, wide, false),
        0x2505 => (3, 2 * t, wide, false),
        0x2506 => (3, t, wide, true),
        0x2507 => (3, 2 * t, wide, true),
        0x2508 => (4, t, wide, false),
        0x2509 => (4, 2 * t, wide, false),
        0x250A => (4, t, wide, true),
        0x250B => (4, 2 * t, wide, true),
        0x254C => (2, t, t, false),
        0x254D => (2, 2 * t, 2 * t, false),
        0x254E => (2, t, 2 * t, true),
        _ => (2, 2 * t, 2 * t, true),
    };
    let (along, across) = if vertical { (h, w) } else { (w, h) };
    if along < 2 * count {
        let solid = if vertical {
            [LIGHT, 0, LIGHT, 0]
        } else {
            [0, LIGHT, 0, LIGHT]
        };
        return lines(solid, w, h, t);
    }
    let gap = gap.min(along / (2 * count));
    let total = along - count * gap;
    let (dash, mut extra) = (total / count, total % count);
    let side = across.saturating_sub(thick) / 2;
    let mut at = if vertical { 0 } else { gap / 2 };
    (0..count)
        .map(|_| {
            let end = at + dash + u32::from(extra > 0);
            extra = extra.saturating_sub(1);
            let piece = if vertical {
                rect(side, at, side + thick, end)
            } else {
                rect(at, side, end, side + thick)
            };
            at = end + gap;
            piece
        })
        .collect()
}

/// A rounded corner from the light lines' centers to two edges.
fn arc(code: u32, w: u32, h: u32, t: u32) -> Vec<Shape> {
    let (wf, hf, tf) = (w as f32, h as f32, t as f32);
    let cx = (w.saturating_sub(t) / 2) as f32 + tf / 2.;
    let cy = (h.saturating_sub(t) / 2) as f32 + tf / 2.;
    let r = wf.min(hf) / 2.;
    let s = 0.25;
    let [dy, dx]: [f32; 2] = [[1., 1.], [1., -1.], [-1., -1.], [-1., 1.]][(code - 0x256D) as usize];
    let (edge_x, edge_y) = (if dx > 0. { wf } else { 0. }, if dy > 0. { hf } else { 0. });
    let path = vec![
        Step::Move(cx, edge_y),
        Step::Line(cx, cy + dy * r),
        Step::Curve([cx, cy + dy * s * r, cx + dx * s * r, cy, cx + dx * r, cy]),
        Step::Line(edge_x, cy),
    ];
    vec![Shape::Stroke(path, tf)]
}

/// Corner-to-corner lines, run half a pixel past so neighbors join once clipped.
fn diagonals(code: u32, w: u32, h: u32, t: u32) -> Vec<Shape> {
    let (wf, hf) = (w as f32, h as f32);
    let (sx, sy) = ((wf / hf).min(1.) / 2., (hf / wf).min(1.) / 2.);
    let line = |from: (f32, f32), to: (f32, f32)| {
        Shape::Stroke(
            vec![Step::Move(from.0, from.1), Step::Line(to.0, to.1)],
            t as f32,
        )
    };
    let rising = line((wf + sx, -sy), (-sx, hf + sy));
    let falling = line((-sx, -sy), (wf + sx, hf + sy));
    match code {
        0x2571 => vec![rising],
        0x2572 => vec![falling],
        _ => vec![rising, falling],
    }
}

/// Block elements: eighths, halves, shades and quadrants.
fn blocks(code: u32, w: u32, h: u32) -> Vec<Shape> {
    let part = |share: f32, side: u32| (side as f32 * share).round() as u32;
    let eighths = |n: u32| n as f32 / 8.;
    let shade = |alpha| Shape::Rect {
        x: 0,
        y: 0,
        w,
        h,
        alpha,
    };
    match code {
        0x2580 => vec![rect(0, 0, w, part(0.5, h))],
        0x2581..=0x2587 => vec![rect(0, h - part(eighths(code - 0x2580), h), w, h)],
        0x2588 => vec![rect(0, 0, w, h)],
        0x2589..=0x258F => vec![rect(0, 0, part(eighths(0x2590 - code), w), h)],
        0x2590 => vec![rect(w - part(0.5, w), 0, w, h)],
        0x2591..=0x2593 => vec![shade((code - 0x2590) as f32 / 4.)],
        0x2594 => vec![rect(0, 0, w, part(0.125, h))],
        0x2595 => vec![rect(w - part(0.125, w), 0, w, h)],
        _ => {
            let bits = QUADRANTS[(code - 0x2596) as usize];
            let (mx, my) = (part(0.5, w), part(0.5, h));
            let (nx, ny) = (w - mx, h - my);
            [
                rect(0, 0, mx, my),
                rect(nx, 0, w, my),
                rect(0, ny, mx, h),
                rect(nx, ny, w, h),
            ]
            .into_iter()
            .enumerate()
            .filter(|(index, _)| bits & (0b1000 >> index) != 0)
            .map(|(_, piece)| piece)
            .collect()
        }
    }
}

/// Powerline's arrows and half circles, filling the cell's height.
fn powerline(code: u32, w: u32, h: u32, t: u32) -> Vec<Shape> {
    let (wf, hf, tf) = (w as f32, h as f32, t as f32);
    let steps = match code {
        0xE0B0..=0xE0B3 => vec![
            Step::Move(0., 0.),
            Step::Line(wf, hf / 2.),
            Step::Line(0., hf),
        ],
        0xE0B4 | 0xE0B6 => half_circle(wf, hf, 0.),
        _ => half_circle(wf, hf, tf / 2.),
    };
    let steps = if matches!(code, 0xE0B2 | 0xE0B3 | 0xE0B6 | 0xE0B7) {
        mirror(steps, wf)
    } else {
        steps
    };
    if matches!(code, 0xE0B0 | 0xE0B2 | 0xE0B4 | 0xE0B6) {
        vec![Shape::Fill(steps)]
    } else {
        vec![Shape::Stroke(steps, tf)]
    }
}

/// The right half of a circle against the left edge, inset `by` so a stroke stays inside.
fn half_circle(w: f32, h: f32, by: f32) -> Vec<Step> {
    let c = (2f32.sqrt() - 1.) * 4. / 3.;
    let r = w.min(h / 2.) - by;
    let (top, bottom) = (by, h - by);
    vec![
        Step::Move(0., top),
        Step::Curve([r * c, top, r, top + r - r * c, r, top + r]),
        Step::Line(r, bottom - r),
        Step::Curve([r, bottom - r + r * c, r * c, bottom, 0., bottom]),
    ]
}

/// The steps flipped left to right within a cell `w` wide.
fn mirror(steps: Vec<Step>, w: f32) -> Vec<Step> {
    steps
        .into_iter()
        .map(|step| match step {
            Step::Move(x, y) => Step::Move(w - x, y),
            Step::Line(x, y) => Step::Line(w - x, y),
            Step::Curve([ax, ay, bx, by, x, y]) => Step::Curve([w - ax, ay, w - bx, by, w - x, y]),
        })
        .collect()
}

#[cfg(test)]
mod tests;
