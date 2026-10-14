use std::rc::Rc;

use alacritty_terminal::vte::ansi::CursorShape;
use gpui::{
    Bounds, Entity, Font, FontId, FontStyle, FontWeight, Hsla, IntoElement, Pixels, Point, Size,
    StrikethroughStyle, Styled, TextRun, UnderlineStyle, Window, canvas, fill, outline, point, px,
    size,
};

use super::{
    frame::{Cursor, Frame, Style, Underline},
    view::TerminalView,
};

/// How the grid paints: font, cell, metrics and the cursor's colors.
#[derive(Clone)]
pub struct Look {
    pub font: Font,
    pub font_id: FontId,
    /// Draws icons the font lacks.
    pub icons: Font,
    pub size: Pixels,
    pub cell: Size<Pixels>,
    /// From a cell's top to the text's baseline.
    pub baseline: Pixels,
    pub descent: Pixels,
    pub rule: Pixels,
    pub cursor: Hsla,
    pub cursor_text: Hsla,
}

/// The grid fitted to its box: it sizes the pty, then paints each row and the cursor.
pub fn grid(
    view: Entity<TerminalView>,
    frame: Rc<Frame>,
    cursor: Option<(Cursor, CursorShape)>,
    look: Look,
) -> impl IntoElement {
    let cell = look.cell;
    canvas(
        move |bounds, window, cx| {
            let columns = (bounds.size.width / cell.width).floor() as usize;
            let lines = (bounds.size.height / cell.height).floor() as usize;
            view.update(cx, |view, cx| {
                if view.fit(columns, lines) {
                    cx.notify();
                    window.request_animation_frame();
                }
            });
        },
        move |bounds, _, window, _| {
            log::trace!(
                "terminal: paint {:?}, row 0 {:?}",
                bounds,
                frame.rows.first().map(|row| row.text.trim_end())
            );
            for (index, row) in frame.rows.iter().enumerate() {
                let top = bounds.origin + point(px(0.), cell.height * index as f32);
                paint_row(row, top, &look, window);
            }
            if let Some((cursor, shape)) = &cursor {
                paint_cursor(cursor, *shape, bounds.origin, &look, window);
            }
        },
    )
    .size_full()
}

fn face(style: &Style, base: &Font) -> Font {
    Font {
        weight: if style.bold {
            FontWeight::BOLD
        } else {
            FontWeight::NORMAL
        },
        style: if style.italic {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        },
        ..base.clone()
    }
}

/// Each run's style with the columns it covers.
fn spans(row: &super::frame::Row) -> Vec<(usize, usize, Style)> {
    let (mut byte, mut column) = (0, 0);
    row.runs
        .iter()
        .map(|(len, style)| {
            let columns = row.text[byte..byte + len].chars().count();
            let span = (column, columns, *style);
            byte += len;
            column += columns;
            span
        })
        .collect()
}

fn paint_row(row: &super::frame::Row, top: Point<Pixels>, look: &Look, window: &mut Window) {
    let cell = look.cell;
    let at = |column: usize| top + point(cell.width * column as f32, px(0.));
    let spans = spans(row);
    for (column, columns, style) in &spans {
        if let Some(bg) = style.bg {
            let width = cell.width * *columns as f32;
            window.paint_quad(fill(Bounds::new(at(*column), size(width, cell.height)), bg));
        }
    }
    let mut byte = 0;
    let runs: Vec<TextRun> = row
        .runs
        .iter()
        .flat_map(|(len, style)| {
            let text = &row.text[byte..byte + len];
            byte += len;
            runs(text, style, style.fg, look, window)
        })
        .collect();
    draw(&row.text, &runs, top, Some(cell.width), look, window);
    for (column, cluster, style) in &row.clusters {
        text(cluster, style.fg, style, at(*column), look, window);
    }
    for (column, columns, style) in spans {
        let width = cell.width * columns as f32;
        decorate(at(column), width, &style, look, window);
    }
}

/// Text runs for `text` in `style` and `color`: icons the font lacks take the icon font.
fn runs(text: &str, style: &Style, color: Hsla, look: &Look, window: &Window) -> Vec<TextRun> {
    let base = face(style, &look.font);
    let system = window.text_system();
    let mut runs: Vec<TextRun> = Vec::new();
    for ch in text.chars() {
        let missing = private(ch) && system.advance(look.font_id, look.size, ch).is_err();
        let font = if missing { &look.icons } else { &base };
        match runs.last_mut() {
            Some(last) if last.font == *font => last.len += ch.len_utf8(),
            _ => runs.push(TextRun {
                len: ch.len_utf8(),
                font: font.clone(),
                color,
                background_color: None,
                underline: None,
                strikethrough: None,
            }),
        }
    }
    runs
}

/// Unicode's private use areas, where Nerd Font icons live.
fn private(ch: char) -> bool {
    matches!(ch, '\u{E000}'..='\u{F8FF}' | '\u{F0000}'..='\u{FFFFD}' | '\u{100000}'..='\u{10FFFD}')
}

/// Paints `content` alone at `origin`, a cell's top-left, its marks where shaping puts them.
fn text(
    content: &str,
    color: Hsla,
    style: &Style,
    origin: Point<Pixels>,
    look: &Look,
    window: &mut Window,
) {
    let runs = runs(content, style, color, look, window);
    draw(content, &runs, origin, None, look, window);
}

/// Shapes `content` and paints each glyph on the cell's baseline below `top`; `force` sets every advance.
fn draw(
    content: &str,
    runs: &[TextRun],
    top: Point<Pixels>,
    force: Option<Pixels>,
    look: &Look,
    window: &mut Window,
) {
    let shaped =
        window
            .text_system()
            .shape_line(content.to_string().into(), look.size, runs, force);
    let baseline = top.y + look.baseline;
    for run in &shaped.runs {
        for glyph in &run.glyphs {
            let color = color_at(runs, glyph.index);
            let at = point(
                top.x + glyph.position.x,
                baseline + upward(glyph.position.y),
            );
            let painted = if glyph.is_emoji {
                window.paint_emoji(at, run.font_id, glyph.id, look.size)
            } else {
                window.paint_glyph(at, run.font_id, glyph.id, look.size, color)
            };
            if let Err(error) = painted {
                log::error!("terminal: a glyph of `{content}` failed to paint: {error:#}");
            }
        }
    }
}

/// The color of the run holding byte `index`.
fn color_at(runs: &[TextRun], index: usize) -> Hsla {
    let mut end = 0;
    runs.iter()
        .find(|run| {
            end += run.len;
            index < end
        })
        .expect("a glyph indexes its own text")
        .color
}

/// CoreText gives glyph offsets growing upward; the screen grows downward.
#[cfg(target_os = "macos")]
fn upward(y: Pixels) -> Pixels {
    -y
}

#[cfg(not(target_os = "macos"))]
fn upward(y: Pixels) -> Pixels {
    y
}

/// Underlines and strikethrough under `width` from `origin`, a cell's top-left.
fn decorate(origin: Point<Pixels>, width: Pixels, style: &Style, look: &Look, window: &mut Window) {
    let rule = look.rule;
    let line = origin.y + look.baseline + (look.descent * 0.45).max(rule);
    let bar = |x: Pixels, y: Pixels, w: Pixels| {
        fill(
            Bounds::new(point(x, y), size(w, rule)),
            style.underline_color,
        )
    };
    match style.underline {
        Underline::None => {}
        Underline::Single => window.paint_quad(bar(origin.x, line, width)),
        Underline::Double => {
            window.paint_quad(bar(origin.x, line - rule, width));
            window.paint_quad(bar(origin.x, line + rule, width));
        }
        Underline::Curly => window.paint_underline(
            point(origin.x, line - rule),
            width,
            &UnderlineStyle {
                thickness: rule,
                color: Some(style.underline_color),
                wavy: true,
            },
        ),
        Underline::Dotted => {
            let mut x = origin.x;
            while x < origin.x + width {
                window.paint_quad(bar(x, line, rule));
                x += rule * 2.;
            }
        }
        Underline::Dashed => {
            let dash = look.cell.width * 0.5;
            let mut x = origin.x;
            while x < origin.x + width {
                window.paint_quad(bar(x, line, dash.min(origin.x + width - x)));
                x += dash * 2.;
            }
        }
    }
    if style.strike {
        let middle = origin.y + look.baseline - look.size * 0.3;
        window.paint_strikethrough(
            point(origin.x, middle),
            width,
            &StrikethroughStyle {
                thickness: rule,
                color: Some(style.fg),
            },
        );
    }
}

fn paint_cursor(
    cursor: &Cursor,
    shape: CursorShape,
    origin: Point<Pixels>,
    look: &Look,
    window: &mut Window,
) {
    let cell = look.cell;
    let corner = origin
        + point(
            cell.width * cursor.column as f32,
            cell.height * cursor.row as f32,
        );
    let width = cell.width * if cursor.wide { 2. } else { 1. };
    let whole = Bounds::new(corner, size(width, cell.height));
    match shape {
        CursorShape::Block => {
            window.paint_quad(fill(whole, look.cursor));
            text(
                &cursor.text,
                look.cursor_text,
                &cursor.style,
                corner,
                look,
                window,
            );
        }
        CursorShape::HollowBlock => {
            window.paint_quad(outline(whole, look.cursor, gpui::BorderStyle::Solid));
        }
        CursorShape::Beam => {
            let bar = Bounds::new(corner, size((look.rule * 2.).max(px(1.)), cell.height));
            window.paint_quad(fill(bar, look.cursor));
        }
        CursorShape::Underline => {
            let height = look.rule * 2.;
            let bar = Bounds::new(
                corner + point(px(0.), cell.height - height),
                size(width, height),
            );
            window.paint_quad(fill(bar, look.cursor));
        }
        CursorShape::Hidden => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::term::frame::{Row, Underline};

    #[test]
    fn spans_count_columns_not_bytes() {
        let style = Style {
            fg: gpui::black(),
            bg: None,
            bold: false,
            italic: false,
            underline: Underline::None,
            underline_color: gpui::black(),
            strike: false,
        };
        let bold = Style {
            bold: true,
            ..style
        };
        let row = Row {
            text: "中 ab".into(),
            runs: vec![(4, style), (2, bold)],
            clusters: Vec::new(),
        };
        let spans: Vec<_> = spans(&row).into_iter().map(|(at, n, _)| (at, n)).collect();
        assert_eq!(spans, [(0, 2), (2, 2)]);
    }

    #[test]
    fn icons_live_in_the_private_use_areas() {
        assert!(private('\u{E0B0}'));
        assert!(private('\u{F0035}'));
        assert!(!private('a'));
        assert!(!private('中'));
    }
}
