# Reviews

Each task gets at most three codex rounds. Each round: the commit, the verdict, the findings and what was done.

## T01

- Round 1, 4e2f3c9: FAIL. Linux had no X11 or Wayland backend and would start headless; macOS CI linked no binary; `Cargo.lock` passed the 500-line rule unstated. Fixed in 9c0c067 and f8857fe: `wayland` and `x11` on, Linux starts with `WAYLAND | X11` and no headless; `cargo build --locked` in macOS CI; the lockfile named as exempt.
- Round 2, f8857fe: PASS. The Windows runner's missing `rustc` goes to T23.

## T02

- Round 1, e1770ba: FAIL. One shared temp name let overlapping saves corrupt the file; parse errors lost the field name. Fixed in 83ec4b1: a temp per save (pid, counter, create_new), cleaned on failure; serde_path_to_error names the field; a four-writer test, red on the old save.
- Round 2, 83ec4b1: PASS.

## T03

- Round 1, 7d4d535: PASS. Theme files match upstream 21f281d byte for byte.

## T04

- Round 1, 62e50fe: FAIL. A folder without enter permission started the shell elsewhere; no pid for the app; folder and exit unlogged; grid and pty sizes split past the kernel's cap. Fixed in 96ae85b.
- Round 2, 96ae85b: FAIL. The permission check raced alacritty's chdir, whose result alacritty ignores; closing a live pty logged nothing. Fixed in fff436c: ELYTerminal's own Unix pty, a checked chdir in pre_exec that fails the spawn, closing and hung-up logs with a test.
- Round 3, fff436c: FAIL. Drop waited on the program while holding the master, so a program ignoring SIGHUP never ended. Fixed in 0e7f309: the master closes before the wait; a test that traps HUP went red, then green.
- The review cap of three rounds is reached. The round-3 fix stands on its red-green test, not a fourth round.
