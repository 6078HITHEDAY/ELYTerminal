# AGENTS.md

ELYTerminal. A terminal on GPUI and Ely components. `PRD.md` says what it does; `TASKS.md` says how far it is.

## Stack

- Rust 1.95, edition 2024. One binary crate, `elyterminal`.
- Ely (`ely-gpui-component`) from git.zacharyzhang.com, pinned by rev. gpui and gpui_platform from crates.io's `gpui-pre`, the exact version Ely pins, under their crate names through `package`.
- `alacritty_terminal` 0.26: grid, parser, pseudo-terminal, event loop.
- `serde`, `serde_json`: settings. `dirs`: the config folder. `log`, `env_logger`: logs to stderr.

## Commands

- `cargo run` opens the app. `RUST_LOG=elyterminal=debug cargo run` logs more.
- `scripts/check.sh`: fmt, clippy `-D warnings`, tests, the 500-line rule.

## Layout

- `src/main.rs`: start, window. `src/lib.rs`: the modules, public so tests reach them.
- `src/settings`: the model, its ranges, and `SettingsStore`, the global that loads, checks and saves `settings.json`.
- `src/themes`: built-in terminal themes, the chrome palette each maps to, and `apply`, which dresses the app for the system's appearance.
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
- 2026-10-14: settings refuse unknown fields; a bad settings file stops the start and is never overwritten.
