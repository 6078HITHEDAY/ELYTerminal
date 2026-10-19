use alacritty_terminal::{
    event::VoidListener,
    term::{Config, test::TermSize},
    vte::ansi::Processor,
};

use super::*;
use crate::term::prompts::Prompts;

const PROMPT: &str = "\x1b]133;A\x07$ \x1b]133;B\x07";

/// A terminal `columns` wide after `text` passed the prompts' rewrite, as the tap passes it.
fn term(columns: usize, text: &str) -> Term<VoidListener> {
    let mut term = Term::new(Config::default(), &TermSize::new(columns, 4), VoidListener);
    let mut parser: Processor = Processor::new();
    parser.advance(&mut term, Prompts::default().pass(text.as_bytes()));
    term
}

fn click(line: i32, column: usize) -> Point {
    Point::new(Line(line), Column(column))
}

#[test]
fn a_click_on_the_typed_line_counts_the_characters_to_it() {
    let typed = term(20, &format!("{PROMPT}echo hello"));
    assert_eq!(arrows(&typed, click(0, 7)), Some(-5), "back to hello");
    assert_eq!(
        arrows(&typed, click(0, 2)),
        Some(-10),
        "the first character"
    );
    assert_eq!(
        arrows(&typed, click(0, 12)),
        Some(0),
        "the cursor's own cell"
    );
    let moved = term(20, &format!("{PROMPT}echo hello\x1b[5D"));
    assert_eq!(arrows(&moved, click(0, 11)), Some(4), "on to the last o");
    assert_eq!(
        arrows(&moved, click(0, 18)),
        Some(5),
        "past the end, the end"
    );
}

#[test]
fn a_click_off_the_input_moves_nothing() {
    let typed = term(20, &format!("{PROMPT}ls"));
    assert_eq!(arrows(&typed, click(0, 0)), None, "on the prompt");
    assert_eq!(arrows(&typed, click(0, 1)), None, "on the prompt's space");
    assert_eq!(arrows(&typed, click(2, 3)), None, "another line");
    let ran = term(20, &format!("{PROMPT}cat\r\nhello"));
    assert_eq!(arrows(&ran, click(1, 2)), None, "a line no prompt shows");
    let output = term(20, "$ plain text");
    assert_eq!(arrows(&output, click(0, 3)), None, "no marks at all");
    let full = term(20, &format!("{PROMPT}vim\x1b[?1049h"));
    assert_eq!(arrows(&full, click(0, 3)), None, "the alternate screen");
}

#[test]
fn a_wide_character_takes_one_arrow() {
    let wide = term(20, &format!("{PROMPT}中文ab"));
    assert_eq!(arrows(&wide, click(0, 3)), Some(-4), "the right half of 中");
    assert_eq!(arrows(&wide, click(0, 4)), Some(-3), "文");
    assert_eq!(arrows(&wide, click(0, 7)), Some(-1), "b");
}

#[test]
fn soft_wrapped_rows_are_one_line() {
    let wrapped = term(10, &format!("{PROMPT}abcdefghijkl"));
    assert_eq!(wrapped.grid().cursor.point, click(1, 4));
    assert_eq!(arrows(&wrapped, click(0, 3)), Some(-11), "back to b");
    let back = term(10, &format!("{PROMPT}abcdefghijkl\x1b[3D"));
    assert_eq!(arrows(&back, click(0, 3)), Some(-8), "up to b");
    assert_eq!(arrows(&back, click(1, 3)), Some(2), "on to l");
}

#[test]
fn trailing_spaces_count_only_up_to_the_cursor() {
    let spaced = term(20, &format!("{PROMPT}ls  "));
    assert_eq!(arrows(&spaced, click(0, 15)), Some(0));
    assert_eq!(arrows(&spaced, click(0, 4)), Some(-2));
}

#[test]
fn a_prompt_over_two_lines_keeps_its_input_on_the_last() {
    let two = term(20, "\x1b]133;A\x07~/src\r\n> \x1b]133;B\x07make");
    assert_eq!(arrows(&two, click(1, 2)), Some(-4));
    assert_eq!(arrows(&two, click(0, 3)), None, "the prompt's first line");
}

/// The press `term` sends for a click at `at`, as text.
fn sent(term: &Term<VoidListener>, at: Point, relative: bool) -> Option<String> {
    press(term, at, relative).map(|bytes| String::from_utf8(bytes).expect("ASCII"))
}

#[test]
fn a_shell_that_takes_clicks_hears_the_press_where_it_landed() {
    let marked = "\x1b]133;A;click_events=1\x07$ \x1b]133;B\x07";
    let typed = term(20, &format!("out\r\n{marked}echo hi"));
    assert_eq!(
        sent(&typed, click(1, 7), false).as_deref(),
        Some("\x1b[<0;8;2M")
    );
    assert_eq!(
        sent(&typed, click(1, 7), true).as_deref(),
        Some("\x1b[<0;8;1M")
    );
    let below = sent(&typed, click(2, 3), true);
    assert_eq!(
        below.as_deref(),
        Some("\x1b[<0;4;2M"),
        "a row below goes too"
    );
    assert_eq!(sent(&typed, click(0, 1), false), None, "before the prompt");
    assert_eq!(sent(&typed, click(1, 0), false), None, "on the prompt");
    assert_eq!(
        sent(&typed, click(1, 1), false),
        None,
        "on the prompt's space"
    );
    let past = sent(&typed, click(1, 15), false);
    assert_eq!(
        past.as_deref(),
        Some("\x1b[<0;16;2M"),
        "past the end, the shell decides"
    );
    let right = term(
        20,
        &format!("{marked}ls\x1b7\x1b[1;17H\x1b]133;A\x07~/a\x1b]133;B\x07\x1b8"),
    );
    assert_eq!(
        sent(&right, click(0, 18), false),
        None,
        "a right prompt after the cursor"
    );
    assert_eq!(
        sent(&right, click(0, 3), false).as_deref(),
        Some("\x1b[<0;4;1M")
    );
    let two = term(
        20,
        "\x1b]133;A;click_events=2\x07~/src\r\n> \x1b]133;B\x07ls",
    );
    let relative = sent(&two, click(1, 3), true);
    assert_eq!(
        relative.as_deref(),
        Some("\x1b[<0;4;2M"),
        "from the prompt's first row"
    );
    let full = term(20, &format!("{marked}vim\x1b[?1049h"));
    assert_eq!(
        sent(&full, click(0, 3), false),
        None,
        "the alternate screen"
    );
    let scrolled = term(20, &format!("{marked}a\r\nb\r\nc\r\nd\r\ne"));
    assert_eq!(
        sent(&scrolled, click(-1, 0), false),
        None,
        "a row in the history"
    );
}

#[test]
fn input_reads_the_line_its_prompt_started() {
    assert_eq!(Shell::Busy.reads(), Shell::Reading(Click::Arrows));
    assert_eq!(
        Shell::Prompting(Click::Absolute).reads(),
        Shell::Reading(Click::Absolute)
    );
    assert_eq!(
        Shell::Reading(Click::Relative).reads(),
        Shell::Reading(Click::Relative)
    );
}
