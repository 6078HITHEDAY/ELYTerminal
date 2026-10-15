# Tasks

Progress truth. Top to bottom, one at a time. A task is ticked only after its codex review passes; rounds go in `tasks/reviews.md`, three at most.

## 2. Foundation

- [x] T01 Repo, crate, window: git, `.gitignore`, `Cargo.toml` with Ely pinned, `main.rs` opens one window through Ely, logging, `scripts/check.sh`, CI workflow, private Gitea repo, first push.

## 3. Settings model and storage

- [x] T02 `Settings` with defaults and ranges; load (missing file, missing field, bad file, bad value) and atomic save; tests.

## 4. Themes

- [x] T03 Built-in themes from upstream files; mapping onto Ely's `Palette`; dark-mode choice; tests.

## 5. Terminal engine

- [x] T04 PTY: spawn through alacritty's event loop, OSC 7 tap on the reader, child id, events to the app; tests.
- [x] T05 Shell integration for zsh, bash, fish, PowerShell, injected at launch; tests that zsh and bash report their folder.
- [x] T06 Terminal view: frame from the grid, colors and styles, wide and combining cells, cursor shapes and blink, font, line height, padding, resize.
- [x] T07 Box drawing, block elements and powerline shapes (U+E0B0–E0B7) painted as geometry; tests.
- [x] T08 Keys: xterm encoding, cursor mode, Option as Alt, kitty keyboard protocol, IME; tests.
- [x] T09 Mouse: selection, mouse reporting, wheel and scrollback, clipboard, copy on select, OSC 52, links; tests.
- [x] T10 Focus reporting, title, silent bell, scrollback limit, find.

## 6. Main window and layout

- [x] T11 `ColumnShell` with sessions, terminal and right columns; toggles kept in settings; window title; a transparent title bar over the window's background, as Ghostty's.

## 7. Sessions panel

- [x] T12 Sessions: new, close, select, keys, titles and folders, ended programs, bell marks.

## 8. Right panel: Files

- [x] T13 Git/Files switch; file tree of the session's folder with Git colors; open; reload on folder and prompt.

## 9. Right panel: Git

- [x] T14 Git through the command line: status, branch, stage, unstage, commit, log, diff; tests on a temp repo.
- [x] T15 Git panel: branch, changes, stage and unstage, commit, recent commits, diff view, errors.

## 10. Settings window

- [x] T16 Settings window: categories, General rows, live apply and save.
- [x] T17 Appearance: theme grids with previews, font, cursor with preview, opacity, blur, padding.
- [x] T18 Terminal rows: scrollback, Option as Alt, copy on select.

## 11. Background blur

- [x] T19 macOS and Windows: `Blurred` or `Transparent` or `Opaque` from settings, applied live.
- [x] T20 Wayland: `ext_background_effect_v1` on the window's surface.
- [ ] T21 X11: `_KDE_NET_WM_BLUR_BEHIND_REGION` on the window.

## 12. Menus and shortcuts

- [ ] T22 macOS menu bar; key bindings on all platforms.

## 13. Acceptance

- [ ] T23 End-to-end checks, README, acceptance table, CI green on three platforms.
