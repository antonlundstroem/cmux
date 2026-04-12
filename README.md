# cmux

A fast TUI for managing Claude Code sessions and tmux workflows. Runs in a tmux popup, lists all running Claude agents grouped by git repo, and lets you switch between them instantly.

## Features

- **Live tab** -- running Claude panes grouped by git repo, with branch, dirty/worktree/ahead-behind indicators, and state detection (running/waiting/idle)
- **Sessions tab** -- list and switch between all tmux sessions
- **Resume tab** -- browse and resume historical Claude Code sessions from `~/.claude/projects`
- **Live preview** -- real-time colored preview of the selected pane (refreshes every 500ms)
- **Fuzzy filter** -- type to filter across all tabs (nucleo-matcher, same as Helix/fzf)
- **`cmux new <dir>`** -- directory picker for creating new tmux sessions
- **Hook-based state detection** -- reads `$XDG_RUNTIME_DIR/claude-agents/<pane_id>.state` with TUI scraping fallback
- **Git worktree support** -- bare repos and linked worktrees grouped correctly

## Usage

```
cmux                  # full TUI, starts on Live tab
cmux sessions         # full TUI, starts on Sessions tab
cmux new ~/work       # directory picker -- create/switch tmux session
cmux --snapshot-json  # debug: dump snapshot as JSON and exit
```

### Keybindings

| Key | Action |
|-----|--------|
| Enter | Switch to selected pane/session, or resume session |
| Tab | Cycle tabs: Live -> Sessions -> Resume |
| Type | Fuzzy filter |
| Backspace | Delete filter char |
| Esc | Clear filter, or quit if empty |
| Up/Down | Move selection |
| Ctrl-p/n | Move selection (vim-style) |
| q | Quit (when filter is empty) |
| Ctrl-c | Quit |

### tmux bindings

```tmux
bind-key -n M-s   display-popup -E -w 80% -h 70% cmux
bind-key -n C-p   display-popup -E -w 50% -h 30% cmux sessions
bind-key -n C-o   display-popup -E -w 50% -h 30% cmux new ~/work
```

Or with Nix:

```nix
bind-key -n M-s display-popup -E -w 80% -h 70% ${pkgs.cmux}/bin/cmux
```

## Building

Requires Rust. If using Nix:

```sh
cd ~/ctui
nix develop
cargo build --release
# binary at target/release/cmux
```

## Nix integration

The flake exports `packages.default`, `apps.default`, and `devShells.default`.

Add as an input to your flake:

```nix
inputs.cmux.url = "github:user/cmux";
```

Use as a package:

```nix
overlays = [(final: prev: { cmux = inputs.cmux.packages.${system}.default; })];
```

Then reference `pkgs.cmux` anywhere in your config.
