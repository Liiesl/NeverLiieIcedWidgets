# Split Button Widget

A split button with a dropdown menu. The button shows the currently selected option as a main action area plus a narrow arrow zone (with a divider) that opens the dropdown.

Options are `Item`s with a value, a label and an optional icon — the same model as `AdvancedDropdown` (re-exported as `split_button::{Item, MenuItem}`). The selected option's icon + label are shown on the main area; the menu renders every option with its icon in a fixed column and supports separators, keyboard navigation, scrolling, and flipping above the button when there is no room below.

## Behavior

| Zone | Click |
|------|-------|
| Main area | Executes the current selection via `on_press(selected)`. With no selection (or no `on_press`), opens the menu instead. |
| Arrow zone (`▼`) | Opens/closes the dropdown menu. |
| Menu row | Emits `on_select(value)` (**select-only**: changes display/default, does not execute). Clicking outside, `Esc`, or reopening closes without selecting. |

`Ctrl` + mouse wheel over the button cycles the selection.

## Basic Usage

```rust
use iced::widget::text;
use iced::Element;
use neverliie_iced_widgets::split_button::{Item, MenuItem, split_button};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action { Save, SaveAs, Export }

#[derive(Debug, Clone)]
enum Message {
    ActionSelected(Action),
    ActionPressed(Action),
}

fn view(selected: Option<Action>) -> Element<'_, Message> {
    // Built fresh on every view call (icons borrow nothing here).
    let options = [
        MenuItem::Item(
            Item::new(Action::Save, "Save").icon(text("💾").size(14)),
        ),
        MenuItem::Item(
            Item::new(Action::SaveAs, "Save As").icon(text("📝").size(14)),
        ),
        MenuItem::Separator,
        MenuItem::Item(Item::new(Action::Export, "Export")), // no icon: label shifts, column stays aligned
    ];

    split_button(options, selected, Message::ActionSelected)
        .placeholder("Choose an action...")
        .on_press(Message::ActionPressed)
        .into()
}

fn update(selected: &mut Option<Action>, message: Message) {
    match message {
        // Menu pick: only changes what the main button shows/does.
        Message::ActionSelected(action) => *selected = Some(action),
        // Main click: executes the current selection.
        Message::ActionPressed(action) => {
            *selected = Some(action);
            // ... perform the action ...
        }
    }
}
```

Icons can be any widget: `text("⧉")` glyphs, `image(...)` / SVG handles, or `LazyIcon`. Inside the menu, labels align in one column whether or not every row has an icon.

## API Reference

### `SplitButton` / `split_button()`

```rust
split_button(options, selected, on_select)
    .placeholder("...")   // shown when nothing is selected
    .on_press(fn)         // main-area click: fn(T) -> Message
    .on_open(msg)         // dropdown opened
    .on_close(msg)        // dropdown closed
    .width(len)           // button width (menu matches it)
    .menu_height(len)     // dropdown height
    .menu_max_height(px)  // cap: menu shrinks, list scrolls
    .padding(p)           // button + menu padding
    .text_size(px)        // label size
    .font(font)           // label font
    .handle(handle)       // arrow-zone glyph (default ▼)
    .border_radius(r)     // face corners (default 4)
    .menu_border_radius(r)
    .style(fn)            // Fn(&Theme, Status) -> Style
    .menu_style(fn)       // dropdown menu style (same as AdvancedDropdown menu)
    .class(c) / .menu_class(c)
```

### `Status`

```rust
pub enum Status {
    Active,                 // idle
    Hovered,                // cursor over either zone
    Opened { is_hovered },  // dropdown open
    Disabled,               // no options
}
```

### `Style`

Same idea as `button::Style` (`text_color`, `background`, `border`, `shadow`), plus `placeholder_color` and `handle_color` for the placeholder label and the chevron. The border color doubles as the divider and zone-highlight source, so it follows the text color (the border itself is not drawn).

Built-in variants mirror the native button ones — pass them to `.style()`:

```rust
use neverliie_iced_widgets::split_button;

split_button(options, selected, Message::Selected)
    .on_press(Message::Pressed)
    .style(split_button::danger) // primary (default), secondary, success, warning, danger
    .into()
```

## How It Works

1. `layout` measures the widest option label (like a pick list) so `Shrink` fits content, and lays out the selected option's icon in a fixed 16px column.
2. `update` hit-tests main vs arrow zones: main publishes `on_press`, arrow toggles `is_open`.
3. `draw` renders the shared background, a short dim divider, a per-zone hover tint (subtle on the main area, stronger on the arrow zone), the chevron handle (down when closed, up while open), and the selected icon + label/placeholder.
4. `overlay` builds the shared `advanced_dropdown` menu anchored to the button bounds (flips above when needed, capped by `menu_max_height`).
