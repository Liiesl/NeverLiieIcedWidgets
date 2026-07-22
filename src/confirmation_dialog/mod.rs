//! Modal confirmation dialog with configurable buttons.
//!
//! This module provides [`ConfirmationDialog`], a widget that shows a
//! centered modal dialog with a title, message, and configurable buttons
//! over a semi-transparent backdrop.
//!
//! # Overview
//!
//! The confirmation dialog system has two main types:
//!
//! - [`ConfirmationDialog`] — wraps content and shows a modal dialog
//! - [`DialogButton`] — a button entry with label, action, and optional style
//!
//! # Example
//!
//! ```no_run
//! use iced::widget::{button, text};
//! use iced::Element;
//! use neverlie_iced_widgets::confirmation_dialog::ConfirmationDialog;
//!
//! enum Message {
//!     Delete,
//!     Cancel,
//!     DismissDialog,
//! }
//!
//! fn view(show_dialog: bool) -> Element<'_, Message> {
//!     let content = button("Delete Item").on_press(Message::Delete);
//!
//!     if show_dialog {
//!         ConfirmationDialog::new(content, show_dialog, "Are you sure?", "This action cannot be undone.")
//!             .on_confirm(Message::Delete)
//!             .on_cancel(Message::Cancel)
//!             .on_dismiss(Message::DismissDialog)
//!             .into()
//!     } else {
//!         content.into()
//!     }
//! }
//! ```
//!
//! # Button Styles
//!
//! [`DialogButton`] supports three style variants via [`ButtonStyle`]:
//!
//! - **`Secondary`** — subtle, matches dialog background with a faint border.
//!   Default for the cancel action.
//! - **`Default`** — colored (primary). Default for the confirm action.
//! - **`Danger`** — red, for destructive actions.
//!
//! ```ignore
//! use neverlie_iced_widgets::confirmation_dialog::{ConfirmationDialog, DialogButton, ButtonStyle};
//!
//! ConfirmationDialog::new(content, true, "Delete?", "This cannot be undone.")
//!     .button(
//!         DialogButton::new("Delete", Message::Delete)
//!             .style(ButtonStyle::Danger),
//!     )
//!     .on_cancel(Message::Cancel)
//!     .on_dismiss(Message::Dismiss)
//!     .into()
//! ```
//!
//! # Blocking Dialogs
//!
//! Call [`.blocking()`](ConfirmationDialog::blocking) to prevent dismissal
//! by clicking outside or pressing Escape. The user must click a button to
//! proceed. A pulsing border flashes when a blocked dismiss is attempted.
//!
//! ```ignore
//! ConfirmationDialog::new(content, true, "Warning", "You must acknowledge this.")
//!     .on_confirm(Message::Ok)
//!     .on_dismiss(Message::Dismiss)
//!     .blocking()
//!     .into()
//! ```
//!
//! # Pointer Cursor Override
//!
//! Call [`.no_pointer()`](ConfirmationDialog::no_pointer) to prevent the
//! dialog overlay from claiming the cursor interaction. When enabled, the
//! overlay always returns the default cursor, keeping the base cursor
//! available to widgets underneath the dialog. This preserves hover
//! detection (enter/exit events) in the widget tree below.
//!
//! The trade-off is that the mouse cursor won't change to a pointer when
//! hovering dialog buttons. Button hover visual feedback (the brightened
//! background) still works.
//!
//! ```ignore
//! ConfirmationDialog::new(content, true, "Delete?", "This cannot be undone.")
//!     .on_confirm(Message::Delete)
//!     .on_cancel(Message::Cancel)
//!     .no_pointer()
//!     .into()
//! ```
//!
//! # Theming
//!
//! Styling is controlled via the [`Catalog`] trait, implemented by
//! [`iced::Theme`] by default. Use [`.style()`](ConfirmationDialog::style)
//! for a custom style function or [`.class()`](ConfirmationDialog::class)
//! for a theme class.
//!
//! [`ConfirmationDialog`]: struct.ConfirmationDialog
//! [`DialogButton`]: struct.DialogButton
//! [`ButtonStyle`]: enum.ButtonStyle
//! [`Catalog`]: trait.Catalog

mod manager;
mod overlay;

pub use manager::ConfirmationDialog;

use iced::{Background, Border, Color, Shadow};

/// A button in the confirmation dialog.
///
/// Create with [`DialogButton::new`].
pub struct DialogButton<'a, Message> {
    label: &'a str,
    action: Message,
    style: Option<ButtonStyle>,
}

impl<'a, Message> DialogButton<'a, Message> {
    /// Creates a new button with the given label and action message.
    pub fn new(label: &'a str, action: Message) -> Self {
        Self {
            label,
            action,
            style: None,
        }
    }

    /// Sets a custom style for this button.
    #[must_use]
    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = Some(style);
        self
    }
}

/// Style variant for a dialog button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonStyle {
    /// Subtle button matching the dialog background, with a faint border.
    /// Used by default for the confirm action.
    Secondary,
    /// Primary/colored button appearance.
    /// Used by default for the cancel action.
    Default,
    /// Danger/destructive action appearance.
    Danger,
}

impl Default for ButtonStyle {
    fn default() -> Self {
        ButtonStyle::Secondary
    }
}

/// Style for a [`ConfirmationDialog`].
#[derive(Debug, Clone)]
pub struct Style {
    /// Semi-transparent backdrop color covering the viewport.
    pub backdrop_color: Color,
    /// Dialog background.
    pub background: Background,
    /// Dialog border.
    pub border: Border,
    /// Dialog drop shadow.
    pub shadow: Shadow,
    /// Title text color.
    pub title_color: Color,
    /// Message text color.
    pub message_color: Color,
    /// Secondary button background (matches dialog bg).
    pub secondary_button_background: Background,
    /// Secondary button border.
    pub secondary_button_border: Border,
    /// Secondary button text color.
    pub secondary_button_text_color: Color,
    /// Default/primary button background.
    pub button_background: Background,
    /// Default/primary button border.
    pub button_border: Border,
    /// Default/primary button text color.
    pub button_text_color: Color,
    /// Danger button background.
    pub danger_button_background: Background,
    /// Danger button border.
    pub danger_button_border: Border,
    /// Danger button text color.
    pub danger_button_text_color: Color,
}

/// A styling function for a [`ConfirmationDialog`].
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme) -> Style + 'a>;

/// Theme catalog for confirmation dialog styling.
///
/// Implemented for [`iced::Theme`] by default, pulling colors from the
/// extended palette.
pub trait Catalog {
    /// The style class.
    type Class<'a>;

    /// Returns the default class for this theme.
    fn default<'a>() -> Self::Class<'a>;

    /// Resolves a class into a concrete [`Style`].
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

impl Catalog for iced::Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> StyleFn<'a, Self> {
        Box::new(default_style)
    }

    fn style(&self, class: &StyleFn<'_, Self>) -> Style {
        class(self)
    }
}

/// Default style derived from the iced theme palette.
pub fn default_style(theme: &iced::Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        backdrop_color: Color {
            a: 0.5,
            ..palette.background.weak.color
        },
        background: palette.background.weak.color.into(),
        border: Border {
            width: 1.0,
            radius: 8.0.into(),
            color: palette.background.strong.color,
        },
        shadow: Shadow::default(),
        title_color: palette.background.weak.text,
        message_color: palette
            .background
            .weak
            .text
            .scale_alpha(0.8),
        secondary_button_background: palette.background.weak.color.into(),
        secondary_button_border: Border {
            width: 1.0,
            radius: 4.0.into(),
            color: palette.background.strong.color,
        },
        secondary_button_text_color: palette.background.weak.text,
        button_background: palette.primary.strong.color.into(),
        button_border: Border {
            width: 0.0,
            radius: 4.0.into(),
            color: Color::TRANSPARENT,
        },
        button_text_color: palette.primary.strong.text,
        danger_button_background: Color::from_rgb(0.7, 0.2, 0.2).into(),
        danger_button_border: Border {
            width: 0.0,
            radius: 4.0.into(),
            color: Color::TRANSPARENT,
        },
        danger_button_text_color: Color::WHITE,
    }
}

impl Default for Style {
    fn default() -> Self {
        Self {
            backdrop_color: Color {
                a: 0.5,
                ..Color::from_rgb(0.0, 0.0, 0.0)
            },
            background: Background::Color(Color::from_rgb(0.12, 0.12, 0.18)),
            border: Border {
                color: Color::from_rgb(0.35, 0.35, 0.5),
                width: 1.0,
                radius: 8.0.into(),
            },
            shadow: Shadow::default(),
            title_color: Color::from_rgb(0.9, 0.9, 0.9),
            message_color: Color::from_rgb(0.7, 0.7, 0.75),
            secondary_button_background: Background::Color(Color::from_rgb(0.12, 0.12, 0.18)),
            secondary_button_border: Border {
                color: Color::from_rgb(0.35, 0.35, 0.5),
                width: 1.0,
                radius: 4.0.into(),
            },
            secondary_button_text_color: Color::from_rgb(0.9, 0.9, 0.9),
            button_background: Background::Color(Color::from_rgb(0.2, 0.25, 0.4)),
            button_border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 4.0.into(),
            },
            button_text_color: Color::WHITE,
            danger_button_background: Background::Color(Color::from_rgb(0.7, 0.2, 0.2)),
            danger_button_border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 4.0.into(),
            },
            danger_button_text_color: Color::WHITE,
        }
    }
}

/// Internal layout constants.
pub(crate) const DIALOG_MAX_WIDTH: f32 = 420.0;
pub(crate) const MIN_BUTTON_WIDTH: f32 = 80.0;
