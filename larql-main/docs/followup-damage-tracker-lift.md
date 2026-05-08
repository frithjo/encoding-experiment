# Follow-up: Damage tracker / dirty-rect scheduler lift

## Context

The Carbonyl lift plan (Option A) explicitly deferred the damage tracker / dirty-rect scheduler as "dead code until we have a local frame producer." This ticket tracks lifting the damage tracking logic from Carbonyl's Rust internals into `larql-tty-io` or a new crate.

## Why this was deferred

- Carbonyl owns the frame pipeline; without a local frame producer (Servo, custom renderer, Leptos-in-TTY), damage tracking has nothing to track
- The complexity of Carbonyl's damage tracker is non-trivial and tightly coupled to Chromium's rendering model
- No clear consumer in the current architecture (Carbonyl runs as a child process with its own TTY)

## Work required

1. **Research Carbonyl's damage tracker**
   - Locate the damage tracking code in Carbonyl's source
   - Understand the dirty-rect scheduling algorithm
   - Identify the interface between damage tracker and frame emitter

2. **Design engine-independent damage tracker API**
   - Define a generic `DamageTracker` trait or struct
   - Input: frame updates (full or partial), viewport size, scroll position
   - Output: dirty rectangles to re-render
   - Consider Sixel vs Kitty vs iTerm2 protocol constraints

3. **Implement in `larql-tty-io` or new crate**
   - If the logic is tightly coupled to rendering protocols, consider a new `larql-damage` crate
   - If it's purely geometric, add to `larql-tty-io` as a new module
   - Ensure it's testable without a real rendering backend

4. **Integrate with future frame producers**
   - When Servo or a custom renderer is added, wire the damage tracker into the frame pipeline
   - Ensure the damage tracker can emit protocol-specific rendering commands

5. **Tests**
   - Unit tests for damage tracking algorithm
   - Integration tests with mock frame producers
   - Benchmark performance for typical workloads

## Dependencies

- A local frame producer (Servo, custom renderer, or Leptos-in-TTY)
- Clear requirements for which rendering backends need damage tracking
- Understanding of Carbonyl's damage tracker internals (may require reverse engineering)

## Acceptance

- Damage tracker crate/module exists with a clean API
- Unit tests cover the core algorithm
- Integration tests demonstrate correct dirty-rect emission for mock frames
- Documentation explains how to wire it into a frame producer
