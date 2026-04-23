# Review: Ratatui Integration in `larql-terminal-renderer`

The current integration is functional but suffers from synchronization, performance, and layering issues.

## 🟢 Strengths
- **Backend-Agnostic**: Works with Sixel, Kitty, ITerm2, and ANSI.
- **Buffer Integration**: ANSI renderer uses Ratatui's `Buffer` directly for zero-flicker text/graphics mix.
- **Resource Reservation**: Correctly uses `set_skip(true)` to prevent Ratatui from overwriting graphics areas.

## 🔴 Remaining Gaps & Roadmap

### 1. Synchronization (Flicker & Drift)
- **Current**: `GraphicsLayer::flush()` runs *after* `terminal.draw()`.
- **Issue**: UI text and graphics are not updated atomically. On slow connections or heavy load, images will lag behind the text UI.
- **Improvement**: Support "In-Band" rendering where graphics sequences are injected into the Ratatui buffer stream, or use a custom `Backend` that intercepts the flush.

### 2. Clipping & Scrolling
- **Current**: Images always render at the full size of the provided `Rect`.
- **Issue**: If an image is inside a scrollable `List` or `Paragraph`, it will bleed out of its container because there is no sub-pixel clipping logic.
- **Improvement**: Implement `clip` support in `Renderer::render_at`.

### 3. Layering (The "Overlay" Problem)
- **Current**: Graphics always render *on top* of Ratatui widgets.
- **Issue**: You cannot display a Ratatui `Popup`, `Menu`, or `Dropdown` over an image.
- **Improvement**: 
  - For **Kitty/ITerm2**: Use the protocol's native `z-index` support.
  - For **Sixel**: Implement "Reverse Clipping" where the image is sliced into fragments around overlapping text widgets.

### 4. Quantization Bottleneck
- **Current**: Sixel backends re-quantize and re-dither every single frame.
- **Issue**: Massive CPU usage for 60fps dashboards.
- **Improvement**: Implement a `SixelCache`. Store the final escape sequence string keyed by `Image::id` and `Area::size`. Only re-compute if the image data or target size changes.

### 5. Interactive Inspect / Zoom
- **Current**: `ImageWidget` is static.
- **Gap**: No built-in way to handle "Zoom in on this head" for high-res attention matrices.
- **Improvement**: Add a `Zoom` capability to the `ImageWidget` state that allows panning across a large `Image` within a small `Rect`.

### 6. Terminal Query Support
- **Current**: Pixel size detection relies on `ioctl`.
- **Issue**: Many modern terminal emulators (especially on macOS/Windows) return `0` for pixel dimensions via `ioctl`.
- **Improvement**: Use `CSI 14 t` (Get Window Size in pixels) as a fallback escape sequence query.
