# Hybrid Terminal Browser Architecture

## Overview

Hybrid approach combining Ratatui for layout/navigation/keyboard handling with Carbonyl for rich content rendering.

## Architecture

### Layer 1: Ratatui TUI (Layout/Navigation/Keyboard)
- **Purpose**: Handle overall layout, navigation menus, status bar, keyboard shortcuts
- **Components**:
  - Navigation bar (task switcher: Workspace, Studio, Explorer, LQL, Trace, Recipes, Runs)
  - Status bar (workspace info, capabilities, performance metrics)
  - Help panel (keyboard shortcuts, context hints)
  - Input area (command line, search)
- **Responsibilities**:
  - Capture and interpret keyboard input
  - Manage focus between UI components
  - Handle navigation between workbench pages
  - Display status information
  - Render text-based UI chrome

### Layer 2: Bridge Layer (Ratatui ↔ Carbonyl)
- **Purpose**: Coordinate between Ratatui and Carbonyl rendering
- **Components**:
  - Render coordinator (alternates between Ratatui and Carbonyl)
  - Input router (routes keyboard events to appropriate layer)
  - State manager (syncs UI state between layers)
- **Responsibilities**:
  - Switch between Ratatui mode (navigation) and Carbonyl mode (content)
  - Route keyboard input to active layer
  - Synchronize state (current page, form data, etc.)
  - Handle transitions between modes

### Layer 3: Carbonyl (Rich Content Rendering)
- **Purpose**: Render rich content (HTML/CSS/JS, images, video, code blocks)
- **Components**:
  - Content area (main viewport for workbench pages)
  - Form rendering (inputs, buttons, textareas)
  - Result display (tables, code blocks, visualizations)
- **Responsibilities**:
  - Render HTML/CSS/JS from workbench
  - Display images, video, streaming media
  - Handle form submissions
  - Display query results and traces

## Rendering Modes

### Mode 1: Navigation Mode (Ratatui)
- **When**: User is navigating between pages, using menus, viewing help
- **Appearance**: Full Ratatui TUI with navigation, status, help panels
- **Carbonyl**: Paused/suspended, not rendering

### Mode 2: Content Mode (Carbonyl)
- **When**: User is viewing a workbench page, filling forms, viewing results
- **Appearance**: Full Carbonyl rendering of workbench HTML
- **Ratatui**: Minimal overlay (optional status indicators)

### Mode 3: Hybrid Mode (Split)
- **When**: User needs both navigation and content simultaneously
- **Appearance**: Ratatui chrome (top/bottom bars) + Carbonyl content area
- **Implementation**: Terminal split-screen or overlay techniques

## Keyboard Handling

### Navigation Mode (Ratatui)
- `g + key`: Switch between tasks (g+w=Workspace, g+s=Studio, etc.)
- `?`: Show help panel
- `Esc`: Return to previous context
- `Ctrl+c`: Exit

### Content Mode (Carbonyl)
- `Ctrl+Enter`: Submit forms
- `Alt+E`: Export
- `Alt+I`: Import
- `Esc`: Return to navigation mode

### Mode Switching
- `Esc`: Content → Navigation
- Enter/Space: Navigation → Content (on selected item)
- `g+key`: Direct navigation to task (switches to content mode)

## Implementation Strategy

### Phase 1: Ratatui Integration
1. Add Ratatui dependencies to Cargo.toml
2. Implement basic Ratatui layout (navigation, status, help)
3. Add keyboard event capture and routing
4. Implement mode switching logic

### Phase 2: Bridge Layer
1. Create render coordinator to alternate between Ratatui/Carbonyl
2. Implement input router for keyboard event distribution
3. Add state manager for UI synchronization
4. Handle Carbonyl process lifecycle (spawn, suspend, resume)

### Phase 3: Carbonyl Integration
1. Modify Carbonyl launch to support suspended mode
2. Implement content area rendering in Carbonyl mode
3. Add form submission handling from Carbonyl
4. Implement mode transition animations

### Phase 4: Polish
1. Add theme system (colors, icons from claude-code-rust patterns)
2. Implement performance telemetry (FPS, startup time)
3. Add virtual scrolling for large content
4. Optimize mode switching performance

## Technical Challenges

### Carbonyl Process Control
- Carbonyl expects to control entire terminal
- Need to suspend/resume Carbonyl rendering without killing process
- May need to patch Carbonyl or use alternative approach

### Terminal Graphics Coordination
- Carbonyl uses terminal graphics (Sixel, Kitty graphics)
- Ratatui uses escape sequences for text rendering
- Need to avoid conflicts between graphics protocols

### Performance Considerations
- Mode switching should be fast (<100ms)
- Carbonyl startup time may be noticeable
- Consider keeping Carbonyl process alive and suspended

### Fallback Strategy
- If hybrid approach proves infeasible, fall back to:
  - Option 1: Apply UX patterns to browser-based approach only
  - Option 2: Separate native TUI mode alongside browser mode

## Dependencies

### New Dependencies (Cargo.toml)
```toml
ratatui = "0.30"
crossterm = "0.29"
tokio = { version = "1", features = ["full"] }
```

### Existing Dependencies (Keep)
```toml
larql-core = { path = "../larql-core" }
larql-tty-io = { path = "../larql-tty-io" }
clap = { version = "4", features = ["derive"] }
reqwest = { version = "0.12", features = ["json"] }
dirs = "5.0"
```

## Directory Structure

```
crates/larql-terminal-browser/
├── src/
│   ├── main.rs              # Entry point, argument parsing
│   ├── ratatui/
│   │   ├── mod.rs           # Ratatui module
│   │   ├── layout.rs        # Layout management
│   │   ├── navigation.rs    # Navigation bar
│   │   ├── status.rs        # Status bar
│   │   ├── help.rs          # Help panel
│   │   └── keyboard.rs      # Keyboard handling
│   ├── bridge/
│   │   ├── mod.rs           # Bridge module
│   │   ├── coordinator.rs   # Render coordinator
│   │   ├── router.rs        # Input router
│   │   └── state.rs         # State manager
│   └── carbonyl/
│       ├── mod.rs           # Carbonyl module
│       ├── launcher.rs      # Process launcher
│       └── controller.rs    # Process control
├── docs/
│   └── hybrid-architecture.md
└── Cargo.toml
```

## Success Criteria

1. **Performance**: Mode switching <100ms, startup <2s
2. **Usability**: Intuitive keyboard shortcuts, smooth transitions
3. **Compatibility**: Works with existing workbench HTML/CSS/JS
4. **Reliability**: Graceful fallback if Carbonyl unavailable
5. **Maintainability**: Clean separation between Ratatui and Carbonyl layers
