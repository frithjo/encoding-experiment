# Terminal Renderer Live Artifacts

Date: 2026-04-23

This directory contains live artifacts generated from the current `larql-terminal-renderer` tree.

Files:
- `interactive-video-ansi-tmux.png`
  - Reconstructed visual proof from a real `tmux` capture of:
  - `cargo run -q -p larql-terminal-renderer --bin termimg -- --interactive --video /tmp/larql-live-proof.mp4 --backend ansi --fps 5`
  - Validates:
  - terminal-only interactive video playback
  - Ratatui frame rendering with image content present in the live PTY stream
  - truecolor ANSI output in the interactive path
- `interactive-video-ansi-tmux.txt`
  - Raw escaped pane capture used to build the PNG proof
- `kitty-png-passthrough.txt`
  - Raw output from:
  - `cargo run -q -p larql-terminal-renderer --bin termimg -- /home/arty/Documents/projects/encoding-experiment/images/logo-lamtab.png --backend kitty`
  - Validates Kitty PNG passthrough by inspection of the live command stream
- `artifact-metrics.txt`
  - Extracted metrics from the live captures
- `video-source-path.txt`
  - Source path of the generated MP4 used for the live video proof
- `sixel-reverse-clipping-tmux.txt`
  - Fallback text-pane capture from a `dashboard_stress_test` run under `tmux` with `Backend: Sixel`
- `sixel-reverse-clipping-pty.bin`
  - Raw PTY capture attempt for the same Sixel path
- `dashboard-fixed-z1.bin`
  - Raw PTY capture from `dashboard_stress_test --z-index 1 --quit-after-ms 1200`
- `dashboard-fixed-zminus1.bin`
  - Raw PTY capture from `dashboard_stress_test --z-index -1 --quit-after-ms 1200`
  - This is the decisive reverse-clipping proof pair because both runs use the same live app path and fixed PTY geometry
- `dashboard-fixed-z1-after-fix.bin`
  - Raw PTY capture after tightening reverse-clipping occlusion
- `dashboard-fixed-zminus1-after-fix.bin`
  - Raw PTY capture after tightening reverse-clipping occlusion

Current validated facts:
- `interactive-video-ansi-tmux.txt` contains live truecolor SGR output for the interactive video path.
- `kitty-png-passthrough.txt` contains `f=100` and a base64 PNG signature, and does not contain raw RGB `f=24`.
- `dashboard-fixed-z1.bin` contains live Sixel payloads.
- `dashboard-fixed-zminus1.bin` contains no live Sixel payloads.

Previous live conclusion for Sixel reverse clipping:
- Before the fix, foreground Sixel (`z=1`) emitted graphics, but background Sixel (`z=-1`) did not emit graphics in the fixed dashboard proof run.

Current conclusion for Sixel reverse clipping:
- After the fix, both foreground and background fixed-dashboard runs emit live Sixel payloads.
- Before fix:
  - `dashboard-fixed-z1.bin`: 14 Sixel headers
  - `dashboard-fixed-zminus1.bin`: 0 Sixel headers
- After fix:
  - `dashboard-fixed-z1-after-fix.bin`: 12 Sixel headers
  - `dashboard-fixed-zminus1-after-fix.bin`: 11 Sixel headers
- This shows the background reverse-clipping path is now live-active instead of being fully occluded by style-only empty cells.
