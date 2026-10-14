use super::*;

/// Cells as fonts give them: even and odd sides, thin and thick lines.
const CELLS: [(u32, u32, u32); 4] = [(16, 32, 2), (9, 19, 1), (20, 41, 3), (7, 15, 1)];

fn arms(ch: char) -> [u8; 4] {
    LINES[(ch as u32 - 0x2500) as usize]
}

fn every_drawn() -> impl Iterator<Item = char> {
    ('\u{2500}'..='\u{259F}').chain('\u{E0B0}'..='\u{E0B7}')
}

/// The spans across an edge of the rects touching it; sides go up, right, down, left.
fn edge(shapes: &[Shape], side: usize, cell_w: u32, cell_h: u32) -> Vec<(u32, u32)> {
    let mut spans: Vec<(u32, u32)> = shapes
        .iter()
        .filter_map(|shape| match *shape {
            Shape::Rect { x, y, w, h, .. } => {
                let touches = [y == 0, x + w == cell_w, y + h == cell_h, x == 0][side];
                let span = if side.is_multiple_of(2) {
                    (x, x + w)
                } else {
                    (y, y + h)
                };
                touches.then_some(span)
            }
            _ => None,
        })
        .collect();
    spans.sort();
    spans
}

fn points(steps: &[Step]) -> Vec<(f32, f32)> {
    steps
        .iter()
        .flat_map(|step| match *step {
            Step::Move(x, y) | Step::Line(x, y) => vec![(x, y)],
            Step::Curve([ax, ay, bx, by, x, y]) => vec![(ax, ay), (bx, by), (x, y)],
        })
        .collect()
}

#[test]
fn the_table_reads_as_unicode_names_say() {
    assert_eq!(arms('─'), [0, 1, 0, 1]);
    assert_eq!(arms('┃'), [2, 0, 2, 0]);
    assert_eq!(arms('┍'), [0, 2, 1, 0], "down light and right heavy");
    assert_eq!(arms('╒'), [0, 3, 1, 0], "down single and right double");
    assert_eq!(arms('╬'), [3, 3, 3, 3]);
    assert_eq!(arms('╽'), [1, 0, 2, 0], "light up and heavy down");
    assert_eq!(arms('╾'), [0, 1, 0, 2], "heavy left and light right");
}

#[test]
fn every_drawn_char_draws_inside_its_cell() {
    for (w, h, t) in CELLS {
        for ch in every_drawn() {
            let shapes = shapes(ch, w, h, t);
            assert!(!shapes.is_empty(), "{ch} draws nothing");
            for shape in &shapes {
                match shape {
                    Shape::Rect {
                        x, y, w: rw, h: rh, ..
                    } => {
                        assert!(*rw > 0 && *rh > 0, "{ch} has an empty rect at {w}x{h}");
                        assert!(x + rw <= w && y + rh <= h, "{ch} leaves {w}x{h}");
                    }
                    Shape::Fill(steps) | Shape::Stroke(steps, _) => {
                        let reach = t as f32;
                        for (px, py) in points(steps) {
                            let inside = (-reach..=w as f32 + reach).contains(&px)
                                && (-reach..=h as f32 + reach).contains(&py);
                            assert!(inside, "{ch} reaches ({px}, {py}) in {w}x{h}");
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn straight_lines_sit_centered_at_their_weight() {
    let (w, h, t) = (16, 32, 2);
    let across = |ch: char, side: usize| edge(&shapes(ch, w, h, t), side, w, h);
    assert_eq!(across('─', 1), [(15, 17)]);
    assert_eq!(across('━', 1), [(14, 18)]);
    assert_eq!(across('═', 1), [(13, 15), (17, 19)]);
    assert_eq!(across('│', 2), [(7, 9)]);
    assert_eq!(across('┃', 2), [(6, 10)]);
    assert_eq!(across('║', 2), [(5, 7), (9, 11)]);
}

#[test]
fn every_arm_meets_the_straight_line_of_its_weight() {
    for (w, h, t) in CELLS {
        let across = [['│', '┃', '║'], ['─', '━', '═']];
        for code in 0x2500..=0x257F_u32 {
            let ch = char::from_u32(code).expect("a char");
            let weights = arms(ch);
            if weights == [0; 4] {
                continue;
            }
            let shapes = shapes(ch, w, h, t);
            for (side, weight) in weights.into_iter().enumerate() {
                let found = edge(&shapes, side, w, h);
                if weight == 0 {
                    assert!(found.is_empty(), "{ch} touches side {side} with no arm");
                    continue;
                }
                let straight = across[side % 2][usize::from(weight) - 1];
                let want = edge(&super::shapes(straight, w, h, t), side, w, h);
                assert_eq!(
                    found, want,
                    "{ch}'s side {side} misses {straight} in {w}x{h}"
                );
            }
        }
    }
}

#[test]
fn dashes_tile_with_one_gap_between_cells() {
    for (w, h, t) in CELLS {
        for (dashes, count) in [("┄┅┆┇", 3), ("┈┉┊┋", 4), ("╌╍╎╏", 2)] {
            for ch in dashes.chars() {
                let vertical = "┆┇┊┋╎╏".contains(ch);
                let side = if vertical { h } else { w };
                let drawn = shapes(ch, w, h, t);
                if side < 2 * count {
                    let solid = if vertical { '│' } else { '─' };
                    assert_eq!(drawn, shapes(solid, w, h, t), "{ch} at {w}x{h} is solid");
                    continue;
                }
                let mut runs: Vec<(u32, u32)> = drawn
                    .iter()
                    .map(|shape| match *shape {
                        Shape::Rect { y, h: rh, .. } if vertical => (y, y + rh),
                        Shape::Rect { x, w: rw, .. } => (x, x + rw),
                        _ => panic!("{ch} is drawn with rects"),
                    })
                    .collect();
                runs.sort();
                assert_eq!(runs.len() as u32, count, "{ch} at {w}x{h}");
                let gap = runs[1].0 - runs[0].1;
                assert!(gap > 0, "{ch} has no gap in {w}x{h}");
                for pair in runs.windows(2) {
                    assert_eq!(pair[1].0 - pair[0].1, gap, "{ch} gaps unevenly");
                }
                let lead = if vertical { 0 } else { gap / 2 };
                assert_eq!(runs[0].0, lead, "{ch} starts off center at {w}x{h}");
                let margins = runs[0].0 + side - runs[runs.len() - 1].1;
                assert_eq!(
                    margins, gap,
                    "{ch} breaks the rhythm across cells at {w}x{h}"
                );
            }
        }
    }
}

#[test]
fn halves_cover_the_cell_and_shades_wash_it() {
    for (w, h, t) in CELLS {
        let span = |ch: char| match shapes(ch, w, h, t)[0] {
            Shape::Rect {
                x, y, w: rw, h: rh, ..
            } => (x, y, x + rw, y + rh),
            _ => panic!("{ch} is a rect"),
        };
        let (upper, lower) = (span('▀'), span('▄'));
        assert_eq!((upper.1, lower.3), (0, h));
        assert!(upper.3 >= lower.1, "a gap between ▀ and ▄ at {w}x{h}");
        let (left, right) = (span('▌'), span('▐'));
        assert_eq!((left.0, right.2), (0, w));
        assert!(left.2 >= right.0, "a gap between ▌ and ▐ at {w}x{h}");
        for (ch, alpha) in [('░', 0.25), ('▒', 0.5), ('▓', 0.75)] {
            let want = Shape::Rect {
                x: 0,
                y: 0,
                w,
                h,
                alpha,
            };
            assert_eq!(shapes(ch, w, h, t), [want]);
        }
        assert_eq!(shapes('▚', w, h, t).len(), 2);
        assert_eq!(shapes('▟', w, h, t).len(), 3);
    }
}

#[test]
fn powerline_fills_the_cell_and_mirrors() {
    let (w, h) = (16., 32.);
    let right = vec![Step::Move(0., 0.), Step::Line(w, h / 2.), Step::Line(0., h)];
    let left = vec![Step::Move(w, 0.), Step::Line(0., h / 2.), Step::Line(w, h)];
    assert_eq!(shapes('\u{E0B0}', 16, 32, 2), [Shape::Fill(right.clone())]);
    assert_eq!(shapes('\u{E0B2}', 16, 32, 2), [Shape::Fill(left)]);
    assert_eq!(shapes('\u{E0B1}', 16, 32, 2), [Shape::Stroke(right, 2.)]);
    let Shape::Fill(round) = &shapes('\u{E0B6}', 16, 32, 2)[0] else {
        panic!("a filled half circle");
    };
    assert_eq!(round.first(), Some(&Step::Move(16., 0.)));
    let Shape::Stroke(outline, 2.) = &shapes('\u{E0B5}', 16, 32, 2)[0] else {
        panic!("a stroked half circle");
    };
    assert_eq!(
        outline.first(),
        Some(&Step::Move(0., 1.)),
        "inset by half a line"
    );
}

#[test]
fn arcs_end_on_the_light_lines_centers() {
    let (w, h, t) = (16, 32, 2);
    let light = (w - t) / 2;
    let Shape::Stroke(path, width) = &shapes('╭', w, h, t)[0] else {
        panic!("╭ is a stroke");
    };
    assert_eq!(*width, t as f32);
    assert_eq!(path.first(), Some(&Step::Move(light as f32 + 1., h as f32)));
    assert_eq!(
        path.last(),
        Some(&Step::Line(w as f32, ((h - t) / 2) as f32 + 1.))
    );
}
