# Changes from upstream

This is the Asciiverse packaging of [settl](https://github.com/mozilla-ai/settl)
by Mozilla.ai: a terminal hex settlement game where you play against
language models running on your own machine (Rust, Ratatui, llamafile).
Upstream is the `upstream` remote (archived on GitHub); the `asciiverse`
branch carries the changes below.

Built from upstream `main` at `2879b73` (2026-04-08), two fixes after its
v0.1.4 tag.

## Licence

Apache-2.0: LICENSE, and `license = "Apache-2.0"` in Cargo.toml (checked
2026-11-13). The AI opponents are not shipped: on first use the game
downloads Mozilla.ai's llamafile of PrismML's Bonsai 1.7B (292 MB) or 8B
(1.2 GB) from huggingface.co/mozilla-ai/llamafile_0.10.0, whose model card
lists both under Apache 2.0 (checked 2026-11-13; both URLs answer 200).

## Code

- **A rule-based Computer opponent** (`src/player/computer.rs`). Any AI
  seat on the New Game screen can be the Computer: cycle past the last
  personality. It plays at once and needs no download or 4 GB of memory:
  it values spots by production and variety (and a harbour it can use),
  builds cities, then settlements, development cards and roads toward the
  next good spot, trades with the bank only when that completes a build,
  plays knights when the robber sits on it or the largest army is near,
  aims the robber at the leader, discards what it is not building toward,
  and judges offered trades with upstream's trade heuristic (refusing a
  player on 8 points or more). A table with no language model seats
  starts without llamafile; saves record Computer seats
  (`SavedPlayerConfig.computer`, defaulting to false for older saves). It
  wins 5 of 6 or better against two random players (tested), and on
  this machine a Bonsai 1.7B move took 14 to 28 seconds on the CPU.
- **`--demo`**: four Computer players play a game on their own, for
  watching and the catalog preview. A game with no human at the table is
  not saved, so it never replaces or deletes the player's autosave.
- **A win reached by a build is reported** (`src/game/rules.rs`).
  Applying the winning build set the phase to GameOver, which
  `check_victory` did not look at, so the game went on past the win.
  Written to go upstream.
- **Files in the state and cache folders** (`src/paths.rs`). Settings,
  saves and the debug log go to `$XDG_STATE_HOME/settl` or
  `~/.local/state/settl` (Application Support on macOS); the downloaded
  models to `$XDG_CACHE_HOME/settl` or `~/.cache/settl`
  (`~/Library/Caches/settl` on macOS). An existing `~/.settl` is kept and
  used as before.
- **No update check.** av installs and updates settl, and upstream is
  archived, so the GitHub release check is off unless
  `SETTL_UPDATE_CHECK=1`.
- **Session logging.** `RUST_LOG=debug settl` (or upstream's
  `SETTL_DEBUG=1`) writes `debug.log` in the state folder: every input
  event as it arrives, upstream's game and model logging, and a snapshot of
  the screen, turn, phase and points once a second.

## Packaging

- Upstream's workflows (CI, docs site, releases, Homebrew tap) and
  Dependabot config are removed; `.github/workflows/release.yml` builds
  static Linux (musl, x86-64 and ARM64) and macOS (x86-64 and ARM64)
  archives for av, plus .deb, .rpm, .apk and Arch packages
  (`asciiverse/nfpm.yaml`). No Windows build: upstream never built one,
  and its llamafile start is written for Unix.
- `asciiverse/settl.cast`: the catalog preview, recorded at 130x60 (the
  game's own minimum, for its board) by `asciiverse/record-preview.sh`
  from `settl --demo`; the catalog crops it to 100x30 from the centre.
- README.md: a note on playing it with av at the top.
