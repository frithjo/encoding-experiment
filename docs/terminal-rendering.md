Most of it can be built with very few dependencies.

The key is to separate the system into **image preparation** and **terminal emission**.

## Core point

Terminal image rendering does **not** require a heavy graphics stack.

The terminal side is mostly:

```text
pixels → encode → escape sequences → stdout
```

You do **not** need a GUI library, OpenGL code, SDL, curses, ratatui, or a full TUI framework for a first serious version.

The heavy parts are:

| Component                      | Needs dependencies? | Notes                                            |
| ------------------------------ | ------------------: | ------------------------------------------------ |
| ANSI truecolor block rendering |                  No | Just write escape sequences.                     |
| Kitty graphics protocol        |           Mostly no | Escape sequences + base64/chunking.              |
| Sixel encoding                 |     No, but tedious | Pure CPU bitmap encoder.                         |
| Terminal size detection        |             Minimal | Can use `ioctl`, or shell out to `stty size`.    |
| PNG/JPEG/WebP decoding         |         Usually yes | This is where dependencies enter.                |
| Resizing/filtering             |                  No | Nearest/bilinear are easy to write.              |
| Dithering                      |                  No | Floyd–Steinberg or ordered dithering are simple. |
| Video decoding                 |                 Yes | Use `ffmpeg`; do not reinvent that.              |

So the clean design is:

```text
Renderer core:
  - terminal capability detection
  - layout/sizing
  - ANSI block renderer
  - Kitty protocol renderer
  - Sixel renderer
  - fallback selection

Image input layer:
  - raw RGB/RGBA
  - PPM/PAM
  - optional PNG/JPEG decoder
  - optional ffmpeg bridge
```

That keeps the real renderer small.

## Minimal architecture

I’d build it like this:

```text
termimg/
  src/
    main.rs
    image_buffer.rs
    resize.rs
    render/
      ansi_blocks.rs
      kitty.rs
      sixel.rs
      protocol.rs
    terminal.rs
```

With these internal types:

```rust
struct Image {
    width: usize,
    height: usize,
    pixels: Vec<Rgb>,
}

struct Rgb {
    r: u8,
    g: u8,
    b: u8,
}

enum RenderBackend {
    Kitty,
    Sixel,
    AnsiHalfBlock,
    AnsiBraille,
}
```

The renderer should not care whether the image came from PNG, JPEG, PPM, a screenshot, or a video frame. It should only receive an `Image`.

That is the main dependency-control move.

## Best first renderer: ANSI half-blocks

This is the easiest serious mode.

Each terminal character cell represents two vertical pixels:

```text
top pixel    → foreground color
bottom pixel → background color
character    → ▀
```

So one cell gives you:

```text
foreground RGB + background RGB + "▀"
```

Example output pattern:

```text
\x1b[38;2;R;G;Bm\x1b[48;2;R;G;Bm▀
```

This gives a decent high-density terminal image using only standard output.

No dependencies required.

The core loop is basically:

```rust
for y in (0..height).step_by(2) {
    for x in 0..width {
        let top = image.get(x, y);
        let bottom = image.get(x, y + 1);

        print!(
            "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m▀",
            top.r, top.g, top.b,
            bottom.r, bottom.g, bottom.b,
        );
    }

    print!("\x1b[0m\n");
}
```

That alone gives you a working terminal image renderer.

## Second renderer: Kitty protocol

Kitty is more “real image” than character approximation.

A minimal implementation needs:

```text
read image bytes
base64 encode
chunk into protocol-safe segments
write Kitty escape sequences
```

You can avoid decoding if you send a supported encoded image format directly, especially PNG. That means your first Kitty renderer can be nearly dependency-free:

```text
PNG file bytes → base64 → Kitty escape sequence → terminal displays image
```

The tradeoff: if you want resizing, cropping, fitting to terminal cells, or format conversion, then you need to decode the image yourself or call another tool.

So the minimal implementation path is:

```text
v1:
  PNG only
  no decoding
  no resizing
  send file bytes through Kitty protocol

v2:
  add terminal sizing
  add placement/scaling options

v3:
  add decoded RGB image path
  add resize/crop

v4:
  add JPEG/WebP through optional decoder or external conversion
```

## Third renderer: Sixel

Sixel is also buildable without dependencies, but it is more annoying.

Conceptually:

```text
RGB image
→ quantize colors
→ group pixels into vertical bands of 6
→ encode each 6-pixel column as a sixel character
→ emit color definitions + sixel data
```

The annoying parts are:

1. color palette generation
2. dithering
3. run-length encoding
4. terminal quirks

A practical no-heavy-dependency Sixel renderer can start with a fixed palette:

```text
216-color RGB cube:
6 levels × 6 levels × 6 levels
```

Then map each pixel to nearest palette color.

That avoids implementing fancy quantization at first.

## Where dependencies are genuinely useful

The one dependency I would not fight too hard is image decoding.

For Rust, the pragmatic choice is usually the `image` crate. But if the design goal is “almost no dependencies,” use this split:

### Dependency-light mode

Accept only:

```text
PPM / P6
raw RGB
raw RGBA
```

Then convert outside the program:

```bash
magick input.jpg output.ppm
```

or:

```bash
ffmpeg -i input.jpg -pix_fmt rgb24 output.rgb
```

Your renderer remains clean and tiny.

### Practical mode

Use:

```toml
image = { version = "...", default-features = false, features = ["png", "jpeg"] }
```

That gives you PNG/JPEG decoding without dragging in every possible format.

## Recommended build path

For Fedora + Kitty, I’d build in this order:

### Stage 1: raw/PPM ANSI renderer

Goal:

```bash
termimg image.ppm
```

Backend:

```text
ANSI truecolor half-blocks
```

This proves the image buffer, scaling, and terminal output path.

### Stage 2: terminal fitting

Add:

```bash
termimg --fit image.ppm
termimg --width 120 image.ppm
```

Implement nearest-neighbor first. Bilinear later.

### Stage 3: Kitty PNG passthrough

Goal:

```bash
termimg --kitty image.png
```

No image decoding needed. Just transmit PNG bytes through the Kitty protocol.

### Stage 4: backend ladder

Add:

```text
try Kitty
else try Sixel
else ANSI half-blocks
```

### Stage 5: optional decoders

Only then add PNG/JPEG decoding internally.

## The important design principle

Do **not** start by building an “image viewer.”

Start by building a **terminal pixel transport layer**.

The correct abstraction is:

```text
ImageSource -> PixelBuffer -> TerminalBackend
```

Not:

```text
Image file -> terminal
```

Because once you have a `PixelBuffer`, the same engine can render:

* images
* screenshots
* plots
* LLM-generated visual artifacts
* video frames
* UI previews
* heatmaps
* graph visualizations

That becomes much more interesting than a terminal image viewer.

## Minimal dependency recommendation

For a serious but lean Rust implementation:

```text
No dependencies:
  - ANSI half-block renderer
  - resize
  - PPM parser
  - Kitty protocol emitter
  - basic base64 encoder
  - basic capability detection

One optional dependency:
  - image crate for PNG/JPEG

External optional tools:
  - ffmpeg for video frames
  - ImageMagick for conversion
```

The sharp version is:

```text
Core renderer: dependency-free
Image decoding: optional boundary module
Video: external tool boundary
```

That gives you control without turning the project into a codec implementation exercise.
