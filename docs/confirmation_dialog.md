# Confirmation Dialog Widget

A modal confirmation dialog with a title, message, and configurable buttons over a semi-transparent backdrop. The dialog is centered on the viewport and supports blocking mode.

## Overview

The confirmation dialog system has three main types:

| Type | Purpose |
|------|---------|
| `ConfirmationDialog` | Wrapper widget that shows a modal dialog |
| `DialogButton` | A button entry with label, action, and optional style |
| `ButtonStyle` | Style variant: `Secondary`, `Default`, or `Danger` |

## Basic Usage

```rust
use iced::widget::{button, text};
use iced::Element;
use neverliie_iced_widgets::confirmation_dialog::ConfirmationDialog;

enum Message {
    Delete,
    Cancel,
    ShowDialog,
    DismissDialog,
}

fn view(show_dialog: bool) -> Element<'_, Message> {
    let content = button("Delete").on_press(Message::ShowDialog);

    if show_dialog {
        ConfirmationDialog::new(content, true, "Delete item?", "This cannot be undone.")
            .on_confirm(Message::Delete)
            .on_cancel(Message::Cancel)
            .on_dismiss(Message::DismissDialog)
            .into()
    } else {
        content.into()
    }
}
```

## Button Styles

`DialogButton` supports three style variants via `ButtonStyle`:

| Style | Description |
|-------|-------------|
| `Secondary` | Subtle, matches dialog background with a faint border. Default for cancel. |
| `Default` | Colored (primary). Default for confirm. |
| `Danger` | Red, for destructive actions. |

### Custom Button

```rust
use neverliie_iced_widgets::confirmation_dialog::{ConfirmationDialog, DialogButton, ButtonStyle};

ConfirmationDialog::new(content, true, "Delete?", "This cannot be undone.")
    .button(
        DialogButton::new("Delete", Message::Delete)
            .style(ButtonStyle::Danger),
    )
    .on_cancel(Message::Cancel)
    .on_dismiss(Message::Dismiss)
    .into()
```

### Multiple Custom Buttons

```rust
ConfirmationDialog::new(content, true, "Save changes?", "Unsaved work will be lost.")
    .button(
        DialogButton::new("Save", Message::Save)
            .style(ButtonStyle::Default),
    )
    .button(
        DialogButton::new("Don't Save", Message::DontSave)
            .style(ButtonStyle::Danger),
    )
    .on_cancel(Message::Cancel)
    .on_dismiss(Message::Dismiss)
    .into()
```

## Blocking Dialogs

Call `.blocking()` to prevent dismissal by clicking outside or pressing Escape. The user must click a button to proceed. A pulsing border flashes when a blocked dismiss is attempted.

```rust
ConfirmationDialog::new(content, true, "Warning", "You must acknowledge this.")
    .on_confirm(Message::Ok)
    .on_dismiss(Message::Dismiss)
    .blocking()
    .into()
```

## Keyboard Navigation

When the dialog is open, the following keys are handled:

| Key | Action |
|-----|--------|
| `Tab` | Cycle focus between buttons |
| `Enter` | Activate focused button |
| `Escape` | Dismiss dialog (unless blocking) |

## How It Works

1. `ConfirmationDialog` wraps your base content widget and takes an `is_open` flag
2. When `is_open` is true, a `DialogOverlay` is created during the overlay phase
3. The overlay renders: a semi-transparent backdrop, the dialog background with border/shadow, title and message text, and styled buttons
4. The dialog auto-sizes based on measured text widths, clamped to a max width of 420px
5. Buttons are centered horizontally at the bottom of the dialog
6. Clicking a button publishes its action message and dismisses the dialog
7. Clicking outside (or pressing Escape) publishes the `on_dismiss` message
8. In blocking mode, outside clicks trigger a pulse animation instead of dismissing

## Theming

Styling is controlled via the `Catalog` trait, implemented for `iced::Theme` by default. Use `.style()` for a custom style function or `.class()` for a theme class.

### Style Properties

```rust
pub struct Style {
    pub backdrop_color: Color,                // Semi-transparent backdrop
    pub background: Background,               // Dialog background
    pub border: Border,                       // Dialog border
    pub shadow: Shadow,                       // Dialog drop shadow
    pub title_color: Color,                   // Title text color
    pub message_color: Color,                 // Message text color
    pub secondary_button_background: Background,  // Secondary button bg
    pub secondary_button_border: Border,          // Secondary button border
    pub secondary_button_text_color: Color,       // Secondary button text
    pub button_background: Background,        // Primary button background
    pub button_border: Border,                // Primary button border
    pub button_text_color: Color,             // Primary button text
    pub danger_button_background: Background, // Danger button background
    pub danger_button_border: Border,         // Danger button border
    pub danger_button_text_color: Color,      // Danger button text
}
```

## API Reference

### `ConfirmationDialog`

```rust
ConfirmationDialog::new(content, is_open, title, message)
    .on_confirm(message)            // Add confirm button (primary style)
    .on_cancel(message)             // Add cancel button (secondary style)
    .on_dismiss(message)            // Set dismiss message
    .button(dialog_button)          // Add custom button
    .blocking()                     // Prevent outside dismiss
    .no_pointer()                   // Don't claim the pointer cursor on hover
    .style(style_fn)                // Custom style function
    .class(class)                   // Theme class
```

### `DialogButton`

```rust
DialogButton::new(label, action)    // Create button
    .style(ButtonStyle::Danger)     // Set style variant
```
