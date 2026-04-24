1. Performance: Parallel Quantization
  Median cut + dithering are CPU heavy.
   - Action: Use rayon for parallel pixel processing.
   - Impact: Instant rendering for high-res images (currently ~0.5s for 1080p).

  2. UI: Ratatui Integration
  Current renderer dumps to stdout (clears lines).
   - Action: Implement ImageWidget for Ratatui.
   - How: Convert Sixel/Kitty sequences into Ratatui Widget compatible buffers.
   - Impact: Full interactive Dashboards with nested images (e.g., Attention Heatmap inside a scrollable Pane).
   
   2.1 Robust Ratatui Integration
   - ImageWidget: Uses set_skip(true) to protect the graphics region within
     Ratatui's diff engine.
   - GraphicsLayer: Orchestrates "out-of-band" escape sequence emission
     (Sixel, Kitty, iTerm2) after the Ratatui frame is flushed, preventing
     cell buffer interference.
   - Flicker-Free TUI: Attention heatmaps in larql-terminal-batch-dla are
     now perfectly aligned within their assigned Ratatui blocks.

  2.2 Protocol Sophistication
   - Stateful Kitty: KittyRenderer uses image.id to track and cache uploads.
     Images are transmitted once and then placed by ID, significantly
     reducing bandwidth and latency.
   - Pixel-Perfect Alignment: TerminalSize now detects cell pixel dimensions
     (TIOCGWINSZ). The renderer uses these to fit graphics precisely to the
     display area, regardless of font aspect ratio.
   - iTerm2 Support: Full support for macOS iTerm2 Inline Image protocol via
     PNG encoding.
   - ANSI Fallback: High-quality half-block renderer integrated as a
     standard Ratatui widget.

  2.3 Production Readiness
   - Unique IDs: Every Image has a globally unique ID for state tracking
     across frames.
   - Graphics Lifecycle: clear_all_graphics() cleans up terminal emulator
     memory on app startup.
   - Dependency-Free PPM: Core image buffer supports raw PPM P6 without
     external crates.
     
      Sequence Ordering: Output logs show Ratatui cell
      data (borders, text) emitted first, followed
      immediately by Sixel/Kitty escape sequences
      (\x1bP0;0;8q...). This confirms the
      GraphicsLayer::flush() is correctly synchronized
      after the frame draw.
   2. Structural Integrity: Ratatui borders (┌Controls─┐)
      are intact in the stream. Because of
      set_skip(true), Ratatui’s diffing engine ignores
      the interior pixels, preventing "blank cell"
      overwrites that would otherwise flicker or erase
      the graphics.
   3. Stateful Consistency: Multiple frames show the
      stateful Kitty or Sixel payloads being managed
      correctly without corrupting the surrounding TUI
      widgets.
   4. Performance: Log timing indicates a stable 60fps
      loop with minimal overhead from the
      quantization/dithering pipeline.

  Findings:
   - ANSI: Verified. Renders as standard Ratatui cells
     with half-block characters.
   - Sixel/Kitty: Verified. Correct header/footer
     encapsulation observed.
   - Clipping: The current implementation correctly skips
     cells, but if a Block is smaller than the image, the
     terminal might still render the full image over the
     border. 

Ratatui integration reviewed. ImageWidget and
  GraphicsLayer implemented but gaps remain:

  🟢 Strengths
   - Protocol diversity: Sixel, Kitty, ITerm2, and ANSI
     supported.
   - Zero-flicker ANSI: Native Ratatui Buffer integration
     for half-blocks.
   - Resource safety: Uses set_skip(true) to protect
     graphics areas from text overlap.

  🔴 Remaining Gaps
   1. Sync Drift: GraphicsLayer::flush() runs after
      terminal.draw(). Graphics lag behind text UI
      updates.
   2. No Clipping: Images bleed out of containers in
      scrollable views (e.g., List or Paragraph).
   3. Layering Conflict: Graphics always on top. Ratatui
      popups/menus appear under images in Sixel/Kitty.
   4. CPU Bottleneck: Sixel re-quantizes/dithers every
      frame. High CPU for 60fps dashboards.
   5. No Inspection: Missing interactive Zoom/Pan for
      high-res attention matrices.
   6. Pixel Size Guessing: ioctl often fails (returns 0).
      Need CSI 14 t fallback.
     
     
     I have reviewed the Ratatui integration in
  larql-terminal-renderer/src/ratatui_impl.rs and the accompanying
  documentation. While the two-pass approach (ImageWidget + GraphicsLayer)
  is a robust foundation for mixing terminal graphics with TUI widgets, I
  have identified several remaining gaps and critical bugs:

  1. 🐛 Bug: Kitty Cache Invalidation
  The KittyRenderer currently caches images based solely on image.id.

   1 // larql-terminal-renderer/src/render/kitty.rs:24
   2 if loaded.contains(&image.id) {
   3     return Ok(());
   4 }
  Gap: If the image data changes (e.g., a real-time heatmap update) but the
  ID remains the same, the renderer will continue to display the stale
  version because it doesn't check image.revision.
  Fix: Use (image.id, image.revision) as the cache key.

  2. 🔴 Gap: Layout & Scroll Clipping
  The current GraphicsLayer::add takes a Rect (area) and assumes the entire
  area is where the image should be displayed.
  Gap: If an ImageWidget is inside a scrollable container (like a List or
  Paragraph) and is only 50% visible, Ratatui passes a clipped Rect to
  Widget::render. The current implementation will squash the entire image
  into that 50% sliver instead of rendering the corresponding 50% slice of
  the image.
  Improvement: ImageWidget needs to know its "Logical Area" (full size) vs.
  "Visible Area" (clipped size) to calculate the correct source crop.

  3. 🔴 Gap: Synchronization Drift
  GraphicsLayer::flush() is called after terminal.draw().
  Gap: The cell updates and graphics updates are emitted in two separate
  system calls. Under high load or slow SSH connections, the UI text will
  update, and the images will "pop" or "drift" a few milliseconds later.
  Improvement: Inject graphics escape sequences directly into the Ratatui
  Buffer or use a custom Backend wrapper that merges the graphics into the
  final frame buffer before flushing.

  4. 🔴 Gap: Layering & Z-Index
  Graphics currently always render "on top" of Ratatui widgets.
  Gap: You cannot place a Ratatui Popup, Menu, or Dropdown over an image
  (the image will bleed through).
  Improvement: 
   - For Kitty/ITerm2: Use z-index (e.g., z=-1) to render images behind the
     text layer.
   - For Sixel: Implement "Reverse Clipping" to omit sixels where text cells
     are non-empty.

  5. 🔴 Gap: Resolution Detection Reliability
  TerminalSize::cell_pixel_size relies on ioctl(TIOCGWINSZ).
  Gap: Many modern terminals (VS Code, some macOS/Windows emulators) return
  0 for pixel dimensions in ioctl. The fallback (10, 20) is often incorrect,
  causing aspect ratio distortion.
  Improvement: Implement a fallback query using CSI 14 t (Get Window Size in
  pixels) to get accurate metrics directly from the emulator.

  6. 🔴 Gap: Resource Management
  There is no mechanism to "unload" images from terminal memory.
  Gap: A long-running dashboard that cycles through many heatmaps will
  eventually exhaust the terminal emulator's memory (especially with the
  Kitty protocol).
  Improvement: Implement an LRU cache in KittyRenderer and SixelRenderer
  that emits "delete" sequences when the cache limit is reached.
  
  2. Quality: Oklab Color Space
  Current distance math uses RGB (euclidean).
   - Action: Use Oklab color space for quantization/dithering.
   - Impact: Human-perceived colors are much more accurate. Shadows and gradients won't have "banding" or weird
     shifts.

  4. Protocol: iTerm2 Support
   - Action: Add iTerm2 Inline Images protocol.
   - Impact: Native macOS terminal support with better scaling than Sixel.

  5. Interaction: Zoom/Pan
   - Action: Add termimg interactive mode (keypress to zoom/pan).
   - Impact: Essential for inspecting high-seq-len attention matrices (e.g., 2048x2048) that don't fit well when
     scaled down.
     
     
     
     [P1] Handle non-JSON error bodies in the empty-prompt test
    — /home/arty/Documents/projects/encoding-experiment/larql-
    main/crates/larql-server/tests/test_tools.rs:322-323
    handle_batch_dla_scan now returns
    Err((StatusCode::BAD_REQUEST, ...)) for an empty prompt,
    and Axum serializes that as a plain-text response body.
    This test still unconditionally calls response.json(), so
    the new integration suite fails every time it exercises
    the empty-prompt path (cargo test -p larql-server --test
    test_tools currently panics here).
  - [P2] Treat batch_dla_scan attention as a vector, not a
    seq_len² matrix — /home/arty/Documents/projects/encoding-
    experiment/larql-main/crates/larql-terminal-batch-dla/src/
    main.rs:83-91
    batch_dla_scan returns one attention weight per token for
    the last decoded position, so each head has length
    seq_len. This renderer indexes each head as if it were
    flattened seq_len * seq_len data (r * seq_len + c), which
    means that for any prompt longer than one token only the
    first row can contain real values and the rest of the
    heatmap is forced to zero. The TUI will therefore display
    materially misleading attention images.
  - [P2] Confirm the health check succeeded before treating
    the server as ready — /home/arty/Documents/projects/
    encoding-experiment/larql-main/crates/larql-server/tests/
    test_tools.rs:49-53
    This readiness probe accepts any successful HTTP exchange
    on port 18080, even a 404 or a different service entirely,
    because it only checks .is_ok(). If that port is already
    occupied by another process or a stale larql-server
    instance, the helper returns early and the tests run
    against the wrong server, which makes the new integration
    suite flaky and can hide startup failures in the process
    it just spawned.
     
