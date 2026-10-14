# AGENTS.md

ELYTerminal. A terminal on GPUI and Ely components. `PRD.md` says what it does; `TASKS.md` says how far it is.

## Stack

- Rust 1.95, edition 2024. One binary crate, `elyterminal`.
- Ely (`ely-gpui-component`) from git.zacharyzhang.com, pinned by rev. gpui and gpui_platform from crates.io's `gpui-pre`, the exact version Ely pins, under their crate names through `package`.
- `alacritty_terminal` 0.26: grid, parser, pseudo-terminal, event loop.
- `serde`, `serde_json`: settings. `dirs`: the config folder. `log`, `env_logger`: logs to stderr.

## Commands

- `cargo run` opens the app. `RUST_LOG=elyterminal=debug cargo run` logs more; `trace` logs each paint.
- `cargo run --example shot -- [--keys "SPECS"] <png> <program> [args…]` runs a program in the main window, plays SPECS on it, and writes a PNG of the window (macOS; no permission needed for the app's own window). It shows its window without activating the app, so gpui keeps drawing it and keystrokes or clicks meant for other apps stay there. Run it with `HOME` pointing at a temp folder to try settings without touching your own. SPECS, space-separated:
  - keys as real AppKit events: `a`, `ctrl-c`, `lopt-f` and `ropt-f` (left and right Option), `cmd-v`, `repeat-up` (AppKit's repeat flag);
  - input method calls: `mark:é` composes, `commit:é` types;
  - mouse gestures handed to gpui's view, at points from the content's top-left, modifiers as for keys: `click@x,y`, `click2@`, `click3@`, `rclick@`, `mclick@`, `drag@x,y@x,y`, `move@x,y`, the steps `ldown@`, `lup@`, `ldrag@`, `rdown@`, `rup@`, `mdown@`, `mup@`, and `scrollup@x,y@lines`, `scrolldown@x,y@lines`.
- `scripts/check.sh`: fmt, clippy `-D warnings`, tests, the 500-line rule.

## Layout

- `src/main.rs`: start. `src/lib.rs`: the modules, public so tests and examples reach them. `src/app.rs`: fonts, settings, integration, then the main window.
- `src/settings`: the model, its ranges, and `SettingsStore`, the global that loads, checks and saves `settings.json`.
- `src/themes`: built-in terminal themes, the chrome palette each maps to, and `apply`, which dresses the app for the system's appearance.
- `src/term`: the terminal engine. `pty` starts a program through alacritty's event loop; `tap` wraps the pty so its reader scans OSC 7 and the mouse modes before the parser. `unix` starts the program itself, so a failed `chdir` fails the start; Windows uses alacritty's ConPTY. `shell` writes the integration scripts at start and builds each shell's launch. `frame` reads the grid into styled rows, `boxes` turns box drawing, block elements and powerline arrows into rects and polygons (`boxes/outline` strokes curves and diagonals into polygons), `ink` resolves colors, `paint` draws rows, decorations and the cursor, `view` is the entity: pty events, metrics, blink, resize. `keys` turns key events into bytes, the kitty protocol and, in `keys/legacy`, xterm's encoding, with no gpui types; `input` is the view's key listeners, the bindings that keep Tab and F6 from Ely's focus moves, and its IME handler; `mac` reads the Option side and repeat flag of AppKit's current event, keeps held keys repeating, and hands Command key-ups to the focused view. `mouse` encodes mouse reports, with no gpui types; `links` finds the link at a cell; `pointer` is the view's mouse listeners: selection, reports, wheel, links, copy and paste.
- `assets/shell-integration`: ELYTerminal's own scripts (not Ghostty's, which are GPL): zsh through `ZDOTDIR`, bash through `--rcfile`, fish through `XDG_DATA_DIRS`, PowerShell through `-NoExit -Command`. Each prompt sends OSC 7 and, except in fish, whose `fish_title` stays, a title.
- `assets/fonts`: JetBrains Mono 2.304's bold and italic faces (OFL; Ely bundles regular), and Nerd Fonts 3.5.1's JetBrains Mono NF Mono Regular, which draws icons a font lacks.
- `assets/themes`: Ghostty theme files copied verbatim from iTerm2-Color-Schemes (MIT, its `LICENSE` beside them).

## Rules

- Hand-written files stay at or under 500 lines; `Cargo.lock`, which Cargo writes, is exempt. Comments are one short line, and rare.
- Fail fast. No silent fallbacks. An error names its owner and what failed.
- Log every state change on a key path with `log`: spawn, exit, folder change, settings load and save, blur path.
- No dead code, no versioned names, no compatibility shims.
- Colors, sizes and radii come from Ely's theme. Gaps use gpui's rem helpers.
- No credential in any file. Pushes pass the token as an `http.extraHeader` on the command line.

## Decisions

- 2026-10-14: ELYTerminal owns its terminal engine on `alacritty_terminal`. Ely's `Terminal` lacks mouse reporting, IME, kitty keys and style settings, and a library widget should not carry an app's engine.
- 2026-10-14: the working directory comes from OSC 7, read by a tap on the pty reader, sent by injected shell integration. vte drops OSC 7.
- 2026-10-14: Git through the `git` command line, not libgit2.
- 2026-10-14: license MIT, as Ely.
- 2026-10-14: on Unix the pty is ELYTerminal's own: alacritty 0.26 ignores `chdir`'s result, so a session could start in the wrong folder. Its traits feed alacritty's event loop, keyed 0 and 1 as alacritty's private constants are; alacritty is pinned to =0.26.0 for that.
- 2026-10-14: bash integration loads through `--rcfile`, not Ghostty's `--posix` and `ENV`: macOS's bash 3.2 ignores `ENV` there. bash then runs as a non-login shell; on macOS the script reads the login files itself, so `logout` and `~/.bash_logout` do not apply.
- 2026-10-14: icons fall back by hand: a private-use character the font lacks paints in JetBrains Mono NF Mono. gpui's macOS fallbacks go through CoreText's cascade list, which never sees fonts added in memory, and gpui loads no font without an `m`, so the symbols-only Nerd Font cannot load.
- 2026-10-14: box drawing (U+2500–U+257F), block elements (U+2580–U+259F) and powerline shapes (U+E0B0–U+E0B7) follow Ghostty's sprite rules (MIT, `LICENSE-GHOSTTY`): whole device pixels, line thickness the underline's, each cell clipped to itself, so lines join across cells whatever the font. A character's pieces never overlap: gpui blends every quad and every path triangle on its own, so a dim line painted twice brightens where it crosses. A drawn base keeps its geometry under combining marks, which paint over a no-break space.
- 2026-10-14: keys follow Ghostty's encoder (MIT). Keys the program reads go out from the key listener; plain text falls through to the input handler, so IME and dead keys compose. Ctrl+[, Ctrl+I and Ctrl+M go as fixterms CSI u, as in Ghostty. Keypad keys always type their characters: Ghostty keeps mode 1035 on, and gpui names keypad keys as main keys on every platform. Ely binds Tab, Shift-Tab, F6 and Shift-F6 to focus moves, and bindings run before listeners; `NoAction` bindings under the `Terminal` context hide them, so these keys reach the listeners with press, repeat and release.
- 2026-10-14: each kitty flag reshapes only what it names, as kitty's encoder does: functional keys keep legacy bytes until disambiguate, events or all is set, and text keys type until all is set. Cmd is kitty's Super, bit 8; a Cmd combo no binding takes reaches the program.
- 2026-10-14: an input method's commit goes to the program as UTF-8 under every kitty flag, as kitty (`keys.c`, `GLFW_IME_COMMIT_TEXT`) and Ghostty (test `kitty: composed text with report all`) send it.
- 2026-10-14: what gpui hides from the kitty protocol: a key alone on a modifier sends nothing (gpui reports no key event for it), and shifted punctuation reports the character it types as its key, since gpui clears Shift for it.
- 2026-10-14: on macOS, which Option key is down and whether a key repeats come from AppKit's current event: gpui rebuilds keys AppKit routes through its commands as fresh presses. As in Ghostty, `ApplePressAndHoldEnabled` is registered off, so held keys repeat instead of opening the accent menu, and a local event monitor hands Command key-ups to the focused view, since AppKit never routes them to the window.
- 2026-10-14: mouse reports follow Ghostty's rules: X10 tracking sends presses alone without modifiers, SGR names the released button, motion inside one cell sends nothing, Shift takes the mouse back for selecting. The tap follows modes 9, 1000, 1002, 1003, 1005 and 1006 as xterm holds them: one tracking and one format at a time, the last set wins, any reset ends it, and RIS clears both. alacritty drops 9 and keeps each mode apart, so its mouse bits go unread. Motion reports the first held button: left, right, middle.
- 2026-10-14: a wheel notch scrolls 3 lines, as Ghostty's discrete wheel; a trackpad scrolls its pixels. The wheel reports to a program that tracks the mouse, sends arrows on the alternate screen under mode 1007, and scrolls history otherwise.
- 2026-10-14: Cmd-C and Cmd-V copy and paste on macOS, Ctrl-Shift-C and Ctrl-Shift-V elsewhere; Cmd-press, Ctrl-press off macOS, opens an OSC 8 target or a URL Ely's `links` finds in the wrapped line. Copies drop the newline alacritty ends a whole line with, as Ghostty's do. Bracketed pastes lose ESC and Ctrl-C, as alacritty's do; plain pastes end lines with CR. Typing and pasting clear the selection.
- 2026-10-14: a key's release goes out only when the terminal saw its press, so a key a binding takes sends no release, as in kitty.
- 2026-10-14: settings refuse unknown fields; a bad settings file stops the start and is never overwritten.
