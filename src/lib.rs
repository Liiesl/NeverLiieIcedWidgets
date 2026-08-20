//! # NeverLie Iced Widgets
//!
//! A reusable widget library for [iced](https://iced.rs).
//!
//! This crate provides additional widgets that follow iced's conventions:
//! builder pattern, generic `Message`/`Theme`/`Renderer` parameters,
//! and Elm-architecture compatibility.
//!
//! ## Available Widgets
//!
//! - [`overlay`] — Floating content positioned relative to parent, viewport, or cursor
//! - [`context_menu`] — Right-click context menu with submenu support
//! - [`confirmation_dialog`] — Modal confirmation dialog with configurable buttons
//! - [`ghost_text_input`] — Text input with animated ghost trail cursor effect
//! - [`slider_tooltip`] — Slider with a floating value tooltip during hover and drag
//! - [`lazy_icon`] — Lazy-loading icon widget with placeholder support
//! - [`ellipsis_text`] — Line-clamped text with ellipsis truncation
//! - [`color_picker`] — HSV/RGBA color picker with overlay (ported from iced_aw)
//!
//! Per-widget guides are also available in the repository's [`docs/`](https://github.com/anomalyco/NeverLiieIcedWidgets/tree/main/docs) folder.
//!
//! ## Quick Start
//!
//! ```no_run
//! use iced::widget::{button, text};
//! use iced::Element;
//! use neverliie_iced_widgets::overlay::{Floating, OverlayManager, Position};
//!
//! #[derive(Clone)]
//! enum Message {
//!     ShowPopup,
//! }
//!
//! fn view(show_popup: bool) -> Element<'_, Message> {
//!     let content = button("Show Popup").on_press(Message::ShowPopup);
//!
//!     if show_popup {
//!         OverlayManager::new(content)
//!             .overlay(
//!                 Floating::new(text("Hello from overlay!"))
//!                     .position(Position::BottomLeft),
//!             )
//!             .into()
//!     } else {
//!         content.into()
//!     }
//! }
//! ```

pub mod advanced_dropdown;
pub mod color_picker;
pub mod confirmation_dialog;
pub mod context_menu;
pub mod ellipsis_text;
pub mod ghost_text_input;
pub mod lazy_icon;
pub mod overlay;
pub mod slider_tooltip;
