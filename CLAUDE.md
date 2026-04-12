# CLAUDE.md

## Project

cmux is a Rust TUI for managing Claude Code tmux sessions. Binary name is `cmux`. The source directory is `~/ctui`.

## Build

No system Rust -- use `nix develop` for the toolchain:

```sh
nix develop
cargo build --release    # binary at target/release/cmux
cargo test               # unit tests
cargo clippy --release   # lint
```

Always run clippy before considering a change done. The git tree warning ("Git tree is dirty") from nix is expected and harmless.

## Architecture

```
src/
  main.rs          -- entry point, arg parsing, terminal setup, picker mode
  tmux.rs          -- all tmux shell-outs (list-panes, capture-pane, switch, sessions)
  claude_state.rs  -- state detection: hook files first, TUI scraping fallback
  git.rs           -- git metadata: branch, dirty, ahead/behind, worktree detection
  history.rs       -- scan ~/.claude/projects for resumable sessions
  snapshot.rs      -- parallel enrichment pipeline (rayon): tmux + git + state
  util.rs          -- shared helpers (shorten_home, fmt_duration, fmt_age)
  ui/
    mod.rs         -- App struct, event loop (500ms poll), tab/key dispatch
    live.rs        -- Live tab: panes grouped by repo with git indicators
    sessions.rs    -- Sessions tab: tmux session list
    resume.rs      -- Resume tab: historical claude sessions
    filter.rs      -- nucleo-matcher fuzzy filter wrapper
    preview.rs     -- live pane preview with ANSI color support
    picker.rs      -- standalone dir picker for `cmux new <dir>`
```

## Key design decisions

- **Snapshot-based**: pane list is captured once at startup, not live-refreshed. Preview IS live (500ms tick).
- **Parallel enrichment**: rayon parallelizes per-pane shell-outs (capture-pane + git probe). This is the critical path for startup latency.
- **Hook-based state > scraping**: `$XDG_RUNTIME_DIR/claude-agents/<pane_id>.state` is checked first. Falls back to string-matching the last 20 lines of pane output. Scraping is brittle but the miss cost is low (shows "unknown" glyph).
- **Bare repo support**: git grouping uses `--git-common-dir` which handles worktrees. `repo_name` derivation checks if common_dir ends in `.git` (normal) vs IS the repo dir (bare).
- **Branch detection separated from grouping**: `--abbrev-ref HEAD` is a separate git call because it fails on repos with no commits (exit 128), which would otherwise kill the grouping probe.

## Edge cases to watch for

- Bare repos (e.g. `~/work/infrastructure.git`) with worktrees inside them
- Repos with no commits (HEAD unresolvable) -- branch shows "?"
- Panes running claude inside a shell wrapper (won't match `pane_current_command == "claude"`)
- tmux popups: cmux runs inside one, so `current_pane_id()` returns the popup's own pane -- it won't match any listed pane (expected)
- ANSI capture can fail for split panes -- falls back to plain capture

## State glyphs

- `●` green = Running (agent actively working)
- `◐` yellow = WaitingPermission (blocked on tool approval)
- `○` blue = Idle (waiting for user input)
- `·` gray = Unknown (couldn't determine)

## Git indicators (shown after branch name)

- `*` = dirty (uncommitted changes)
- `⚘` = linked worktree (not the main checkout)
- `↑N` = commits ahead of upstream
- `↓N` = commits behind upstream
