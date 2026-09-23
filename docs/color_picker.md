# Color Picker Widget

A color picker dialog. Ported from `iced_aw`'s `color_picker` module, reworked to a single-column two-tab layout: top-level `Color | Library` tabs (in the draggable header for the floating window), a saturation/value square with a hue slider below it, HSV/RGB(A) tabbed gradient sliders with channel fields and a hex input in the Color tab (HSV open by default), swatch sets plus a recent colors grid in the Library tab, and shared Original/New preview panels with the Reset/Eyedropper/OK buttons below the hex input (the former Cancel slot now hosts the [eye dropper](#eye-dropper); the floating window keeps its header "x" as a cancel path).

The dialog comes in two shapes:

| Type | Purpose |
|------|---------|
| `ColorPicker` | **Generic inline widget** — plant it into any builder like a regular widget; always visible, no spawn button required |
| `FloatingColorPicker` | **Floating window mode** — wraps an underlay (typically a button) and spawns the dialog inside a draggable window-like shell with a header (`Color | Library` tabs + drag area + close "x" button) |
| `State` / `FloatingState` | Per-widget state: selected color, focus, tab, swatches, recent colors |
| `ActiveTab` | The active controls tab: `Rgb` or `Hsv` |
| `PickerTab` | The top-level tab: `Color` or `Library` |
| `SwatchSet` | A named set of swatch colors shown in the swatch tab bar |
| `Status` | Style status: Active, Hovered, Pressed, Disabled, Focused, Selected |
| `StyleState` | Internal style state: Active, Selected, Hovered, Focused |

Swatches and recent colors are kept in memory only (no persistence), and all styling is derived from the active iced `Theme` palette.

## Mode 1 — Generic Inline Widget

`ColorPicker` is a plain always-visible widget. There is no underlay and no `show_picker` flag: control visibility by planting or not planting the element.

```rust
use iced::{Color, Element};
use neverliie_iced_widgets::color_picker::color_picker;

#[derive(Clone, Debug)]
enum Message {
    Cancel,
    Submit(Color),
    ColorChanged(Color),
}

fn view(color: Color) -> Element<'_, Message> {
    // Plant it anywhere in any builder:
    iced::widget::column![
        iced::widget::text("Accent color"),
        color_picker(color, Message::Cancel, Message::Submit)
            .on_color_change(Message::ColorChanged),
    ]
    .into()
}
```

Or use the struct builder API:

```rust
use neverliie_iced_widgets::color_picker::ColorPicker;

let picker = ColorPicker::new(
    color,                    // initial color
    Message::Cancel,          // placeholder message (no cancel UI in inline mode)
    Message::Submit,          // submit callback: Fn(Color) -> Message
)
.on_color_change(Message::ColorChanged);
```

`on_cancel` is still a required argument in inline mode, but the dialog has no cancel control anymore: the internal inputs and buttons only reuse the message as a dummy publication that the widget intercepts before it reaches the application, so any cheap-to-build message works. Only the [floating window](#mode-2--floating-window) publishes it for real, via its header "x" close button.

While mounted the picker "owns" the value internally. The `color` argument only re-seeds the selection when it differs from both the previous argument (so echoed live updates are ignored) and the current internal selection — passing a genuinely new color resets the dialog.

## Mode 2 — Floating Window

`FloatingColorPicker` keeps the classic spawn-by-button flow, but the spawned dialog is a free-floating window-like shell (still a regular overlay/widget, not a separate OS window):

- a **header** with an empty dragging area and an **"x" close button** publishing `on_cancel`;
- the whole header drags the window anywhere inside the viewport;
- `.position(...)` sets the *initial* position; after dragging, the dragged position wins and **survives close/reopen**.

```rust
use iced::widget::button;
use neverliie_iced_widgets::color_picker::floating_color_picker;
use neverliie_iced_widgets::overlay::Position;

let floating = floating_color_picker(
    show_picker,                       // whether the window is open
    color,
    button("Pick color").on_press(Message::Open),
    Message::Cancel,                   // header "x" close button
    Message::Submit,
)
.position(Position::ViewportCenter);   // initial placement only
```

## Real-Time Color Changes

Both modes publish a message on every selection change (ring, square, bars, hex input, swatch, keyboard), not just on submit:

```rust
picker.on_color_change(Message::ColorChanged)          // inline
floating.on_color_change(Message::ColorChanged)        // floating
```

One-shot shortcuts exist for both:

```rust
color_picker_with_change(color, cancel, submit, change)
floating_color_picker_with_change(show, color, underlay, cancel, submit, change)
```

While a floating picker is open it owns the value: re-renders with a live-updated `color` argument do **not** reset the selection. The initial color shown in the Original preview panel is frozen at the open-time color and only refreshed when the picker is reopened.

## Positioning (floating mode)

`.position(...)` uses the same strategies as the [overlay widget](overlay.md) (`neverliie_iced_widgets::overlay::Position`) but only for the first open:

```rust
floating.position(Position::BottomRight)      // relative to the underlay
floating.position(Position::ViewportCenter)   // relative to the viewport
floating.position(Position::FollowCursor)     // follows the mouse at spawn
```

Without a position the window first appears centered over the underlay and bounces back into the viewport. Every frame the window is clamped so it stays fully visible, even after viewport resizes. Dragging by the header overrides all of this; the last dragged spot persists across close/reopen (per picker instance).

## Dialog Layout

The dialog is a single column with two top-level tabs (`Color | Library`,
in the draggable header for the floating window, on top for the inline
widget):

### Color Tab (raw picking)

- **Saturation/Value square** — drag inside the square to pick saturation (x-axis) and value (y-axis); an outline circle indicates the current position
- **Hue slider** — a normal horizontal gradient slider below the square; drag or scroll to pick the hue
- **Controls tab bar** — switch between the `HSV` and `RGB(A)` tabs (`HSV` is leftmost and default)
- **Gradient slider bars** — four bars per tab: R, G, B, A on the RGB(A) tab and H, S, V, A on the HSV tab (the alpha bar is always present); drag to adjust, or click to jump
- **Channel value fields** — seven text inputs (`[R, G, B, A, H, S, V]`); RGB(A) channels and S/V are on the `0..=255` scale, hue on `0..=359`. Values are clamped on input
- **Hex input** — freeform hex color input, see below

### Library Tab

- **Swatch tab bar** — named swatch sets; switch sets by clicking a tab, close a set with its "x" mark, and create a new set via the trailing "+" tab
- **New swatch set prompt** — typing a name (followed by Enter or the Add button) creates an empty set and selects it; empty names are ignored
- **Add-current-color button** — inserts the current color at the front of the active swatch set
- **Recent colors grid** — up to 12 previously submitted colors

### Shared Footer (both tabs)

- **Original / New preview panels** — the open-time color vs. the current selection, over a checkerboard pattern (alpha-aware)
- **Buttons below the hex input** — Reset (restores the open-time color), Eyedropper (see below), OK

## Eye Dropper

The dialog includes an **in-window eye dropper** (the button in the former Cancel slot). It samples colors from a frozen snapshot of the *application window contents only* — it never captures anything outside the window. When active, a **magnifier lens** follows the cursor: a 13×13 zoomed pixel grid with a crosshair marking the exact pixel and a `#RRGGBB` pill below. Pixels outside the captured area render as checkerboard.

Interaction:

| Input | Action |
|-------|--------|
| Left click / Enter / Space | Commit the hovered pixel (fires `on_color_change`) and leave picking mode |
| Right click / Escape | Abort without changing the selection |
| Arrow keys | Nudge the hovered pixel by one screen pixel |

While picking, every mouse/keyboard event is consumed: nothing beneath reacts (the floating dialog fully freezes the underlying UI; the inline widget suppresses interaction inside its own tree branch). The lens is always clamped so it stays fully inside the window.

Because iced widgets cannot spawn `Task`s themselves, the capture is plumbed through the application:

1. Hand a clone of a shared [`DropperBuffer`](crate::color_picker::DropperBuffer) to the picker via `.dropper_buffer(...)` and set `.on_dropper_capture(|| Msg::Capture)`. Without both, the eyedropper button renders disabled.
2. On `Msg::Capture`, run `window::latest().and_then(window::screenshot)` and map the result to e.g. `Msg::Shot`.
3. On `Msg::Shot(screenshot)`, call `buffer.store(&screenshot)`.

The widget picks the fresh frame up on its next update pass and enters picking mode; each frame is consumed exactly once and stale frames are discarded on activation. Pressing the eyedropper button again while waiting for the capture aborts the request.

```rust
# use neverliie_iced_widgets::color_picker::{floating_color_picker_with_change, DropperBuffer};
# use iced::{Color, Task, window};
# #[derive(Clone, Debug)]
# enum Message { Capture, Shot(window::screenshot::Screenshot), Cancel, Submit(Color) }
let buffer = DropperBuffer::new();

let floating = floating_color_picker_with_change(
    true,
    Color::BLACK,
    iced::widget::button("Pick color").on_press(Message::Cancel),
    Message::Cancel,
    Message::Submit,
    Message::Submit,
)
.dropper_buffer(buffer.clone())
.on_dropper_capture(|| Message::Capture);

fn update(message: Message, buffer: &DropperBuffer) -> Task<Message> {
    match message {
        Message::Capture => window::latest().and_then(window::screenshot).map(Message::Shot),
        Message::Shot(screenshot) => {
            buffer.store(&screenshot);
            Task::none()
        }
        _ => Task::none(),
    }
}
```

## Hex Input

The hex field parses `#RGB`, `#RGBA`, `#RRGGBB` and `#RRGGBBAA`:

- A leading `#` is added automatically when the text is exactly 3, 4, 6 or 8 hex digits
- Shorter forms are expanded per nibble (`#f80` → `#FF8800`)
- `#RGB` / `#RRGGBB` keep the current alpha instead of defaulting it
- Invalid characters are filtered out as you type (hex digits and `#` only, max 9 chars)
- Valid input reformats the field to canonical `#RRGGBBAA`

## Swatches and Recent Colors

- Swatch sets and recent colors live in memory and are **not persisted**
- Clicking a swatch applies its color and fires `on_color_change`
- The "add current color" button (and submitting a color) deduplicates by RGBA bytes, inserts at the front, and truncates to the limits:
  - `24` swatches per set
  - `12` recent colors
- New sets start empty; the last remaining set cannot be closed (no "x" on a single set)
- Sets are identified by name; there is no rename support

## Keyboard Navigation

Focus moves with **Tab** (and back with **Shift+Tab**) through a cycle that adapts to the active tab:

`Overlay → Ring → Square → channels (R,G,B,A or H,S,V,A) → Hex input → [NewSetName] → tabs → Swatches → Reset → Dropper → Submit`

(`[NewSetName]` only appears in the cycle while the "new swatch set" naming prompt is open.)

- **Arrow keys** adjust the focused control:
  - Ring / H-S bar: hue by `1°` (wraps via `% 360`)
  - Square: saturation / value by `0.005` per press (Up/Down swap places with Left/Right on the S and V bars, mirroring the reference dialog)
  - R/G/B/A bars: channel by `1` on the `0..=255` scale
- **Enter / Space** activates the focused tab, Reset button, eyedropper button (starting a [capture request](#eye-dropper)), or swatch cell
- **Arrow keys** move a cell cursor through the swatch grid (clamped to the set's bounds); **Enter/Space** applies the focused swatch
- Typing goes to the hex and channel value inputs while they hold focus; the outer cycle is skipped while a text input is focused
- **Escape** aborts the "new swatch set" name prompt; **Tab** while inside the name input refocuses the cycle

## Styling

All default styling is derived from the active iced `Theme` extended palette, following the dark, panel-based look of the reference dialog (`#2D2D2D` background, `#333333` panels, `#FFFFFF`/`#BBBBBB` text, `#3A3A3A`/`#4A4A4A` neutral surfaces, danger-toned Reset button).

The `Style` struct covers every surface of the dialog (dialog background/border, panels, tabs, slider bars/handles/grooves, checkerboard tiles, preview/swatch borders, Reset button) plus the floating-window chrome:

```rust
pub struct Style {
    // ... dialog fields ...
    pub header_background: Background,       // Draggable window header strip
    pub header_border_color: Color,          // Divider line under the header
    pub close_button_background: Color,      // Header "x" button
    pub close_button_hover_background: Color,
    pub close_button_border_color: Color,
    pub close_symbol_color: Color,           // "x" glyph

    pub lens_backdrop: Color,                // Eye dropper magnifier backdrop
    pub lens_border_color: Color,            // Magnifier lens border
    pub lens_crosshair_color: Color,         // Crosshair over the exact pixel
    pub lens_pill_background: Color,         // Hex readout pill background
    pub lens_pill_text: Color,               // Hex readout pill text
}
```

Apply a custom style based on the theme and [`Status`]:

```rust
use neverliie_iced_widgets::color_picker::{color_picker, style};

color_picker(color, Message::Cancel, Message::Submit)
    .style(|theme, status| {
        let mut style = style::primary(theme, status);
        style.border_color = theme.extended_palette().primary.strong.color;
        style
    })
```

| Status | Effect on the default style |
|--------|-----------------------------|
| `Active` | Base style |
| `Hovered` | Bar borders take the accent color |
| `Focused` | Dialog/border colors take the primary color, tabs highlight |
| `Selected` | Active tab background takes the primary color |
| `Pressed` / `Disabled` | Base style |

## How It Works

1. `ColorPickerOverlay` (in `overlay.rs`) is the shared **dialog content view**: manual two-pane layout, hit-testing and canvas drawing used by both public widgets
2. The inline `ColorPicker` implements `Widget` directly and delegates each pass to the content view; its `State` re-seeds only on genuine external color changes
3. `FloatingColorPicker` forwards layout/events/drawing to its underlay and returns a `ColorPickerWindow` overlay element while shown; the window adds the draggable header, the close button and position persistence (`State::dialog_position`)
4. Dragging follows the same press/move/release idiom as the color bars: press on the header stores the grab offset, cursor moves update the stored origin (clamped to the viewport), release ends the drag
5. A `State` tree node per widget holds the selection state; the floating variant synchronizes with the provided color only at open time (`force_synchronize`), keeping the Original preview frozen while the window is open
6. The hue ring and S/V square are cached `canvas` widgets; the caches are cleared whenever the color or layout changes
7. Keyboard input is only handled when the content's internal `Focus` is set, producing the Tab cycle and arrow-key adjustments described above
8. Submitting publishes `on_submit(picked_color)`; canceling (the floating window's header "x") publishes `on_cancel`. The dialog buttons row hosts Reset, the eye dropper and OK.

## API Reference

### Shortcuts

```rust
// Inline
color_picker(color, on_cancel, on_submit)
color_picker_with_change(color, on_cancel, on_submit, on_color_change)

// Floating window
floating_color_picker(show_picker, color, underlay, on_cancel, on_submit)
floating_color_picker_with_change(show_picker, color, underlay, on_cancel, on_submit, on_color_change)
```

### `ColorPicker` (inline)

```rust
ColorPicker::new(color, on_cancel, on_submit)

// Builder methods
    .on_color_change(callback)    // Fn(Color) -> Message, real-time updates
    .dropper_buffer(buffer)       // enable the eye dropper (DropperBuffer)
    .on_dropper_capture(f)        // Fn() -> Message, capture request
    .style(style_fn)              // Fn(&Theme, Status) -> Style
    .class(class)                 // Catalog class
```

### `FloatingColorPicker` (window)

```rust
FloatingColorPicker::new(show_picker, color, underlay, on_cancel, on_submit)

// Builder methods
    .on_color_change(callback)    // Fn(Color) -> Message, real-time updates
    .dropper_buffer(buffer)       // enable the eye dropper (DropperBuffer)
    .on_dropper_capture(f)        // Fn() -> Message, capture request
    .position(position)           // initial position strategy (default: centered over underlay)
    .style(style_fn)              // Fn(&Theme, Status) -> Style
    .class(class)                 // Catalog class
```

Both structs implement `From<...> for Element`, so `.into()` works everywhere.

### State

```rust
State::new(color)         // New state for an inline picker
state.reset()             // Reset the state's color/focus

FloatingState::new(color) // New state for a floating picker
floating_state.reset()    // Reset the state's color/focus
```

Hex parsing (`#RGB`, `#RGBA`, `#RRGGBB`, `#RRGGBBAA` with optional `#`) and HSV conversion are implemented internally by the widget; they are not part of the public API.

### `StyleState`

```rust
StyleState::Active     // Default state
StyleState::Selected   // Selected state
StyleState::Hovered    // Hovered state
StyleState::Focused    // Focused state
```
