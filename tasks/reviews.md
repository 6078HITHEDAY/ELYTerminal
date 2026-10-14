# Reviews

Each task gets at most three codex rounds. Each round: the commit, the verdict, the findings and what was done.

## T01

- Round 1, 4e2f3c9: FAIL. Linux had no X11 or Wayland backend and would start headless; macOS CI linked no binary; `Cargo.lock` passed the 500-line rule unstated. Fixed in 9c0c067 and f8857fe: `wayland` and `x11` on, Linux starts with `WAYLAND | X11` and no headless; `cargo build --locked` in macOS CI; the lockfile named as exempt.
- Round 2, f8857fe: PASS. The Windows runner's missing `rustc` goes to T23.
