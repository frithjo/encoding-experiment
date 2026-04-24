# Carbonyl Lift Follow-up Tickets

This file tracks follow-up work from the Carbonyl lift implementation (Option A + B).

## Platform Artifacts

### aarch64 Support
- **Status**: Pending
- **Description**: Add Linux aarch64 Carbonyl binary artifact to CARBONYL_VERSION pinning
- **Work**:
  - Determine Carbonyl aarch64 release URL format
  - Add aarch64 SHA256 to CARBONYL_VERSION (dual-line format)
  - Extend install script to detect architecture and download correct artifact
  - Update CI to test aarch64 build
- **Priority**: Medium
- **Blocker**: Requires Carbonyl aarch64 release availability

### macOS Support
- **Status**: Pending
- **Description**: Add macOS Carbonyl binary artifact to CARBONYL_VERSION pinning
- **Work**:
  - Determine Carbonyl macOS release URL format (arm64, x86_64, universal)
  - Add macOS SHA256 to CARBONYL_VERSION (multi-line format)
  - Extend install script to detect OS and download correct artifact
  - Update CI to test macOS build
- **Priority**: Medium
- **Blocker**: Requires Carbonyl macOS release availability

## larql-tty-io Enhancements

### Damage Tracker
- **Status**: Deferred
- **Description**: Implement damage tracking / dirty-rect scheduler for efficient frame updates
- **Work**:
  - Add `DamageTracker` struct to larql-tty-io
  - Track dirty regions per frame
  - Merge overlapping dirty rectangles
  - Provide `get_dirty_regions()` API for renderer
  - Integration test with larql-terminal-renderer
- **Priority**: Low
- **Blocker**: Requires local frame producer (dead code until non-Carbonyl path)
- **Note**: Deferred per plan - only useful when we have a frame source

### DPR/Zoom Geometry Helpers
- **Status**: Deferred
- **Description**: Lift Carbonyl's DPR/zoom geometry mapping logic
- **Work**:
  - Add `Geometry` struct to larql-tty-io
  - Map cell-grid geometry to CSS pixels with DPR scaling
  - Provide `cell_to_pixel()` and `pixel_to_cell()` helpers
  - Add zoom level support
- **Priority**: Low
- **Blocker**: Carbonyl still owns geometry mapping
- **Note**: Deferred per plan - Carbonyl handles this internally

### SIGWINCH on macOS
- **Status**: Pending
- **Description**: Implement SIGWINCH handling for macOS in larql-tty-io
- **Work**:
  - Use signal-hook or similar crate for macOS signal handling
  - Implement `ioctl(TIOCGWINSZ)` for macOS
  - Update `spawn_resize_handler()` to work on macOS
  - Test on macOS hardware
- **Priority**: Low
- **Blocker**: macOS hardware for testing

### Bracketed Paste Decoding
- **Status**: Placeholder
- **Description**: Complete bracketed paste decoding in larql-tty-io
- **Work**:
  - Implement full OSC 200;200p / 201;201p parsing
  - Handle paste content between start/end markers
  - Add `PasteContent` event variant
  - Add tests
- **Priority**: Low
- **Blocker**: None
- **Note**: Currently a placeholder in v1

## Engine Evaluation

### Servo Evaluation
- **Status**: Pending
- **Description**: Evaluate Servo as a non-Carbonyl rendering engine path
- **Work**:
  - Research Servo offscreen rendering API status
  - Build proof-of-concept Servo integration with larql-tty-io
  - Compare feature completeness vs Carbonyl (HTML/CSS/JS/MSE)
  - Assess web-compat gaps
  - Evaluate build system integration (Cargo vs Chromium build)
- **Priority**: Medium
- **Blocker**: Servo offscreen API stability verification
- **Note**: This is the primary alternative to Carbonyl for in-tree engine

## Documentation

### Update CARBONYL_VERSION with Real SHA256
- **Status**: Pending
- **Description**: Replace placeholder SHA256 in CARBONYL_VERSION with actual value
- **Work**:
  - Download Carbonyl 0.0.3 Linux x86_64 release
  - Compute SHA256: `sha256sum carbonyl-linux-x86_64.tar.gz`
  - Update apps/terminal-runtime/CARBONYL_VERSION
  - Verify install script succeeds with real download
- **Priority**: High
- **Blocker**: None
- **Note**: Current placeholder will fail download verification

## Summary

**High Priority**:
- CARBONYL_VERSION real SHA256

**Medium Priority**:
- aarch64 artifact support
- macOS artifact support
- Servo evaluation

**Low Priority**:
- Damage tracker (deferred)
- DPR/zoom helpers (deferred)
- SIGWINCH macOS
- Bracketed paste decoding
