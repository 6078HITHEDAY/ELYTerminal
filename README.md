# ELYTerminal

A terminal for macOS, Windows and Linux, in Rust on GPUI and Ely components. Sessions sit on the left, the terminal in the middle, Git or Files on the right. Every setting has a control in the settings window.

## Build and run

Rust 1.95.

```sh
cargo run --release
```

On Linux, install the libraries gpui links first. On Debian and Ubuntu:

```sh
sudo apt-get install pkg-config libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev \
  libxcb1-dev libx11-xcb-dev libfontconfig-dev libfreetype-dev
```

The app writes its log to stderr. `RUST_LOG=elyterminal=debug` writes more; `trace` logs each paint.

## What it does

- The terminal runs on `alacritty_terminal`: 256 colors and truecolor, bold, dim, italic, five underline styles, wide characters and combining marks. Box drawing, block elements and powerline shapes are drawn as geometry, so lines join across cells. Keys follow xterm and the kitty keyboard protocol; input methods compose at the cursor; programs can track the mouse; pastes are bracketed when a program asks; OSC 8 and plain links open with the link press below; find searches the scrollback.
- Shell integration for zsh, bash, fish and PowerShell: each prompt reports its folder and title.
- Sessions: new, close and switch. A bell marks a hidden session's row and makes no sound.
- A button at each end of the title bar hides or shows its side column; the window opens as it was left.
- Files shows the session's folder as a tree with Git's marks.
- Git shows the branch, stages, unstages, discards, commits, lists recent commits and opens diffs, all through the `git` command line.
- Themes: Ely Dark, Ely Light, Catppuccin Mocha, Macchiato, Frappé and Latte, Tokyo Night, Dracula, Nord, Gruvbox Dark, Rosé Pine, Solarized Light, One Light and Kanagawa Wave. A second theme can take over while the system is dark.
- Background opacity and blur: NSVisualEffectView on macOS, acrylic on Windows, `ext_background_effect_v1` or `org_kde_kwin_blur` on Wayland, `_KDE_NET_WM_BLUR_BEHIND_REGION` on X11, which KWin reads.

## Keys

| | macOS | Windows and Linux |
|---|---|---|
| New session | Cmd-T | Ctrl-Shift-T |
| Close session | Cmd-W | Ctrl-Shift-W |
| Session 1 to 8, last | Cmd-1 to Cmd-8, Cmd-9 | Alt-1 to Alt-8, Alt-9 |
| Next, previous session | Ctrl-Tab, Ctrl-Shift-Tab | Ctrl-Tab, Ctrl-Shift-Tab |
| Sessions column | Cmd-B | Ctrl-Shift-B |
| Right column | Cmd-Alt-B | Ctrl-Shift-Alt-B |
| Copy, paste | Cmd-C, Cmd-V | Ctrl-Shift-C, Ctrl-Shift-V |
| Select all | Cmd-A | Ctrl-Shift-A |
| Find | Cmd-F | Ctrl-Shift-F |
| Bigger, smaller, reset size | Cmd-=, Cmd-minus, Cmd-0 | Ctrl-=, Ctrl-minus, Ctrl-0 |
| Open a link | Cmd-press | Ctrl-press |
| Settings | Cmd-, | Ctrl-, |
| Minimize | Cmd-M | |
| Quit | Cmd-Q | Ctrl-Shift-Q |

## Settings

The settings window saves each change at once to `settings.json` in the platform's config folder:

- macOS: `~/Library/Application Support/ELYTerminal/settings.json`
- Linux: `$XDG_CONFIG_HOME/ELYTerminal/settings.json`, else `~/.config/ELYTerminal/settings.json`
- Windows: `%APPDATA%\ELYTerminal\settings.json`

A file that does not parse, or holds a value out of range, stops the start with an error naming the file and the field. The app never overwrites it.

## Development

`scripts/check.sh` runs fmt, clippy, the tests and the 500-line rule. `AGENTS.md` lists the rest: the layout, the decisions and the tools.

## License

MIT, in `LICENSE`. Box drawing and key encoding follow Ghostty's code, MIT, in `LICENSE-GHOSTTY`. The theme files come from iTerm2-Color-Schemes, MIT, with its license in `assets/themes`. JetBrains Mono and its Nerd Font build are under the SIL Open Font License, in `assets/fonts`.
