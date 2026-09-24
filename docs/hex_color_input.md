# Hex Color Input Widget

Compact hex color input pill with an optional alpha percentage and a floating color picker dialog.

Reference layout: `[■ FFFFFF | 100 %]`.

- The swatch rectangle opens the floating `ColorPicker` dialog.
- The hex field edits `RRGGBB` (3 or 6 hex digits, `#` optional).
- The percentage field edits alpha (`0..=100`, optional via `.show_alpha(false)`).
- When the value is a gradient, the hex field becomes the static text `"gradient"`, alpha is hidden, and an angle field with the `ANGLE_SVG` icon appears.

```rust
use neverliie_iced_widgets::hex_color_input::{HexColorInput, HexColorValue};

HexColorInput::new(value, Message::ColorChanged, show_picker, Message::Open, Message::Close)
    .show_alpha(true)
    .position(neverliie_iced_widgets::overlay::Position::BottomLeft)
```

`HexColorValue` is either `Solid(Color)` or `Gradient { gradient, angle }` (angle in degrees `0..360`). Picker selections map back through `on_change` / `on_submit`, preserving the angle.
