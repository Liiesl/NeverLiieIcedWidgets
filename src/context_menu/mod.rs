//! Right-click context menu with submenu support.
//!
//! This module provides [`ContextMenu`], a widget that shows a floating
//! menu when the user right-clicks on the wrapped content.
//!
//! # Overview
//!
//! - [`ContextMenu`] — wraps content and shows a menu on right-click
//! - [`Menu`] — builder for menu items
//! - [`MenuItem`] — individual menu entry (action or separator)
//! - [`Item`] — a clickable action item with optional icon and shortcut
//!
//! # Example
//!
//! ```no_run
//! use iced::widget::{container, text};
//! use iced::Element;
//! use neverliie_iced_widgets::context_menu::{ContextMenu, Menu};
//!
//! #[derive(Clone)]
//! enum Message {
//!     Copy,
//!     Paste,
//!     DismissMenu,
//! }
//!
//! fn view() -> Element<'_, Message> {
//!     let content = container(text("Right-click me"))
//!         .center_x(200)
//!         .center_y(200);
//!
//!     let menu = Menu::new()
//!         .item("Copy", Message::Copy)
//!         .item("Paste", Message::Paste);
//!
//!     ContextMenu::new(content, menu)
//!         .on_dismiss(Message::DismissMenu)
//!         .into()
//! }
//! ```
//!
//! # Submenus
//!
//! Items can have nested submenus that appear on hover:
//!
//! ```ignore
//! let menu = Menu::new()
//!     .item("Copy", Message::Copy)
//!     .separator()
//!     .submenu(
//!         "More",
//!         Menu::new()
//!             .item("Option A", Message::A)
//!             .item("Option B", Message::B),
//!     );
//! ```
//!
//! # Icons
//!
//! Items can have an icon — any [`Element`], such as an
//! [`image`](iced::widget::image) / SVG, a glyph ([`text`](iced::widget::text)),
//! or a [`LazyIcon`](crate::lazy_icon::LazyIcon):
//!
//! ```ignore
//! let menu = Menu::new()
//!     .item("Copy", Message::Copy)
//!     .icon(text("⧉").size(14))
//!     .shortcut("Ctrl+C")
//!     .item("Save", Message::Save)
//!     .icon(image(image::Handle::from_path("icons/save.png")));
//! ```
//!
//! [`ContextMenu`]: struct.ContextMenu
//! [`Menu`]: struct.Menu
//! [`MenuItem`]: enum.MenuItem
//! [`Item`]: struct.Item

mod manager;
mod overlay;

pub use manager::ContextMenu;

use iced::{Background, Border, Color, Element, Shadow};

/// A single entry in a [`Menu`].
///
/// Either an actionable item or a visual separator.
pub enum MenuItem<'a, Message, Theme, Renderer> {
    /// A clickable menu item.
    Item(Item<'a, Message, Theme, Renderer>),
    /// A horizontal separator line.
    Separator,
}

/// A clickable menu item with a label, action, optional icon and shortcut.
///
/// Create with [`Menu::item`] or [`Menu::item_disabled`].
pub struct Item<'a, Message, Theme, Renderer> {
    label: &'a str,
    action: Option<Message>,
    icon: Option<Element<'a, Message, Theme, Renderer>>,
    shortcut: Option<&'a str>,
    submenu: Option<Menu<'a, Message, Theme, Renderer>>,
}

impl<'a, Message, Theme, Renderer> Item<'a, Message, Theme, Renderer> {
    /// Creates a new enabled item.
    pub fn new(label: &'a str, action: Message) -> Self {
        Self {
            label,
            action: Some(action),
            icon: None,
            shortcut: None,
            submenu: None,
        }
    }

    /// Creates a new disabled item.
    pub fn disabled(label: &'a str) -> Self {
        Self {
            label,
            action: None,
            icon: None,
            shortcut: None,
            submenu: None,
        }
    }

    /// Sets the icon of this item.
    ///
    /// The icon can be any [`Element`] — an [`image`](crate::lazy_icon) /
    /// [`LazyIcon`](crate::lazy_icon::LazyIcon), an SVG, a glyph
    /// ([`text`](iced::widget::text)), or any other widget.
    #[must_use]
    pub fn icon(
        mut self,
        icon: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Sets the keyboard shortcut text displayed on the right.
    #[must_use]
    pub fn shortcut(mut self, shortcut: &'a str) -> Self {
        self.shortcut = Some(shortcut);
        self
    }

    /// Attaches a submenu to this item.
    #[must_use]
    pub fn submenu(mut self, menu: Menu<'a, Message, Theme, Renderer>) -> Self {
        self.submenu = Some(menu);
        self
    }

    /// Returns `true` if this item is interactive (has an action or a submenu).
    pub fn is_enabled(&self) -> bool {
        self.action.is_some() || self.submenu.is_some()
    }

    /// Returns `true` if this item has a submenu.
    pub fn has_submenu(&self) -> bool {
        self.submenu.is_some()
    }
}

/// A context menu containing a list of [`MenuItem`]s.
///
/// Build with the builder methods, then pass to [`ContextMenu::new`].
///
/// # Example
///
/// ```ignore
/// let menu = Menu::new()
///     .item("Copy", Message::Copy)
///     .shortcut("Ctrl+C")
///     .item("Paste", Message::Paste)
///     .shortcut("Ctrl+V")
///     .separator()
///     .item_disabled("Undo", Message::Undo)
///     .submenu("More", Menu::new().item("A", Message::A));
/// ```
pub struct Menu<'a, Message, Theme, Renderer> {
    pub(crate) items: Vec<MenuItem<'a, Message, Theme, Renderer>>,
}

impl<'a, Message, Theme, Renderer> Menu<'a, Message, Theme, Renderer> {
    /// Creates an empty menu.
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Adds a clickable item with the given label and action message.
    #[must_use]
    pub fn item(mut self, label: &'a str, action: Message) -> Self {
        self.items.push(MenuItem::Item(Item::new(label, action)));
        self
    }

    /// Adds a disabled (greyed-out) item with the given label.
    #[must_use]
    pub fn item_disabled(mut self, label: &'a str) -> Self {
        self.items.push(MenuItem::Item(Item::disabled(label)));
        self
    }

    /// Adds a horizontal separator line.
    #[must_use]
    pub fn separator(mut self) -> Self {
        self.items.push(MenuItem::Separator);
        self
    }

    /// Sets the icon of the last added item.
    ///
    /// The icon can be any [`Element`] — an [`image`](crate::lazy_icon) /
    /// [`LazyIcon`](crate::lazy_icon::LazyIcon), an SVG, a glyph
    /// ([`text`](iced::widget::text)), or any other widget.
    ///
    /// Does nothing if the last entry is a separator or the menu is empty.
    #[must_use]
    pub fn icon(
        mut self,
        icon: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        if let Some(MenuItem::Item(item)) = self.items.last_mut() {
            item.icon = Some(icon.into());
        }
        self
    }

    /// Sets the keyboard shortcut text on the last added item.
    ///
    /// Does nothing if the last entry is a separator or the menu is empty.
    #[must_use]
    pub fn shortcut(mut self, shortcut: &'a str) -> Self {
        if let Some(MenuItem::Item(item)) = self.items.last_mut() {
            item.shortcut = Some(shortcut);
        }
        self
    }

    /// Adds a submenu item with the given label and child menu.
    #[must_use]
    pub fn submenu(mut self, label: &'a str, sub: Menu<'a, Message, Theme, Renderer>) -> Self {
        self.items.push(MenuItem::Item(Item {
            label,
            action: None,
            icon: None,
            shortcut: None,
            submenu: Some(sub),
        }));
        self
    }

    /// Returns the number of visible (non-separator) items.
    pub fn item_count(&self) -> usize {
        self.items
            .iter()
            .filter(|m| matches!(m, MenuItem::Item(_)))
            .count()
    }

    /// Returns the index of the nth visible item in the full items list.
    pub fn visible_index(&self, nth: usize) -> Option<usize> {
        let mut count = 0;
        for (i, item) in self.items.iter().enumerate() {
            if matches!(item, MenuItem::Item(_)) {
                if count == nth {
                    return Some(i);
                }
                count += 1;
            }
        }
        None
    }
}

impl<'a, Message, Theme, Renderer> Default for Menu<'a, Message, Theme, Renderer> {
    fn default() -> Self {
        Self::new()
    }
}

/// Style for a [`ContextMenu`] menu.
#[derive(Debug, Clone)]
pub struct Style {
    /// Menu background.
    pub background: Background,
    /// Menu border.
    pub border: Border,
    /// Menu drop shadow.
    pub shadow: Shadow,
    /// Default item text color.
    pub text_color: Color,
    /// Disabled item text color.
    pub disabled_text_color: Color,
    /// Hovered item background.
    pub selected_background: Background,
    /// Hovered item text color.
    pub selected_text_color: Color,
    /// Separator line color.
    pub separator_color: Color,
    /// Shortcut text color.
    pub shortcut_text_color: Color,
}

/// A styling function for a [`ContextMenu`].
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme) -> Style + 'a>;

/// Theme catalog for context menu styling.
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
        background: palette.background.weak.color.into(),
        border: Border {
            width: 1.0,
            radius: 4.0.into(),
            color: palette.background.strong.color,
        },
        shadow: Shadow::default(),
        text_color: palette.background.weak.text,
        disabled_text_color: palette
            .background
            .weak
            .text
            .scale_alpha(0.4),
        selected_text_color: palette.primary.strong.text,
        selected_background: palette.primary.strong.color.into(),
        separator_color: palette.background.strong.color,
        shortcut_text_color: palette
            .background
            .weak
            .text
            .scale_alpha(0.6),
    }
}

impl Default for Style {
    fn default() -> Self {
        Self {
            background: Background::Color(Color::from_rgb(0.12, 0.12, 0.18)),
            border: Border {
                color: Color::from_rgb(0.35, 0.35, 0.5),
                width: 1.0,
                ..Default::default()
            },
            shadow: Shadow::default(),
            text_color: Color::from_rgb(0.9, 0.9, 0.9),
            disabled_text_color: Color::from_rgb(0.4, 0.4, 0.45),
            selected_background: Background::Color(Color::from_rgb(0.2, 0.25, 0.4)),
            selected_text_color: Color::WHITE,
            separator_color: Color::from_rgb(0.3, 0.3, 0.4),
            shortcut_text_color: Color::from_rgb(0.55, 0.55, 0.65),
        }
    }
}

/// Internal layout constants.
pub(crate) const ITEM_PADDING_X: f32 = 16.0;
pub(crate) const ITEM_PADDING_Y: f32 = 6.0;
pub(crate) const SEPARATOR_HEIGHT: f32 = 9.0;
pub(crate) const SHORTCUT_SPACING: f32 = 24.0;
/// Width reserved for item icons.
pub(crate) const ICON_WIDTH: f32 = 16.0;
/// Spacing between the icon and the item label.
pub(crate) const ICON_SPACING: f32 = 6.0;
