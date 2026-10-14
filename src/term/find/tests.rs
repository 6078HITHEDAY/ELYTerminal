use alacritty_terminal::{
    event::VoidListener,
    term::{Config, test::TermSize},
    vte::ansi::Processor,
};

use super::*;

const PLAIN: FindOptions = FindOptions {
    case: false,
    word: false,
    regex: false,
};

fn term(bytes: &str, columns: usize, lines: usize, history: usize) -> Term<VoidListener> {
    let config = Config {
        scrolling_history: history,
        ..Config::default()
    };
    let mut term = Term::new(config, &TermSize::new(columns, lines), VoidListener);
    feed(&mut term, bytes);
    term
}

fn feed(term: &mut Term<VoidListener>, bytes: &str) {
    let mut parser: Processor = Processor::new();
    parser.advance(term, bytes.as_bytes());
}

fn found(term: &Term<VoidListener>, query: &str, options: FindOptions) -> Found {
    Found::search(term, query, options).expect("a pattern")
}

fn at(line: i32, column: usize) -> Point {
    Point::new(Line(line), Column(column))
}

#[test]
fn find_takes_case_words_and_patterns() {
    let term = term("one two One\r\nthree phone one", 20, 3, 10);
    let count =
        |query: &str, options| Found::search(&term, query, options).map(|found| found.hits.len());
    assert_eq!(count("one", PLAIN), Ok(4));
    assert_eq!(
        count(
            "One",
            FindOptions {
                case: true,
                ..PLAIN
            }
        ),
        Ok(1)
    );
    let word = FindOptions {
        word: true,
        ..PLAIN
    };
    assert_eq!(count("one", word), Ok(3), "phone holds no whole word");
    let regex = FindOptions {
        regex: true,
        ..PLAIN
    };
    assert_eq!(count("t[wh]", regex), Ok(2));
    assert_eq!(count("one$", regex), Ok(2), "a line ends after its text");
    assert_eq!(count("(", PLAIN), Ok(0), "plain text is escaped");
    assert!(count("(", regex).is_err());
}

#[test]
fn marks_wide_cells_and_unicode_words_are_found_where_they_show() {
    let shown = "cafe\u{301} caf\u{e9} \u{4e2d}\u{6587} \x1b[8msecret\x1b[0m";
    let line = term(shown, 30, 2, 10);
    let hits = |query: &str, options| found(&line, query, options).hits;
    assert_eq!(
        hits("cafe\u{301}", PLAIN),
        [at(0, 0)..=at(0, 3)],
        "a mark rides its cell"
    );
    let word = FindOptions {
        word: true,
        ..PLAIN
    };
    assert_eq!(hits("caf\u{e9}", word), [at(0, 5)..=at(0, 8)]);
    assert_eq!(hits("\u{4e2d}\u{6587}", word), [at(0, 10)..=at(0, 12)]);
    assert!(hits("secret", PLAIN).is_empty(), "hidden text stays hidden");
    let after = term("\u{4e2d}x", 10, 2, 10);
    let x = found(&after, "x", PLAIN).hits;
    assert_eq!(
        x,
        [at(0, 2)..=at(0, 2)],
        "a wide cell's spacer holds no text"
    );
}

#[test]
fn a_wrapped_line_is_one_line_mapped_back_to_its_cells() {
    let term = term("12345678needle", 10, 3, 10);
    assert_eq!(found(&term, "needle", PLAIN).hits, [at(0, 8)..=at(1, 3)]);
    let regex = FindOptions {
        regex: true,
        ..PLAIN
    };
    assert_eq!(found(&term, "needle$", regex).hits.len(), 1);
}

#[test]
fn the_current_match_follows_its_line() {
    let mut erased = term("needle OLD\r\nneedle KEEP\r\nneedle LAST", 20, 4, 10);
    let then = found(&erased, "needle", PLAIN);
    feed(&mut erased, "\x1b[1;1H\x1b[2K");
    let now = found(&erased, "needle", PLAIN);
    assert_eq!(now.follow(&then, 1), Some(0), "KEEP, though OLD is gone");

    let mut ring = term("n 1\r\nn 2\r\nn 3\r\nn 4\r\nn 5", 10, 3, 2);
    let then = found(&ring, "n", PLAIN);
    feed(&mut ring, "\r\nn 6");
    let now = found(&ring, "n", PLAIN);
    assert_eq!(
        now.follow(&then, 2),
        Some(1),
        "n 3, a line up, though n 1 left"
    );

    let mut same = term("n\r\nn\r\nn", 10, 3, 10);
    let then = found(&same, "n", PLAIN);
    feed(&mut same, "\r\nx\r\nx");
    let now = found(&same, "n", PLAIN);
    assert_eq!(
        now.follow(&then, 1),
        Some(1),
        "of like lines, the one that scrolled"
    );

    let mut gone = term("needle A\r\nneedle B\r\n\r\nneedle C", 20, 5, 10);
    let then = found(&gone, "needle", PLAIN);
    feed(&mut gone, "\x1b[2;1H\x1b[2K");
    let now = found(&gone, "needle", PLAIN);
    assert_eq!(now.follow(&then, 1), Some(0), "B is gone: A is nearest");

    let mut wide = term(&format!("{}needle", "x".repeat(15)), 12, 4, 10);
    let then = found(&wide, "needle", PLAIN);
    wide.resize(TermSize::new(30, 4));
    let now = found(&wide, "needle", PLAIN);
    assert_eq!(now.follow(&then, 0), Some(0));
    assert_eq!(now.hits[0].start().column, Column(15), "the reflowed cells");
    assert_ne!(now.hits[0].start(), then.hits[0].start());
}

#[test]
fn steps_go_round_the_ends() {
    assert_eq!(stepped(Some(0), 3, false), Some(2));
    assert_eq!(stepped(Some(2), 3, true), Some(0));
    assert_eq!(stepped(Some(1), 3, false), Some(0));
    assert_eq!(stepped(None, 0, true), None);
}
