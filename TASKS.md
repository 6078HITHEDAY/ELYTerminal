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
- [ ] T05 Shell integration for zsh, bash, fish, PowerShell, injected at launch; tests that zsh and bash report their folder.
- [ ] T06 Terminal view: frame from the grid, colors and styles, wide and combining cells, cursor shapes and blink, font, line height, padding, resize.
- [ ] T07 Box drawing and block elements painted as geometry; tests.
- [ ] T08 Keys: xterm encoding, modes, Option as Alt, kitty keyboard protocol, IME; tests.
- [ ] T09 Mouse: selection, mouse reporting, wheel and scrollback, clipboard, copy on select, OSC 52, links; tests.
- [ ] T10 Focus reporting, title, silent bell, scrollback limit, find.

## 6. Main window and layout

- [ ] T11 `ColumnShell` with sessions, terminal and right columns; toggles kept in settings; window title.

## 7. Sessions panel

- [ ] T12 Sessions: new, close, select, keys, titles and folders, ended programs, bell marks.

## 8. Right panel: Files

- [ ] T13 Git/Files switch; file tree of the session's folder with Git colors; open; reload on folder and prompt.

## 9. Right panel: Git

- [ ] T14 Git through the command line: status, branch, stage, unstage, commit, log, diff; tests on a temp repo.
- [ ] T15 Git panel: branch, changes, stage and unstage, commit, recent commits, diff view, errors.

## 10. Settings window

- [ ] T16 Settings window: categories, General rows, live apply and save.
- [ ] T17 Appearance: theme grids with previews, font, cursor with preview, opacity, blur, padding.
- [ ] T18 Terminal rows: scrollback, Option as Alt, copy on select.

## 11. Background blur

- [ ] T19 macOS and Windows: `Blurred` or `Transparent` or `Opaque` from settings, applied live.
- [ ] T20 Wayland: `ext_background_effect_v1` on the window's surface.
- [ ] T21 X11: `_KDE_NET_WM_BLUR_BEHIND_REGION` on the window.

## 12. Menus and shortcuts

- [ ] T22 macOS menu bar; key bindings on all platforms.

## 13. Acceptance

- [ ] T23 End-to-end checks, README, acceptance table, CI green on three platforms.
