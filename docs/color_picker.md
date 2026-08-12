# Color Picker Widget

A dialog-style color picker input element. Ported from `iced_aw`'s `color_picker` module, with the overlay reworked to mirror the PySide6 `BetterColorDialog`: a hue ring and saturation/value square on the left, RGB(A)/HSV tabbed gradient sliders with channel fields and a hex input, and on the right Original/New preview panels, tabbed swatch sets, a recent colors grid and the Reset/OK/Cancel buttons.

## Overview

| Type | Purpose |
|------|---------|
| `ColorPicker` | The picker widget; wraps an underlay element and shows the dialog as an overlay |
| `State` | Per-widget state: selected color, focus, tab, swatches, recent colors |
| `ActiveTab` | The active controls tab: `Rgb` or `Hsv` |
| `SwatchSet` | A named set of swatch colors shown in the swatch tab bar |
| `Status` | Style status: Active, Hovered, Pressed, Disabled, Focused, Selected |
| `StyleState` | Internal style state: Active, Selected, Hovered, Focused |

Swatches and recent colors are kept in memory only (no persistence), and all styling is derived from the active iced `Theme` palette.

## Basic Usage

Use the `color_picker` shortcut function:

```rust
use iced::{widget::button, Color, Element};
use neverliie_iced_widgets::color_picker::color_picker;
use neverliie_iced_widgets::overlay::Position;

#[derive(Clone, Debug)]
enum Message {
    Open,
    Cancel,
    Submit(Color),
    ColorChanged(Color),
}

fn view(show_picker: bool, color: Color) -> Element<'_, Message> {
    color_picker(
        show_picker,
        color,
        button("Pick color").on_press(Message::Open),
        Message::Cancel,
        Message::Submit,
    )
    .into()
}
```

Or use the struct builder API:

```rust
use neverliie_iced_widgets::color_picker::ColorPicker;
use iced::widget::{Button, Text};

let picker = ColorPicker::new(
    true,                                   // show the picker overlay
    Color::default(),                       // initial color
    Button::new(Text::new("Pick color"))
        .on_press(Message::Open),           // underlay element
    Message::Cancel,                        // cancel button message
    Message::Submit,                        // submit callback: Fn(Color) -> Message
);
```

The arguments are:

1. `show_picker` — whether the overlay is visible
2. `color` — the initial color to show
3. `underlay` — the `Element` the picker wraps around (e.g. a button)
4. `on_cancel` — message sent when the Cancel button is pressed
5. `on_submit` — `Fn(Color) -> Message` called with the picked color when OK is pressed

## Real-Time Color Changes

Publish a message on every selection change (ring, square, bars, hex input, swatch, keyboard), not just on submit:

```rust
color_picker(show, color, underlay, Message::Cancel, Message::Submit)
    .on_color_change(Message::ColorChanged)
```

There is also a one-shot shortcut equivalent to the builder call above:

```rust
use neverliie_iced_widgets::color_picker::color_picker_with_change;

color_picker_with_change(
    show, color, underlay,
    Message::Cancel,
    Message::Submit,
    Message::ColorChanged,
)
```

While the picker is open it "owns" the color value: re-renders with a live-updated `color` argument do **not** reset the selection. The initial color shown in the Original preview panel is frozen at the open-time color and only refreshed when the picker is reopened.

## Positioning

Set where the dialog appears with `.position(...)`, using the same `Position` strategies as the [overlay widget](overlay.md) (`neverliie_iced_widgets::overlay::Position`):

```rust
use neverliie_iced_widgets::overlay::Position;

color_picker(show, color, underlay, Message::Cancel, Message::Submit)
    .position(Position::BottomRight)      // relative to the underlay
    .position(Position::ViewportCenter)   // relative to the viewport
    .position(Position::FollowCursor)     // follows the mouse
```

Without a position (the default) the dialog is centered over the underlay and bounced back into the viewport, so it always stays fully on screen. Cursor-following positions request redraws on every cursor move so the dialog tracks the mouse.

## Dialog Layout

The dialog is split into two panes:

### Left Pane

- **Hue ring** — a 300px circular ring; drag on the ring band to pick the hue, or scroll the mouse wheel over it to nudge the hue
- **Saturation/Value square** — drag inside the square to pick saturation (x-axis) and value (y-axis); an outline circle indicates the current position
- **Controls tab bar** — switch between the `RGB(A)` and `HSV` tabs
- **Gradient slider bars** — one per channel of the active tab: R, G, B, A or H, S, V; drag to adjust, or click to jump
- **Channel value fields** — seven text inputs (`[R, G, B, A, H, S, V]`); RGB(A) channels and S/V are on the `0..=255` scale, hue on `0..=359`. Values are clamped on input
- **Hex input** — freeform hex color input, see below

### Right Pane

- **Original / New preview panels** — the open-time color vs. the current selection, over a checkerboard pattern (alpha-aware)
- **Swatch tab bar** — named swatch sets; switch sets by clicking a tab, close a set with its "x" mark, and create a new set via the trailing "+" tab
- **New swatch set prompt** — typing a name (followed by Enter or the Add button) creates an empty set and selects it; empty names are ignored
- **Add-current-color button** — inserts the current color at the front of the active swatch set
- **Recent colors grid** — up to 12 previously submitted colors
- **Buttons** — Reset (restores the open-time color), Cancel, OK

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

`Overlay → Ring → Square → channels (R,G,B,A or H,S,V) → tabs → Swatches → [NewSetName] → Reset → Cancel → Submit`

- **Arrow keys** adjust the focused control:
  - Ring / H-S bar: hue by `1°` (wraps via `% 360`)
  - Square: saturation / value by `0.005` per press (Up/Down swap places with Left/Right on the S and V bars, mirroring the reference dialog)
  - R/G/B/A bars: channel by `1` on the `0..=255` scale
- **Enter / Space** activates the focused tab, Reset button, or swatch cell
- **Arrow keys** move a cell cursor through the swatch grid (clamped to the set's bounds); **Enter/Space** applies the focused swatch
- Typing goes to the hex and channel value inputs while they hold focus; the outer cycle is skipped while a text input is focused
- **Escape** aborts the "new swatch set" name prompt; **Tab** while inside the name input refocuses the cycle

## Styling

All default styling is derived from the active iced `Theme` extended palette, following the dark, panel-based look of the reference dialog (`#2D2D2D` background, `#333333` panels, `#FFFFFF`/`#BBBBBB` text, `#3A3A3A`/`#4A4A4A` neutral surfaces, danger-toned Reset button).

The `Style` struct covers every surface of the dialog:

```rust
pub struct Style {
    pub background: Background,             // Dialog background
    pub border_radius: f32,                 // Dialog corner radius
    pub border_width: f32,                  // Dialog border width
    pub border_color: Color,                // Dialog border color
    pub panel_background: Background,       // Inner panes/containers
    pub panel_border_radius: f32,
    pub panel_border_color: Color,
    pub text_primary: Color,                // Main text (#FFFFFF)
    pub text_secondary: Color,              // Labels/secondary (#BBBBBB)
    pub tab_background: Background,         // Inactive tab
    pub tab_selected_background: Background,
    pub tab_hover_background: Background,
    pub tab_border_color: Color,
    pub bar_border_radius: f32,             // Gradient slider bars
    pub bar_border_width: f32,
    pub bar_border_color: Color,
    pub slider_groove_border_color: Color,  // Slider grooves (#3A3A3A)
    pub slider_handle_background: Color,    // Slider handles
    pub slider_handle_hover_background: Color,
    pub slider_handle_border_color: Color,
    pub sv_square_indicator_radius: f32,    // S/V square outline circle
    pub checker_color_1: Color,             // Checkerboard tiles (light)
    pub checker_color_2: Color,             // Checkerboard tiles (dark)
    pub checker_alpha_1: Color,             // Alpha groove checker (light)
    pub checker_alpha_2: Color,             // Alpha groove checker (dark)
    pub preview_border_color: Color,        // Original/New panels
    pub swatch_border_color: Color,         // Swatch buttons
    pub swatch_hover_border_color: Color,
    pub reset_background: Color,            // Reset button
    pub reset_hover_background: Color,
}
```

Apply a custom style based on the theme and [`Status`]:

```rust
use neverliie_iced_widgets::color_picker::{color_picker, style};

color_picker(show, color, underlay, Message::Cancel, Message::Submit)
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

1. `ColorPicker` is a custom iced widget that forwards layout, events, drawing and operations to its underlay, and returns the dialog through its `overlay()` implementation when `show_picker` is true
2. A `State` tree node per widget holds the selection state; `diff` synchronizes it with the widget's `color` argument only at open time (`force_synchronize`), keeping the Original preview frozen while the dialog is open
3. The hue ring and S/V square are cached `canvas` widgets; the caches are cleared whenever the color or layout changes
4. What happens on interaction:
   - Dragging the ring/square/bars updates the color and fires `on_color_change` (if set)
   - The hex input and channel fields write into `State`, are validated/clamped, and reformat the display strings
   - Swatch clicks, the add-current-color button and submit push into the swatch/recent lists with byte-exact deduplication
5. The dialog is positioned via `Node::center_and_bounce` (default) or resolved like the overlay's `Position` strategies, clamped to the viewport
6. Keyboard input is only handled when the overlay's internal `Focus` is set, producing the Tab cycle and arrow-key adjustments described above
7. Submitting publishes `on_submit(picked_color)`; canceling publishes `on_cancel`

## API Reference

### `color_picker` (shortcut)

```rust
color_picker(show_picker, color, underlay, on_cancel, on_submit)
color_picker_with_change(show_picker, color, underlay, on_cancel, on_submit, on_color_change)
```

### `ColorPicker`

```rust
ColorPicker::new(show_picker, color, underlay, on_cancel, on_submit)

// Builder methods
    .on_color_change(callback)    // Fn(Color) -> Message, real-time updates
    .position(position)           // overlay Position (default: centered)
    .style(style_fn)              // Fn(&Theme, Status) -> Style
    .class(class)                 // Catalog class
```

The struct also implements `From<ColorPicker> for Element`, so `.into()` works everywhere.

### `State`

```rust
State::new(color)   // New state for a picker widget
state.reset()       // Reset the state's color/focus
```

Hex parsing (`#RGB`, `#RGBA`, `#RRGGBB`, `#RRGGBBAA` with optional `#`) and HSV conversion are implemented internally by the widget; they are not part of the public API.

### `StyleState`

```rust
StyleState::Active     // Default state
StyleState::Selected   // Selected state
StyleState::Hovered    // Hovered state
StyleState::Focused    // Focused state
```