# AGENTS.md

ELYTerminal. A terminal on GPUI and Ely components. `PRD.md` says what it does; `TASKS.md` says how far it is.

## Stack

- Rust 1.95, edition 2024. One binary crate, `elyterminal`.
- Ely (`ely-gpui-component`) from git.zacharyzhang.com, pinned by rev. gpui and gpui_platform from crates.io's `gpui-pre`, the exact version Ely pins, under their crate names through `package`.
- `alacritty_terminal` 0.26: grid, parser, pseudo-terminal, event loop.
- `serde`, `serde_json`: settings. `dirs`: the config folder. `log`, `env_logger`: logs to stderr.

## Commands

- `cargo run` opens the app. `RUST_LOG=elyterminal=debug cargo run` logs more; `trace` logs each paint.
- `cargo run --example shot -- <png> <program> [args…]` runs a program in the main window and writes a PNG of it (macOS; no permission needed for the app's own window).
- `scripts/check.sh`: fmt, clippy `-D warnings`, tests, the 500-line rule.

## Layout

- `src/main.rs`: start. `src/lib.rs`: the modules, public so tests and examples reach them. `src/app.rs`: fonts, settings, integration, then the main window.
- `src/settings`: the model, its ranges, and `SettingsStore`, the global that loads, checks and saves `settings.json`.
- `src/themes`: built-in terminal themes, the chrome palette each maps to, and `apply`, which dresses the app for the system's appearance.
- `src/term`: the terminal engine. `pty` starts a program through alacritty's event loop; `tap` wraps the pty so its reader scans OSC 7 before the parser. `unix` starts the program itself, so a failed `chdir` fails the start; Windows uses alacritty's ConPTY. `shell` writes the integration scripts at start and builds each shell's launch. `frame` reads the grid into styled rows, `boxes` turns box drawing, block elements and powerline arrows into shapes, `ink` resolves colors, `paint` draws rows, decorations and the cursor, `view` is the entity: pty events, metrics, blink, resize.
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
- 2026-10-14: box drawing (U+2500–U+257F), block elements (U+2580–U+259F) and powerline shapes (U+E0B0–U+E0B7) follow Ghostty's sprite rules (MIT, `LICENSE-GHOSTTY`): whole device pixels, line thickness the underline's, each cell clipped to itself, so lines join across cells whatever the font.
- 2026-10-14: settings refuse unknown fields; a bad settings file stops the start and is never overwritten.
