# AGENTS.md

Notes for AI agents and contributors working on rsAmp.

## Project

- A classic-style skinnable music player in Rust (winit + softbuffer, cpal + symphonia).
- The name is **rsAmp** (capital A) in anything user-facing: window title, messages, folder
  names, docs. The Cargo package is `rsamp`, the binary `rsAmp`.
- Mention Winamp only where needed to describe skin compatibility. Don't use its name or
  artwork for rsAmp itself.
- Never commit skins (`.wsz`) or other third-party artwork. `.gitignore` excludes them.

## Building and testing

- `cargo test` runs behavioural tests that drive the UI with simulated mouse input.
- Windows exe from Linux: `cargo build --release --target x86_64-pc-windows-gnu`
  (needs mingw-w64). `build.rs` embeds `assets/rsamp.ico` into it.
- `rsAmp --screenshot out.png [--zoom 130] [--skin x.wsz] [files]` renders one frame without
  a window, which is handy for checking rendering changes.

## Not implemented

Playlist menu items for these are commented out in `PL_MENUS` (`src/sprites.rs`). Restoring
one means uncommenting it there, cropping the menu's `bar` back to the full height, and
handling its sprite row in `App::menu_action` (`src/app.rs`).

- **ADD > Add URL**: streaming / SHOUTcast and other internet radio URLs.
- **REM > Rem Misc**: the extra remove options (duplicates, dead files).
- **MISC > File Info**: a file info / tag editor dialog.
- **MISC > Misc Opts**: playlist options (sort options, generate file list, etc.).

Also missing:

- `region.txt` (non-rectangular windows), skin cursors, `gen.bmp` / `video.bmp`.
- Undocking windows. All three are stacked in one OS window.
- EQ preset files (`.eqf`) and the presets menu.
- Context (right-click) menus.
