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
//!
//! ## Quick Start
//!
//! ```no_run
//! use iced::widget::{button, text};
//! use iced::Element;
//! use never_lie_iced_widgets::overlay::{Floating, OverlayManager, Position};
//!
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

pub mod confirmation_dialog;
pub mod context_menu;
pub mod overlay;
pub mod slider_tooltip;
