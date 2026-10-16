# Acceptance

`PRD.md` line by line, as of T23. Status: **Pass** when a test, a run or a review shows it; **Partial** when part of it was shown; **Open** when nothing has shown it yet. Reviews are in `tasks/reviews.md`; tests are named by module (`cargo test -- MODULE`).

What no check reached: no one has looked at the window on Windows, which CI builds and tests; no real KWin or Wayland desktop drew the blur. On 2026-10-15 macOS gave `shot` the window's ground alone, for old and new builds alike, while the terminal it ran under lacked Screen Recording (`CGPreflightScreenCaptureAccess` false); that day's macOS checks read the app's log.

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
| CI on Gitea Actions: macOS builds, tests and lints; Ubuntu and Windows build and test | T01, T23 | `.github/workflows/ci.yml`; run 926 on `65bb4d3`, see the CI notes | Pass |
| The repo, private until v0.1.1 and public since; no credential in any file or commit | T01, T26 | T22's review scanned the tree and its 398 reachable objects: 0 hits; T23's scans for the Gitea token and the runners' SSH details: 0 hits; before the owner made it public, a scan of all 91 commits' contents and messages found no token, SSH password, gateway, port, SSH user or private key | Pass |

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
| On Omarchy 4, an Omarchy theme wears the current theme and follows each switch; the default there; another pick stops following | T27 | `omarchys_ghostty_conf_reads_without_its_glass`, `omarchys_file_is_worn_again_once_it_reads`; on X11, as a user, with Omarchy v4.0.4's own renderer staging and swapping the folder as `omarchy-theme-set` does: with no theme saved the window wore Omarchy's, and through all 22 of its themes the terminal's ground matched each background within 1.5 s; Omarchy's glass lines changed nothing; a file without palette 7 was logged and the last theme stayed; a file swapped in unreadable was worn once made readable, its stamp unchanged; a folder unreadable for 2 s kept the theme; each failure was logged once while it lasted; a picked Catppuccin Mocha stayed through a switch, and with it worn a bad Omarchy file was logged and the start went on; with Omarchy's folder unreadable the start stopped, a theme saved or not, and with settings wearing it and no Omarchy too, each naming the file and the error; the first release build, on the owner's Omarchy 4.0.4 under Hyprland 0.56.2, wore the live theme, sepia-night, which is none of Omarchy's 22, and the release build from `aae7fba`, under Xvfb on Debian 13 with that theme's folder, painted its background | Pass |

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
| Columns toggle and keep their state | T11, T23 | a click on each title bar button hid its column and saved it, and a restart opened the window with both hidden (147 columns against 87); Cmd-B with the app active; Ctrl-Shift-B on a real X11 window | Pass |
| The title shows the session's title, nothing else | T11 | log: "the title is", per session shown | Pass |
| The pointer over the terminal is the arrow | T26 | the terminal sets no pointer, so gpui's arrow shows; on X11 with the pointer over the grid, gpui asked for no I-beam ("text"), which it had asked for, and failed to load under Xvfb, before | Pass |
| The title bar is transparent | T11, T19 | T19 review | Pass |
| Every title bar control takes its press, Ely's window buttons on Windows and Linux too | T26 | Ely `cdc5ce8`: `window_controls_occlude_what_lies_under` (each style; before, the box under Windows' caption buttons took the press), `a_wobbling_press_on_a_slot_moves_no_window` (before, the window moved), `a_wobbling_press_on_a_column_title_moves_the_window`; on the owner's PC, window messages asked the app for Windows' hit test along the title bar: v0.1.0 answered caption from end to end, T26's build answers client over the toggles and +, caption over the titles, min, max and close over Ely's buttons, and a press on close closed the app; the owner, given v0.1.1's build and these controls to click, reported it works on Windows | Pass |

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
| A switch at the top, centered below the title bar: Git, Files | T13, T26 | T13 review; a capture on X11 shows it centered below the title bar, the right toggle and Ely's window buttons in the bar above | Pass |
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
| Cmd-, opens it; categories left, rows right, each scrolling on its own; changes apply at once and save | T16, T26 | T16 review; on X11, eight wheel notches over the Terminal page carried it down to its Edit and View shortcuts while the category column and both headers stayed | Pass |
| On Windows and Linux, a gear in the title bar opens it too | T26 | on X11 under Xvfb with xfwm4, xdotool's click on the gear, just inside the right toggle, opened the 900×640 Settings window ("settings: the window opens"), after a click on + opened a session; sent through `App::dispatch_action`, the same click had opened nothing; the owner, given v0.1.1's build and the gear to click, reported it works on Windows | Pass |
| Terminal: Shell, Start in | T16, T26 | `settings::tests`; the General page is gone, and a capture on X11 shows its two rows leading Terminal | Pass |
| Appearance: theme grids, dark theme, font, size, line height, ligatures, cursor and preview, opacity, blur, padding | T17 | T17 review | Pass |
| On Omarchy, the Omarchy theme first in the grid | T27 | on X11 with Omarchy's theme folder, the Omarchy card leads the grid, chosen, over a preview in its colors | Pass |
| Terminal: scrollback, Option as Alt (macOS), copy on select | T18 | T18 review | Pass |
| Terminal: every shortcut, with a search | T26 | read from the keymap, the bindings that apply in the main window's terminal; on X11 the list showed this platform's keys (Ctrl+Shift+T, Alt+1 and on), and typing "paste" left Edit's Paste, Ctrl+Shift+V | Pass |

## 11. Background blur

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| macOS: `Blurred` (NSVisualEffectView) | T19, T23 | `shot` reads `BlurredView, blur 30` off the view's backdrop layer; the owner saw the desktop blur | Pass |
| Windows: `Blurred` (acrylic) | T19 | the same gpui call; on 2026-10-15 the owner saw the window blur on Windows | Pass |
| Wayland: `ext_background_effect_v1` beside gpui's `org_kde_kwin_blur` | T20 | `blur::wayland` tests on a `wayland-server` compositor; no real compositor | Partial |
| X11: `_KDE_NET_WM_BLUR_BEHIND_REGION`, removed when blur turns off | T21 | `blur::x11` test on Xvfb; the property on a real xfwm4 window; KWin not run | Partial |
| Neither protocol: transparency without blur; the log names the path | T20, T21 | the log lines in `src/blur` | Pass |

## 12. Menus and shortcuts

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| The macOS menu bar | T22 | `shot` lists every item with its key | Pass |
| The same shortcuts off macOS, Ctrl-Shift where they collide | T22 | xdotool on a real X11 window: select all, size, toggle, find, quit | Pass |
| On Windows Ctrl-V pastes too, as in Windows Terminal | T26 | the binding is Windows' alone; on a real X11 window, with a command on the clipboard through xclip, Ctrl-Shift-V pasted its 33 characters and the shell ran it, and Ctrl-V pasted nothing, as Linux keeps it for programs; the owner, given v0.1.1's build and Ctrl-V to press, reported it works on Windows | Pass |

## 13. Acceptance

| PRD line | Evidence | Status |
|---|---|---|
| Each task ticked with a passing review | each task is ticked only after its review, as `tasks/reviews.md` records; T04 and T10 reached the three-round cap without a passing round, their last fixes standing on red-green tests | Partial |
| `scripts/check.sh` passes | macOS: 135 tests, clippy, fmt, 500 lines; Linux through `scripts/linux.sh`: 138 tests, clippy, on aarch64 in Colima and on x86_64 in WSL's Docker | Pass |
| CI green on macOS, Ubuntu and Windows | run 955 on `43ea5b1`, v0.1.1's commit: all three jobs pass; see the CI notes below | Pass |

## 14. Icon and the macOS app

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| The icon: Ely, a girl with long pink hair, holding a terminal | T24 | `assets/icons`, drawn at 16 to 1024 px and legible at 32; the owner saw it | Pass |
| macOS: the rounded square; Windows and Linux: the figure alone, flat | T24 | `elyterminal-macos.svg` on Apple's grid; `elyterminal.svg`, which the Linux tarball carries and the Windows `.ico` renders | Pass |
| `scripts/mac-app.sh` builds the app, signed ad hoc, or for a release with Developer ID and notarized, and its disk image | T24, T26 | its run: `codesign --verify --strict` passes on the app and on its copy in the image; the `.icns` holds 16 to 1024 px; the image verifies and mounts with the app beside Applications; LaunchServices opens the app as `com.zacharyzhang.elyterminal`; for v0.1.1 the Release workflow (run 959) ran it on `macos-signing`: notarization Accepted, the ticket stapled, and on this Mac the downloaded image and its app pass `codesign --verify --strict` under the Developer ID and Gatekeeper's `accepted, source=Notarized Developer ID`; the owner opened it; without the key the script stops before building | Pass |

## 15. Release

| PRD line | Task | Evidence | Status |
|---|---|---|---|
| A Gitea release, `vVERSION`, with the macOS disk image, a Windows zip and a Linux tarball | T25, T26, T27 | `v0.1.1` at `43ea5b1`: the zip from its script on the owner's PC, the disk image from the Release workflow, signed and notarized; the Linux tarball, rebuilt for T27 from `aae7fba` by its script on the owner's PC under Omarchy, took the first one's place, and the notes say so; each downloaded back anonymously, once the repository was public, matches the SHA-256 in the release notes. `v0.1.0` at `0075667`, its three files built from that commit by their scripts and checked again: the disk image verifies and its app passes `codesign --verify --strict`; the zip's executable has the icon and no C runtime DLL; the tarball opens its window on Ubuntu 22.04; each file downloaded back matches the SHA-256 in the release notes | Pass |
| The Windows executable carries the icon | T25 | its resources: icon group 1 with the seven images of `elyterminal.ico`, 16 to 256 px, and gpui's manifest (PerMonitorV2); a GUI program, importing no C runtime DLL; not yet seen on a Windows screen | Pass |
| The Linux tarball carries the icon and a desktop entry; the window names itself `com.zacharyzhang.elyterminal` | T25, T27 | on Ubuntu 22.04: `desktop-file-validate` reports nothing, every library resolves, glibc 2.35; under Xvfb `WM_CLASS` reads `com.zacharyzhang.elyterminal` twice, and under headless sway the window's `app_id` reads the same; T27's rebuild passes the same checks on Ubuntu 22.04, whose llvmpipe (LLVM 15) leaves the window black in a capture, the first tarball's too, while Debian 13's paints it | Pass |
| Each build comes from its script | T25, T27 | `scripts/mac-app.sh` on macOS, `scripts/windows-zip.ps1` on the owner's Windows PC, `scripts/linux-tar.sh` in the owner's WSL, and for T27 on the owner's PC under Omarchy | Pass |

## CI notes

On 2026-10-15 every job first failed on its runner, not on the code. With the owner's leave, each runner was put right:

- macOS: fish and PowerShell were missing, so the two shell tests failed, as they should without their shells. Homebrew installed both.
- Windows: rustup's `.cargo\bin` held `rustup.exe` alone, without the `cargo.exe` and `rustc.exe` proxies, so the toolchain step stopped at `rustc: command not found`. rustup's installer, run again, put them back.
- Ubuntu: no runner carried `ubuntu-24.04`, so 71 runs waited; they were cancelled, and a runner for this repository alone runs the job in Docker, in a VM of its own. Its first build was killed linking gpui's test binaries with debuginfo in 8 GB, so the Linux and Windows jobs build without debuginfo, as `scripts/linux.sh` does.

A clippy run for `x86_64-pc-windows-msvc` on macOS, with stand-ins for the MSVC assembler and `llvm-rc`, found `testlog` unused on Windows; it now builds only beside the Unix tests that use it.

Run 907 then failed the PowerShell test on macOS: it typed its probe right after the first folder report, which pwsh 7.6.6 sends before PSReadLine reads its first key, and the line was echoed, never run. The shell tests now type a line only once the line editor reads raw keys.

Run 926, on `65bb4d3`, passed all three jobs: macOS ran `scripts/check.sh` (135 tests, the fonts check, clippy, fmt, the 500-line rule) and the build; Ubuntu ran 138 tests and the build; Windows ran 113 tests and the build, without the Unix-only pty and shell tests.

After run 928 cargo failed on the Windows PC: `Swatinem/rust-cache`'s post step had deleted `rustup` from the host's `~/.cargo/bin`, on the Mac mini too, leaving the proxies pointing at nothing. The cache step now runs only in the Linux container. On the hosts, `rustup toolchain install --no-self-update` reads `rust-toolchain.toml` in place of dtolnay's action, which ran `rustup default`, so CI leaves their default toolchain alone. Run 934, on `0075667`, passed all three jobs, each on 1.95.0 "overridden by rust-toolchain.toml".

## The owner's checks

On 2026-10-15 the owner ran the app on macOS and found three faults, now fixed:

- The desktop showed through sharp: on macOS 27 gpui's `Selection` material builds no blur; the window's view now takes `underWindowBackground`, radius 30.
- The debug build lagged; the release build does not.
- No button hid the side columns; each now has one at its end of the title bar, its state saved.
