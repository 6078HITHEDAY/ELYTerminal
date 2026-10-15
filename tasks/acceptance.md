# Acceptance

`PRD.md` line by line, as of T23. Status: **Pass** when a test, a run or a review shows it; **Partial** when part of it was shown; **Open** when nothing has shown it yet. Reviews are in `tasks/reviews.md`; tests are named by module (`cargo test -- MODULE`).

What no check reached: no build has run on Windows; no real KWin or Wayland desktop drew the blur. On 2026-10-15 macOS gave `shot` blank pictures for old and new builds alike, so that day's macOS checks read the app's log.

## 1. Product

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| One window: sessions left, terminal middle, Git or Files right | T11 | T11 review; a real X11 window shows the terminal and the right column | Pass |
| Every setting has a control in the settings window | T16–T18 | 17 of the 20 fields have a row; `show_sessions`, `show_panel` and `panel` are the main window's toggles and switch, which §6 keeps in the file | Pass |
| Silent: no sound, bell, notification, update check, telemetry, tip or onboarding | T10, T12 | a bell is logged and marks a hidden row; the code holds no network call or notification | Pass |
| Clean: no subtitle, tagline, logging option, status bar or badge without state | T11–T18 | reviews of the layout and settings tasks | Pass |
| Built-in themes, each with a preview, Catppuccin among them; a theme colors terminal and chrome | T03, T17 | `themes::tests`; T17 review | Pass |
| The background blurs on macOS, Windows, Wayland and X11 | T19–T21 | see §11 | Partial |

## 2. Foundation

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| One binary crate `elyterminal`, edition 2024, Rust 1.95 | T01 | `Cargo.toml` | Pass |
| Ely by pinned rev; gpui and gpui_platform from `gpui-pre` at Ely's version | T01 | `Cargo.toml`: Ely `a2cb531`, `gpui-pre` `=0.3.8` | Pass |
| `alacritty_terminal` 0.26 | T04 | `=0.26.0` | Pass |
| Logs through `log` to stderr, `RUST_LOG` sets the level, no log setting | T01 | `src/main.rs`; no such field in `Settings` | Pass |
| CI on Gitea Actions: macOS builds, tests and lints; Ubuntu and Windows build and test | T01, T23 | `.github/workflows/ci.yml`; see §13 | Open |
| Private repo; no credential in any file or commit | T01 | each review scans the tree and every reachable object: 0 hits (T22: 398 objects) | Pass |

## 3. Settings model and storage

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| One `settings.json` in `dirs::config_dir()/ELYTerminal` | T02 | `settings::store` tests | Pass |
| Every field has a default; a missing file or field means its default | T02 | `settings::tests`, `settings::store` tests | Pass |
| A bad file or value stops the start, naming file and field; never overwritten | T02 | `a_bad_file_is_an_error_naming_it`, `a_bad_value_is_an_error_naming_the_field` | Pass |
| Temp file in the same folder, then rename; one writer | T02 | T02 rounds 1–2 | Pass |

## 4. Themes

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| The 14 built-in themes | T03 | `assets/themes`, byte for byte with upstream `21f281d`; Ely's two | Pass |
| Background, foreground, cursor, cursor text, selection, 16 ANSI from upstream | T03 | `themes::tests` | Pass |
| A theme maps onto Ely's `Palette` through `Theme::set_palette` | T03, T19 | `the_chrome_wears_the_theme` | Pass |
| One theme, or a second for dark mode | T17 | T17 review | Pass |

## 5. Terminal engine

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| PTY through alacritty's event loop; OSC 7 scanned on the reader | T04 | `term::pty`, `term::tap` tests; round 3 failed at the cap, its fix stands on a red-green test | Pass |
| Shell integration for zsh, bash, fish, PowerShell; a shell without it reports nothing and the panels say so | T05 | `term::shell` tests on macOS and Linux | Pass |
| Colors, styles, five underlines, wide characters, combining marks | T06 | `term::frame`, `term::ink`, `term::paint` tests | Pass |
| Box drawing, blocks and powerline shapes as geometry | T07 | `term::boxes` tests | Pass |
| JetBrains Mono, any installed font, Nerd Font icons | T06, T17 | `tests/fonts.rs` | Pass |
| Cursor shapes, blink by setting and DECSCUSR, hollow unfocused | T06 | T06 review | Pass |
| xterm keys, cursor mode, Option as Alt, kitty protocol, keypad | T08 | `term::keys` tests | Pass |
| IME: marked text at the cursor, commits to the program | T08 | T08 review | Pass |
| Selection by character, word, line; Shift extends; mouse reporting; wheel | T09 | `term::mouse`, `term::tap` tests | Pass |
| Copy, paste, bracketed paste, copy on select, OSC 52 | T09 | T09 review | Pass |
| OSC 8 and plain links on Cmd-press, Ctrl-press off macOS | T09 | `term::links` tests | Pass |
| Focus reporting, title, scrollback limit, find | T10 | `term::find` tests; round 3 failed at the cap, its fix stands on a red-green test | Pass |
| A bell makes no sound; the row marks it | T10, T12 | T12 review | Pass |

## 6. Main window and layout

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| `ColumnShell` with three columns; blur when the setting is on | T11, T19 | `shot` names a `BlurredView` under the window | Pass |
| Columns toggle and keep their state | T11 | Cmd-B, the app active, saved `show_sessions` false; Ctrl-Shift-B hid the sessions on a real X11 window | Pass |
| The title shows the session's title, nothing else | T11 | log: "the title is", per session shown | Pass |
| The title bar is transparent | T11, T19 | T19 review | Pass |

## 7. Sessions panel

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| Rows: title, then the folder, home as `~` | T12 | `sessions::tests` | Pass |
| New (+, Cmd-T), close (×, Cmd-W), select (press, Cmd-1…9, Ctrl-Tab) | T12 | 2026-10-15: + opened session 2, a press showed session 1 again | Pass |
| A program's end closes its row; the last row closes the window | T12 | T12 review | Pass |
| A bell in a hidden session marks its row | T12 | T12 review | Pass |

## 8. Right panel: Files

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| Git and Files switch | T13 | T13 review | Pass |
| The session's folder as a tree, folders first, Git colors | T13 | `files::tests` | Pass |
| Double press opens a folder, or a file in the default app | T13 | T13 review, with a logging `open` stub | Pass |
| Reloads on a new folder and at each prompt | T13 | log: "the tree shows", per folder reported | Pass |

## 9. Right panel: Git

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| Through the `git` command line | T14 | `git::tests` on temp repos | Pass |
| Branch, staged and unstaged, commit field and Commit, recent commits | T15 | `git::panel` tests; T15 review | Pass |
| Stage and unstage; a press opens the diff in `DiffViewer` | T15 | T15 review | Pass |
| Discard asks first | T15 | T15 review, in temp repos | Pass |
| Outside a repository it says so; a failed command shows its error | T15 | "Not a Git repository." on a real X11 window | Pass |
| Reloads at each prompt and on a folder change | T15 | log: "git panel: shows", per folder | Pass |

## 10. Settings window

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| Cmd-, opens it; categories left, rows right; changes apply at once and save | T16 | T16 review | Pass |
| General: Shell, Start in | T16 | `settings::tests` | Pass |
| Appearance: theme grids, dark theme, font, size, line height, ligatures, cursor and preview, opacity, blur, padding | T17 | T17 review | Pass |
| Terminal: scrollback, Option as Alt (macOS), copy on select | T18 | T18 review | Pass |

## 11. Background blur

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| macOS: `Blurred` (NSVisualEffectView) | T19 | `BlurredView` comes and goes with the settings | Pass |
| Windows: `Blurred` (acrylic) | T19 | the same gpui call; no Windows build has run | Open |
| Wayland: `ext_background_effect_v1` beside gpui's `org_kde_kwin_blur` | T20 | `blur::wayland` tests on a `wayland-server` compositor; no real compositor | Partial |
| X11: `_KDE_NET_WM_BLUR_BEHIND_REGION`, removed when blur turns off | T21 | `blur::x11` test on Xvfb; the property on a real xfwm4 window; KWin not run | Partial |
| Neither protocol: transparency without blur; the log names the path | T20, T21 | the log lines in `src/blur` | Pass |

## 12. Menus and shortcuts

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| The macOS menu bar | T22 | `shot` lists every item with its key | Pass |
| The same shortcuts off macOS, Ctrl-Shift where they collide | T22 | xdotool on a real X11 window: select all, size, toggle, find, quit | Pass |

## 13. Acceptance

| PRD line | Evidence | Status |
|---|---|---|
| Each task ticked with a passing review | T01–T22 ticked; T04 and T10 reached the three-round cap without a passing round; T23 open | Partial |
| `scripts/check.sh` passes | macOS: 135 tests, clippy, fmt, 500 lines; Linux in Colima: 138 tests, clippy | Pass |
| CI green on macOS, Ubuntu and Windows | see the CI notes below | Open |

## CI notes

As probed on 2026-10-15, each job fails on its runner, not on the code:

- macOS, on the Mac mini runner: Homebrew is on the PATH, but fish and PowerShell are not installed, so `fish_loads_its_script_and_restores_its_data_dirs` and `powershell_reports_its_folder_byte_for_byte_and_keeps_the_status` fail, as they should without their shells.
- Windows, on its runner: `C:\Users\zhang\.cargo\bin` holds `rustup.exe` alone, without the `cargo.exe` and `rustc.exe` proxies, so the toolchain step stops at `rustc: command not found`. `cargo clippy --target x86_64-pc-windows-msvc --all-targets -D warnings` passes on macOS, with a stand-in for the MSVC assembler and `llvm-rc`; no Windows test has run.
- Ubuntu: no runner carries the `ubuntu-24.04` label, so the job never starts. The tests pass in a Linux container (`scripts/linux.sh test --locked`).
