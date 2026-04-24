# Follow-up: DPR/zoom geometry helpers lift

## Context

The Carbonyl lift plan (Option A) explicitly deferred DPR (device pixel ratio) and zoom geometry helpers. Carbonyl currently owns the mapping between CSS pixels and terminal cells. This ticket tracks lifting that logic into `larql-tty-io` or a new crate.

## Why this was deferred

- Carbonyl's geometry mapping is tightly coupled to Chromium's rendering model
- Without a local frame producer, the geometry helpers have no clear consumer
- The mapping logic is complex (DPR, zoom level, viewport size, scroll position)
- Carbonyl still owns geometry mapping in the current architecture

## Work required

1. **Research Carbonyl's geometry mapping**
   - Locate the DPR/zoom calculation code in Carbonyl's source
   - Understand how CSS pixels map to terminal cells
   - Identify the zoom level propagation path from Chromium to TTY

2. **Design engine-independent geometry API**
   - Define a `Geometry` struct with: DPR, zoom level, viewport size (cells), viewport size (pixels)
   - Helper functions: `css_pixels_to_cells()`, `cells_to_css_pixels()`, `apply_zoom()`
   - Consider terminal protocol constraints (Sixel vs Kitty vs iTerm2)

3. **Implement in `larql-tty-io` or new crate**
   - If the logic is general-purpose, add to `larql-tty-io` as a new module
   - If it's rendering-specific, consider a new `larql-geometry` crate
   - Ensure it's testable without a real rendering backend

4. **Integrate with future frame producers**
   - When Servo or a custom renderer is added, wire the geometry helpers into the frame pipeline
   - Ensure the geometry helpers can emit protocol-specific rendering commands

5. **Tests**
   - Unit tests for geometry conversion functions
   - Integration tests with mock frame producers
   - Benchmark performance for typical workloads

## Dependencies

- A local frame producer (Servo, custom renderer, or Leptos-in-TTY)
- Clear requirements for which rendering backends need geometry helpers
- Understanding of Carbonyl's geometry mapping internals (may require reverse engineering)

## Acceptance

- Geometry helper crate/module exists with a clean API
- Unit tests cover the core conversion functions
- Integration tests demonstrate correct geometry mapping for mock frames
- Documentation explains how to wire it into a frame producer
