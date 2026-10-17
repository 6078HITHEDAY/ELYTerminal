use alacritty_terminal::{
    event::VoidListener,
    index::Column,
    term::{Config, test::TermSize},
    vte::ansi::Processor,
};

use super::*;

/// What the parser gets from `chunks`, each one read.
fn passed(chunks: &[&[u8]]) -> Vec<u8> {
    let mut prompts = Prompts::default();
    let mut out = Vec::new();
    for chunk in chunks {
        let held = prompts.held();
        let mut buf = vec![0; held + chunk.len()];
        buf[held..].copy_from_slice(chunk);
        let count = prompts.pass(&mut buf, chunk.len());
        out.extend_from_slice(&buf[..count]);
    }
    out
}

#[test]
fn a_mark_becomes_a_link_of_the_same_length() {
    assert_eq!(passed(&[b"\x1b]133;A\x07"]), b"\x1b]8;;\x7f\0\x07");
    assert_eq!(passed(&[b"\x1b]133;N\x07"]), b"\x1b]8;;\x7f\0\x07");
    assert_eq!(passed(&[b"\x1b]133;B\x1b\\"]), b"\x1b]8;;\0\0\x1b\\");
    assert_eq!(passed(&[b"\x1b]133;D;1\x07"]), b"\x1b]8;;\0\0\0\0\x07");
    assert_eq!(
        passed(&[b"\x1b]133;A;k=i\x07x"]),
        b"\x1b]8;;\x7f\0\0\0\0\0\x07x"
    );
    assert_eq!(passed(&[b"\x1b]1\x0033;C\x07"]), b"\x1b]8\x00;;\0\0\x07");
}

#[test]
fn a_mark_split_across_reads_waits_for_its_letter() {
    let whole: &[u8] = b"a\x1b]133;A;k=i\x07$ \x1b]133;B\x1b\\b";
    let once = passed(&[whole]);
    for cut in 0..=whole.len() {
        let (first, second) = whole.split_at(cut);
        assert_eq!(passed(&[first, second]), once, "cut at {cut}");
    }
    let bytes: Vec<&[u8]> = whole.chunks(1).collect();
    assert_eq!(passed(&bytes), once);
}

#[test]
fn other_sequences_pass_as_they_came() {
    for bytes in [
        &b"\x1b]1337;A\x07"[..],
        b"\x1b]13;A\x07",
        b"\x1b]133;P;k=i\x07",
        b"\x1b]133;L\x07",
        b"\x1b]133;\x07",
        b"\x1b]133\x18;A\x07",
        b"\x1b]0;133;A\x07",
        b"\x1b[31m\x1bc",
        b"text",
    ] {
        assert_eq!(
            passed(&[bytes]),
            bytes,
            "{}",
            String::from_utf8_lossy(bytes)
        );
    }
}

#[test]
fn a_mark_s_start_waits_no_longer_than_its_cap() {
    let mut long = b"\x1b]".to_vec();
    long.extend([0u8; MOST]);
    assert_eq!(passed(&[&long]), long);
}

/// A terminal `columns` wide and two lines high after `bytes` passed through the rewrite.
fn parsed(bytes: &[u8], columns: usize) -> Term<VoidListener> {
    let mut term = Term::new(Config::default(), &TermSize::new(columns, 2), VoidListener);
    let mut parser: Processor = Processor::new();
    parser.advance(&mut term, &passed(&[bytes]));
    term
}

#[test]
fn a_prompt_s_cells_carry_its_link_and_its_input_none() {
    let term = parsed(b"x\r\n\x1b]133;A\x07\x1b[31m$\x1b[0m \x1b]133;B\x07ls", 8);
    let cell = |line, column| prompt_link(&term.grid()[Line(line)][Column(column)]);
    assert!(cell(0, 0).is_none());
    assert!(
        cell(1, 0).is_some(),
        "a prompt keeps its link through SGR 0"
    );
    assert_eq!(cell(1, 0), cell(1, 1));
    assert!(cell(1, 2).is_none(), "input");
    assert!(starts(&term, Line(1)));
    assert!(!starts(&term, Line(0)));
}

#[test]
fn a_prompt_over_two_rows_starts_at_its_first() {
    let term = parsed(b"\x1b]133;A\x07~/dir\r\n> \x1b]133;B\x07", 8);
    assert!(starts(&term, Line(0)));
    assert!(!starts(&term, Line(1)), "the same prompt goes on");
    let next = parsed(b"\x1b]133;A\x07$ \x1b]133;B\x07\r\n\x1b]133;A\x07$ ", 8);
    assert!(
        starts(&next, Line(1)),
        "an empty line's next prompt is its own"
    );
}

/// Prompts on lines 0, 3 and 6 of a history four lines high, the last on the screen.
fn history() -> Term<VoidListener> {
    let mut bytes = Vec::new();
    for _ in 0..3 {
        bytes.extend_from_slice(
            b"\x1b]133;A\x07$ \x1b]133;B\x07c\r\n\x1b]133;C\x07o\r\no\r\n\x1b]133;D;0\x07",
        );
    }
    bytes.extend_from_slice(b"\x1b]133;A\x07$ \x1b]133;B\x07");
    let mut term = Term::new(Config::default(), &TermSize::new(8, 4), VoidListener);
    let mut parser: Processor = Processor::new();
    parser.advance(&mut term, &passed(&[&bytes]));
    term
}

#[test]
fn up_and_down_go_to_the_prompts_beside_the_view_s_top() {
    let mut term = history();
    assert_eq!(term.grid().history_size(), 6);
    // From the bottom: the prompt at the view's top (line 0) is passed, the one before it found.
    assert_eq!(jump(&term, true), Some(3));
    term.scroll_display(Scroll::Delta(3));
    assert_eq!(jump(&term, true), Some(6));
    term.scroll_display(Scroll::Delta(3));
    assert_eq!(jump(&term, true), None, "no prompt above the first");
    assert_eq!(jump(&term, false), Some(3));
    term.scroll_display(Scroll::Delta(-3));
    assert_eq!(
        jump(&term, false),
        Some(0),
        "a prompt on the screen means the bottom"
    );
}

#[test]
fn a_prompt_keeps_its_start_through_a_resize() {
    let bytes = b"\x1b]133;A\x07~/folder $ \x1b]133;B\x07ls\r\nout\r\n\x1b]133;A\x07$ ";
    let mut term = Term::new(Config::default(), &TermSize::new(12, 4), VoidListener);
    let mut parser: Processor = Processor::new();
    parser.advance(&mut term, &passed(&[bytes]));
    term.resize(TermSize::new(6, 4));
    let lines = term.topmost_line().0..=term.bottommost_line().0;
    let starts: Vec<i32> = lines.filter(|&line| starts(&term, Line(line))).collect();
    // The first prompt wraps over two rows and starts at its first; the last starts two below.
    let first = starts.first().copied().expect("a prompt");
    assert_eq!(starts.len(), 2, "{starts:?}");
    assert!(!starts.contains(&(first + 1)), "{starts:?}");
}
