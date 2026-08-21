//! A dropdown widget for selecting a single value from a list of entries.
//!
//! Ported from iced's native [`PickList`] and extended with the entry model
//! of a context menu: items with optional icons, group labels and separators,
//! plus an optional search box that filters the items while open.
//!
//! # Example
//! ```no_run
//! # mod iced { pub mod widget { pub use iced_widget::*; } pub use iced_widget::Renderer; pub use iced_widget::core::*; }
//! # pub type Element<'a, Message> = iced_widget::core::Element<'a, Message, iced_widget::Theme, iced_widget::Renderer>;
//! #
//! use iced::widget::text;
//! use neverliie_iced_widgets::advanced_dropdown::{
//!     advanced_dropdown, Item, MenuItem,
//! };
//!
//! struct State {
//!    favorite: Option<Fruit>,
//! }
//!
//! #[derive(Debug, Clone, Copy, PartialEq, Eq)]
//! enum Fruit {
//!     Apple,
//!     Orange,
//!     Strawberry,
//!     Tomato,
//! }
//!
//! #[derive(Debug, Clone)]
//! enum Message {
//!     FruitSelected(Fruit),
//!     NewItemPressed,
//! }
//!
//! fn view(state: &State) -> Element<'_, Message> {
//!     let entries = [
//!         MenuItem::Label("Fruits"),
//!         MenuItem::Item(
//!             Item::new(Fruit::Apple, "Apple")
//!                 .icon(text("🍎").size(14)),
//!         ),
//!         MenuItem::Item(Item::new(Fruit::Orange, "Orange")),
//!         MenuItem::Separator,
//!         MenuItem::Item(Item::new(Fruit::Strawberry, "Strawberry")),
//!         MenuItem::Item(Item::new(Fruit::Tomato, "Tomato")),
//!     ];
//!
//!     advanced_dropdown(entries, state.favorite, Message::FruitSelected)
//!         .placeholder("Select your favorite fruit...")
//!         .searchable(true)
//!         .footer(
//!             Footer::new("+ Add fruit", Message::NewItemPressed)
//!                 .icon(text("➕").size(14))
//!         )
//!         .into()
//! }
//!
//! fn update(state: &mut State, message: Message) {
//!     match message {
//!         Message::FruitSelected(fruit) => {
//!             state.favorite = Some(fruit);
//!         }
//!         Message::NewItemPressed => {
//!             // Open your "create new item" dialog here.
//!         }
//!     }
//! }
//! ```
pub mod menu;

use iced::advanced::layout;
use iced::advanced::mouse;
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::text::paragraph;
use iced::advanced::text::{self, Text};
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget};
use iced::{
    alignment, border, keyboard, touch, window, Background, Border, Color, Element,
    Event, Length, Padding, Pixels, Point, Rectangle, Size, Theme, Vector,
};
use iced::widget::text_input;

use std::borrow::{Borrow, BorrowMut};
use std::f32;

use menu::Menu;

/// Width reserved for item icons inside the menu.
pub(crate) const ICON_WIDTH: f32 = 16.0;
/// Spacing between the icon and the item label inside the menu.
pub(crate) const ICON_SPACING: f32 = 6.0;
/// Height of a separator row inside the menu.
pub(crate) const SEPARATOR_HEIGHT: f32 = 9.0;

/// A single entry of an [`AdvancedDropdown`] menu.
///
/// Either a selectable [`Item`], a non-selectable group [`Label`], or a
/// visual [`Separator`].
///
/// [`AdvancedDropdown`]: struct.AdvancedDropdown
/// [`Item`]: struct.Item
/// [`Label`]: enum.MenuItem.html#variant.Label
/// [`Separator`]: enum.MenuItem.html#variant.Separator
pub enum MenuItem<'a, T, Message, Theme, Renderer> {
    /// A selectable item with a value, label and optional icon.
    Item(Item<'a, T, Message, Theme, Renderer>),
    /// A non-selectable group label shown as a header row.
    Label(&'a str),
    /// A horizontal separator line.
    Separator,
}

/// A selectable entry of an [`AdvancedDropdown`] with a value, label and
/// optional icon.
///
/// Create with [`Item::new`] or [`Item::with_value`].
pub struct Item<'a, T, Message, Theme, Renderer> {
    value: T,
    label: Option<String>,
    icon: Option<Element<'a, Message, Theme, Renderer>>,
}

impl<'a, T, Message, Theme, Renderer> Item<'a, T, Message, Theme, Renderer>
where
    T: ToString,
{
    /// Creates a new [`Item`] with the given value and label.
    pub fn new(value: T, label: impl Into<String>) -> Self {
        Self {
            value,
            label: Some(label.into()),
            icon: None,
        }
    }

    /// Creates a new [`Item`] whose label is the [`ToString`] rendering of
    /// its value.
    pub fn with_value(value: T) -> Self {
        Self {
            value,
            label: None,
            icon: None,
        }
    }

    /// Sets the icon of this item.
    ///
    /// The icon can be any [`Element`] — an [`image`](iced::widget::image) /
    /// SVG, a glyph ([`text`](iced::widget::text)), or any other widget.
    #[must_use]
    pub fn icon(
        mut self,
        icon: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Returns the value of this item.
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Returns the label of this item, falling back to the [`ToString`]
    /// rendering of its value when no explicit label was provided.
    pub fn label(&self) -> String {
        self.label
            .clone()
            .unwrap_or_else(|| self.value.to_string())
    }
}

/// A clickable footer row pinned at the bottom of the [`AdvancedDropdown`] menu.
///
/// Footers are rendered below the scrollable list, separated by a single
/// divider. Multiple footers are supported and each carries its own message.
pub struct Footer<'a, Message, Theme, Renderer> {
    pub(crate) label: String,
    pub(crate) icon: Option<Element<'a, Message, Theme, Renderer>>,
    pub(crate) on_press: Message,
}

impl<'a, Message, Theme, Renderer> Footer<'a, Message, Theme, Renderer> {
    /// Creates a new footer with the given label and message.
    pub fn new(label: impl Into<String>, on_press: Message) -> Self {
        Self {
            label: label.into(),
            icon: None,
            on_press,
        }
    }

    /// Sets the icon of this footer.
    #[must_use]
    pub fn icon(
        mut self,
        icon: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// Returns the label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the message produced when pressed.
    pub fn on_press(&self) -> &Message
    where
        Message: Clone,
    {
        &self.on_press
    }
}

/// A widget for selecting a single value from a list of [`MenuItem`]s.
///
/// Based on iced's native [`PickList`] and extended with:
///
/// - **Icons**: each item can carry an optional icon, rendered in a fixed
///   column on the left of the menu (and next to the label when selected).
/// - **Labels**: non-selectable group header rows.
/// - **Separators**: horizontal divider rows.
/// - **Search**: when [`AdvancedDropdown::searchable`] is enabled, a search
///   box is shown at the top of the open menu and filters the items.
///
/// # Example
/// ```no_run
/// # mod iced { pub mod widget { pub use iced_widget::*; } pub use iced_widget::Renderer; pub use iced_widget::core::*; }
/// # pub type Element<'a, Message> = iced_widget::core::Element<'a, Message, iced_widget::Theme, iced_widget::Renderer>;
/// #
/// use iced::widget::text;
/// use neverliie_iced_widgets::advanced_dropdown::{
///     advanced_dropdown, Item, MenuItem,
/// };
///
/// struct State {
///    favorite: Option<Fruit>,
/// }
///
/// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// enum Fruit {
///     Apple,
///     Orange,
///     Strawberry,
///     Tomato,
/// }
///
/// #[derive(Debug, Clone)]
/// enum Message {
///     FruitSelected(Fruit),
///     NewItemPressed,
/// }
///
/// fn view(state: &State) -> Element<'_, Message> {
///     let entries = [
///         MenuItem::Label("Fruits"),
///         MenuItem::Item(
///             Item::new(Fruit::Apple, "Apple")
///                 .icon(text("🍎").size(14)),
///         ),
///         MenuItem::Item(Item::new(Fruit::Orange, "Orange")),
///         MenuItem::Separator,
///         MenuItem::Item(Item::new(Fruit::Strawberry, "Strawberry")),
///         MenuItem::Item(Item::new(Fruit::Tomato, "Tomato")),
///     ];
///
///     advanced_dropdown(entries, state.favorite, Message::FruitSelected)
///         .placeholder("Select your favorite fruit...")
///         .searchable(true)
///         .footer(
///             Footer::new("+ Add fruit", Message::NewItemPressed)
///                 .icon(text("➕").size(14)),
///         )
///         .into()
/// }
///
/// fn update(state: &mut State, message: Message) {
///     match message {
///         Message::FruitSelected(fruit) => {
///             state.favorite = Some(fruit);
///         }
///         Message::NewItemPressed => {
///             // Open your "create new item" dialog here.
///         }
///     }
/// }
/// ```
pub struct AdvancedDropdown<
    'a,
    T,
    L,
    V,
    Message,
    Theme = iced::Theme,
    Renderer = iced::Renderer,
> where
    T: ToString + PartialEq + Clone,
    L: BorrowMut<[MenuItem<'a, T, Message, Theme, Renderer>]> + 'a,
    V: Borrow<T> + 'a,
    Theme: Catalog,
    Renderer: text::Renderer,
{
    on_select: Box<dyn Fn(T) -> Message + 'a>,
    on_open: Option<Message>,
    on_close: Option<Message>,
    footers: Vec<Footer<'a, Message, Theme, Renderer>>,
    options: L,
    placeholder: Option<String>,
    selected: Option<V>,
    searchable: bool,
    width: Length,
    padding: Padding,
    search_padding: Padding,
    text_size: Option<Pixels>,
    text_line_height: text::LineHeight,
    text_shaping: text::Shaping,
    font: Option<Renderer::Font>,
    handle: Handle<Renderer::Font>,
    border_radius: Option<border::Radius>,
    menu_border_radius: Option<border::Radius>,
    search_border_radius: Option<border::Radius>,
    class: <Theme as Catalog>::Class<'a>,
    menu_class: <Theme as menu::Catalog>::Class<'a>,
    last_status: Option<Status>,
    menu_height: Length,
    menu_max_height: Option<f32>,
}

impl<'a, T, L, V, Message, Theme, Renderer>
    AdvancedDropdown<'a, T, L, V, Message, Theme, Renderer>
where
    T: ToString + PartialEq + Clone,
    L: BorrowMut<[MenuItem<'a, T, Message, Theme, Renderer>]> + 'a,
    V: Borrow<T> + 'a,
    Message: Clone,
    Theme: Catalog,
    Renderer: text::Renderer,
{
    /// Creates a new [`AdvancedDropdown`] with the given list of entries,
    /// the current selected value, and the message to produce when an option
    /// is selected.
    pub fn new(
        options: L,
        selected: Option<V>,
        on_select: impl Fn(T) -> Message + 'a,
    ) -> Self {
        Self {
            on_select: Box::new(on_select),
            on_open: None,
            on_close: None,
            footers: Vec::new(),
            options,
            placeholder: None,
            selected,
            searchable: false,
            width: Length::Shrink,
            padding: iced::widget::button::DEFAULT_PADDING,
            search_padding: Padding::new(4.0),
            text_size: None,
            text_line_height: text::LineHeight::default(),
            text_shaping: text::Shaping::default(),
            font: None,
            handle: Handle::default(),
            border_radius: None,
            menu_border_radius: None,
            search_border_radius: None,
            class: <Theme as Catalog>::default(),
            menu_class: <Theme as Catalog>::default_menu(),
            last_status: None,
            menu_height: Length::Shrink,
            menu_max_height: None,
        }
    }

    /// Sets the placeholder of the [`AdvancedDropdown`].
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Enables or disables the search box of the [`AdvancedDropdown`].
    ///
    /// When enabled, the open menu shows a search box at the top that
    /// filters the items by their label.
    pub fn searchable(mut self, searchable: bool) -> Self {
        self.searchable = searchable;
        self
    }

    /// Sets the width of the [`AdvancedDropdown`].
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the [`Menu`].
    pub fn menu_height(mut self, menu_height: impl Into<Length>) -> Self {
        self.menu_height = menu_height.into();
        self
    }

    /// Sets the max height of the [`Menu`]. When set, the menu will be
    /// `Shrink` (fit content) but never exceed this height – the list becomes
    /// scrollable instead of growing. This is the preferred way to limit the
    /// dropdown size without forcing a fixed height. The cap applies to the
    /// total overlay height (search input + scrollable list + footers).
    ///
    /// When `menu_max_height` is set, `menu_height` is ignored (the menu
    /// always uses `Length::Shrink` and is capped via layout limits).
    pub fn menu_max_height(mut self, max_height: impl Into<Pixels>) -> Self {
        self.menu_max_height = Some(max_height.into().0);
        self
    }

    /// Sets the [`Padding`] of the [`AdvancedDropdown`].
    pub fn padding<P: Into<Padding>>(mut self, padding: P) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the text size of the [`AdvancedDropdown`].
    pub fn text_size(mut self, size: impl Into<Pixels>) -> Self {
        self.text_size = Some(size.into());
        self
    }

    /// Sets the text [`text::LineHeight`] of the [`AdvancedDropdown`].
    pub fn text_line_height(
        mut self,
        line_height: impl Into<text::LineHeight>,
    ) -> Self {
        self.text_line_height = line_height.into();
        self
    }

    /// Sets the [`text::Shaping`] strategy of the [`AdvancedDropdown`].
    pub fn text_shaping(mut self, shaping: text::Shaping) -> Self {
        self.text_shaping = shaping;
        self
    }

    /// Sets the font of the [`AdvancedDropdown`].
    pub fn font(mut self, font: impl Into<Renderer::Font>) -> Self {
        self.font = Some(font.into());
        self
    }

    /// Sets the [`Handle`] of the [`AdvancedDropdown`].
    pub fn handle(mut self, handle: Handle<Renderer::Font>) -> Self {
        self.handle = handle;
        self
    }

    /// Sets the message that will be produced when the [`AdvancedDropdown`]
    /// is opened.
    pub fn on_open(mut self, on_open: Message) -> Self {
        self.on_open = Some(on_open);
        self
    }

    /// Sets the message that will be produced when the [`AdvancedDropdown`]
    /// is closed.
    pub fn on_close(mut self, on_close: Message) -> Self {
        self.on_close = Some(on_close);
        self
    }

    /// Sets the outer [`Padding`] around the search field inside the menu.
    ///
    /// Uniform padding (same on all sides) is applied to inset the search
    /// box from the menu border so it does not look out of place.
    pub fn search_padding<P: Into<Padding>>(mut self, padding: P) -> Self {
        self.search_padding = padding.into();
        self
    }

    /// Sets the border radius of the closed field.
    pub fn border_radius(mut self, radius: impl Into<border::Radius>) -> Self {
        self.border_radius = Some(radius.into());
        self
    }

    /// Sets the border radius of the dropdown menu.
    pub fn menu_border_radius(
        mut self,
        radius: impl Into<border::Radius>,
    ) -> Self {
        self.menu_border_radius = Some(radius.into());
        self
    }

    /// Sets the border radius of the search field inside the menu.
    pub fn search_border_radius(
        mut self,
        radius: impl Into<border::Radius>,
    ) -> Self {
        self.search_border_radius = Some(radius.into());
        self
    }

    /// Adds a clickable footer row pinned at the bottom of the menu.
    ///
    /// Multiple footers are supported; they are rendered below the scrollable
    /// list with a single divider above the block. Each footer closes the
    /// menu before its message is produced.
    pub fn footer(mut self, footer: Footer<'a, Message, Theme, Renderer>) -> Self {
        self.footers.push(footer);
        self
    }

    /// Adds multiple clickable footer rows.
    pub fn footers(
        mut self,
        footers: impl IntoIterator<Item = Footer<'a, Message, Theme, Renderer>>,
    ) -> Self {
        self.footers.extend(footers);
        self
    }

    /// Sets the style of the [`AdvancedDropdown`].
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        <Theme as Catalog>::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Sets the style of the [`Menu`].
    #[must_use]
    pub fn menu_style(
        mut self,
        style: impl Fn(&Theme) -> menu::Style + 'a,
    ) -> Self
    where
        <Theme as menu::Catalog>::Class<'a>: From<menu::StyleFn<'a, Theme>>,
    {
        self.menu_class = (Box::new(style) as menu::StyleFn<'a, Theme>).into();
        self
    }

    /// Sets the style class of the [`AdvancedDropdown`].
    #[must_use]
    pub fn class(
        mut self,
        class: impl Into<<Theme as Catalog>::Class<'a>>,
    ) -> Self {
        self.class = class.into();
        self
    }

    /// Sets the style class of the [`Menu`].
    #[must_use]
    pub fn menu_class(
        mut self,
        class: impl Into<<Theme as menu::Catalog>::Class<'a>>,
    ) -> Self {
        self.menu_class = class.into();
        self
    }
}

/// Creates a new [`AdvancedDropdown`] with the given list of entries, the
/// current selected value, and the message to produce when an option is
/// selected.
pub fn advanced_dropdown<'a, T, L, V, Message, Theme, Renderer>(
    options: L,
    selected: Option<V>,
    on_select: impl Fn(T) -> Message + 'a,
) -> AdvancedDropdown<'a, T, L, V, Message, Theme, Renderer>
where
    T: ToString + PartialEq + Clone + 'a,
    L: BorrowMut<[MenuItem<'a, T, Message, Theme, Renderer>]> + 'a,
    V: Borrow<T> + 'a,
    Message: Clone,
    Theme: Catalog + menu::Catalog,
    Renderer: text::Renderer,
{
    AdvancedDropdown::new(options, selected, on_select)
}

impl<'a, T, L, V, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for AdvancedDropdown<'a, T, L, V, Message, Theme, Renderer>
where
    T: Clone + ToString + PartialEq + 'a,
    L: BorrowMut<[MenuItem<'a, T, Message, Theme, Renderer>]>,
    V: Borrow<T>,
    Message: Clone + 'a,
    Theme: Catalog + 'a,
    Renderer: text::Renderer + 'a,
    for<'b> <Theme as text_input::Catalog>::Class<'b>:
        From<text_input::StyleFn<'b, Theme>>,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State<Renderer::Paragraph>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::<Renderer::Paragraph>::new())
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: Length::Shrink,
        }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();

        let font = self.font.unwrap_or_else(|| renderer.default_font());
        let text_size =
            self.text_size.unwrap_or_else(|| renderer.default_size());
        let options = self.options.borrow_mut();

        state.options.resize_with(options.len(), Default::default);

        let option_text = Text {
            content: "",
            bounds: Size::new(
                f32::INFINITY,
                self.text_line_height.to_absolute(text_size).into(),
            ),
            size: text_size,
            line_height: self.text_line_height,
            font,
            align_x: text::Alignment::Default,
            align_y: alignment::Vertical::Center,
            shaping: self.text_shaping,
            wrapping: text::Wrapping::default(),
        };

        for (i, entry) in options.iter().enumerate() {
            let label = match entry {
                MenuItem::Item(item) => item.label(),
                MenuItem::Label(text) => text.to_string(),
                MenuItem::Separator => String::new(),
            };

            let _ = state.options[i].update(Text {
                content: &label,
                ..option_text
            });
        }

        if let Some(placeholder) = &self.placeholder {
            let _ = state.placeholder.update(Text {
                content: placeholder,
                ..option_text
            });
        }

        ensure_icon_trees(&mut state.icon_trees, options);

        let selected = self.selected.as_ref().map(Borrow::borrow);
        let selected_index = selected.and_then(|selected| {
            options.iter().position(|entry| {
                matches!(
                    entry,
                    MenuItem::Item(item) if Some(&item.value) == Some(selected)
                )
            })
        });
        let selected_has_icon = selected_index.is_some_and(|index| {
            matches!(
                &options[index],
                MenuItem::Item(item) if item.icon.is_some()
            )
        });

        let max_width = match self.width {
            Length::Shrink => {
                let labels_width =
                    state.options.iter().fold(0.0, |width, paragraph| {
                        f32::max(width, paragraph.min_width())
                    });

                labels_width.max(
                    self.placeholder
                        .as_ref()
                        .map(|_| state.placeholder.min_width())
                        .unwrap_or(0.0),
                )
            }
            _ => 0.0,
        };

        let size = {
            let icon_space = if selected_has_icon {
                ICON_WIDTH + ICON_SPACING
            } else {
                0.0
            };
            let intrinsic = Size::new(
                max_width + icon_space + text_size.0 + self.padding.left,
                f32::from(self.text_line_height.to_absolute(text_size)),
            );

            limits
                .width(self.width)
                .shrink(self.padding)
                .resolve(self.width, Length::Shrink, intrinsic)
                .expand(self.padding)
        };

        let mut children = Vec::new();

        if let Some(index) = selected_index
            && let MenuItem::Item(item) = &mut options[index]
            && let Some(icon) = item.icon.as_mut()
            && let Some(icon_tree) = state.icon_trees[index].as_mut()
        {
            let icon_limits = layout::Limits::new(
                Size::ZERO,
                Size::new(ICON_WIDTH, f32::from(self.text_line_height.to_absolute(text_size))),
            );
            let mut icon_node = icon.as_widget_mut().layout(icon_tree, renderer, &icon_limits);
            let icon_size = icon_node.size();
            let x = self.padding.left + (ICON_WIDTH - icon_size.width) / 2.0;
            let cy = (size.height - icon_size.height) / 2.0;
            icon_node.move_to_mut(Point::new(x, cy));
            children.push(icon_node);
        }

        layout::Node::with_children(size, children)
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if state.is_open {
                    // Event wasn't processed by overlay, so cursor was clicked
                    // either outside its bounds or on the drop-down, either way
                    // we close the overlay.
                    state.is_open = false;

                    if let Some(on_close) = &self.on_close {
                        shell.publish(on_close.clone());
                    }

                    shell.capture_event();
                } else if cursor.is_over(layout.bounds()) {
                    let selected = self.selected.as_ref().map(Borrow::borrow);

                    state.is_open = true;
                    state.menu.search.clear();
                    for h in &mut state.menu.footers_hovered {
                        *h = false;
                    }
                    state.hovered_option = self
                        .options
                        .borrow()
                        .iter()
                        .position(|entry| {
                            matches!(
                                entry,
                                MenuItem::Item(item) if Some(&item.value) == selected
                            )
                        });

                    state.menu.search_focus = self.searchable;

                    if let Some(on_open) = &self.on_open {
                        shell.publish(on_open.clone());
                    }

                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Lines { y, .. },
            }) => {
                if state.keyboard_modifiers.command()
                    && cursor.is_over(layout.bounds())
                    && !state.is_open
                {
                    fn find_next<'a, T: PartialEq>(
                        selected: &'a T,
                        mut options: impl Iterator<Item = &'a T>,
                    ) -> Option<&'a T> {
                        let _ = options.find(|&option| option == selected);

                        options.next()
                    }

                    let options = self.options.borrow();
                    let selected = self.selected.as_ref().map(Borrow::borrow);
                    let mut values =
                        options.iter().filter_map(|entry| match entry {
                            MenuItem::Item(item) => Some(&item.value),
                            MenuItem::Label(_) | MenuItem::Separator => None,
                        });

                    let next_option = if *y < 0.0 {
                        if let Some(selected) = selected {
                            find_next(selected, values)
                        } else {
                            values.next()
                        }
                    } else if *y > 0.0 {
                        if let Some(selected) = selected {
                            find_next(selected, values.rev())
                        } else {
                            values.last()
                        }
                    } else {
                        None
                    };

                    if let Some(next_option) = next_option {
                        shell.publish((self.on_select)(next_option.clone()));
                    }

                    shell.capture_event();
                }
            }
            Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                state.keyboard_modifiers = *modifiers;
            }
            Event::Keyboard(keyboard::Event::KeyPressed { key, .. })
                if state.is_open
                    && matches!(
                        key.as_ref(),
                        keyboard::Key::Named(keyboard::key::Named::Escape)
                    ) =>
            {
                state.is_open = false;

                if let Some(on_close) = &self.on_close {
                    shell.publish(on_close.clone());
                }

                shell.capture_event();
            }
            _ => {}
        };

        let status = {
            let is_hovered = cursor.is_over(layout.bounds());

            if state.is_open {
                Status::Opened { is_hovered }
            } else if is_hovered {
                Status::Hovered
            } else {
                Status::Active
            }
        };

        if let Event::Window(window::Event::RedrawRequested(_now)) = event {
            self.last_status = Some(status);
        } else if self
            .last_status
            .is_some_and(|last_status| last_status != status)
        {
            shell.request_redraw();
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let bounds = layout.bounds();
        let is_mouse_over = cursor.is_over(bounds);

        if is_mouse_over {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let font = self.font.unwrap_or_else(|| renderer.default_font());
        let selected = self.selected.as_ref().map(Borrow::borrow);
        let state = tree.state.downcast_ref::<State<Renderer::Paragraph>>();

        let bounds = layout.bounds();

        let mut widget_style = Catalog::style(
            theme,
            &self.class,
            self.last_status.unwrap_or(Status::Active),
        );

        if let Some(radius) = self.border_radius {
            widget_style.border.radius = radius;
        }

        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: widget_style.border,
                ..renderer::Quad::default()
            },
            widget_style.background,
        );

        let handle = match &self.handle {
            Handle::Arrow { size } => Some((
                Renderer::ICON_FONT,
                Renderer::ARROW_DOWN_ICON,
                *size,
                text::LineHeight::default(),
                text::Shaping::Basic,
            )),
            Handle::Static(Icon {
                font,
                code_point,
                size,
                line_height,
                shaping,
            }) => Some((*font, *code_point, *size, *line_height, *shaping)),
            Handle::Dynamic { open, closed } => {
                if state.is_open {
                    Some((
                        open.font,
                        open.code_point,
                        open.size,
                        open.line_height,
                        open.shaping,
                    ))
                } else {
                    Some((
                        closed.font,
                        closed.code_point,
                        closed.size,
                        closed.line_height,
                        closed.shaping,
                    ))
                }
            }
            Handle::None => None,
        };

        if let Some((font, code_point, size, line_height, shaping)) = handle {
            let size = size.unwrap_or_else(|| renderer.default_size());

            renderer.fill_text(
                Text {
                    content: code_point.to_string(),
                    size,
                    line_height,
                    font,
                    bounds: Size::new(
                        bounds.width,
                        f32::from(line_height.to_absolute(size)),
                    ),
                    align_x: text::Alignment::Right,
                    align_y: alignment::Vertical::Center,
                    shaping,
                    wrapping: text::Wrapping::default(),
                },
                Point::new(
                    bounds.x + bounds.width - self.padding.right,
                    bounds.center_y(),
                ),
                widget_style.handle_color,
                *viewport,
            );
        }

        let selected_index = selected.and_then(|selected| {
            self.options.borrow().iter().position(|entry| {
                matches!(
                    entry,
                    MenuItem::Item(item) if Some(&item.value) == Some(selected)
                )
            })
        });
        let selected_has_icon = selected_index.is_some_and(|index| {
            matches!(
                &self.options.borrow()[index],
                MenuItem::Item(item) if item.icon.is_some()
            )
        });

        let mut children = layout.children();

        if let Some(index) = selected_index
            && let MenuItem::Item(item) = &self.options.borrow()[index]
            && let Some(icon) = item.icon.as_ref()
            && let Some(icon_layout) = children.next()
            && let Some(icon_tree) = state.icon_trees[index].as_ref()
        {
            icon.as_widget().draw(
                icon_tree,
                renderer,
                theme,
                style,
                icon_layout,
                cursor,
                viewport,
            );
        }

        let label = selected.map(ToString::to_string);

        if let Some(label) = label.or_else(|| self.placeholder.clone()) {
            let text_size =
                self.text_size.unwrap_or_else(|| renderer.default_size());
            let icon_space = if selected_has_icon {
                ICON_WIDTH + ICON_SPACING
            } else {
                0.0
            };

            renderer.fill_text(
                Text {
                    content: label,
                    size: text_size,
                    line_height: self.text_line_height,
                    font,
                    bounds: Size::new(
                        bounds.width - self.padding.x(),
                        f32::from(self.text_line_height.to_absolute(text_size)),
                    ),
                    align_x: text::Alignment::Default,
                    align_y: alignment::Vertical::Center,
                    shaping: self.text_shaping,
                    wrapping: text::Wrapping::default(),
                },
                Point::new(
                    bounds.x + self.padding.left + icon_space,
                    bounds.center_y(),
                ),
                if selected.is_some() {
                    widget_style.text_color
                } else {
                    widget_style.placeholder_color
                },
                *viewport,
            );
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let state = tree.state.downcast_mut::<State<Renderer::Paragraph>>();
        let font = self.font.unwrap_or_else(|| renderer.default_font());

        if state.is_open {
            let bounds = layout.bounds();

            let on_select = &self.on_select;
            // Move footers out but keep `self.footers` len for subsequent overlay calls
            // within the same widget instance (avoid 0 len on next layout). We drain
            // icons via `take()` so labels remain for next call.
            let mut footers = Vec::with_capacity(self.footers.len());
            for f in &mut self.footers {
                footers.push(Footer {
                    label: f.label.clone(),
                    icon: f.icon.take(),
                    on_press: f.on_press.clone(),
                });
            }
            let mut menu = Menu::new(
                &mut state.menu,
                self.options.borrow_mut(),
                &mut state.hovered_option,
                &mut state.icon_trees,
                |option| {
                    state.is_open = false;

                    (on_select)(option)
                },
                None,
                footers,
                &self.menu_class,
            )
            .width(bounds.width)
            .padding(self.padding)
            .search_padding(self.search_padding)
            .font(font)
            .text_shaping(self.text_shaping)
            .searchable(self.searchable);

            if let Some(radius) = self.menu_border_radius {
                menu = menu.menu_border_radius(radius);
            }
            if let Some(radius) = self.search_border_radius {
                menu = menu.search_border_radius(radius);
            }

            if let Some(text_size) = self.text_size {
                menu = menu.text_size(text_size);
            }

            Some(if let Some(max) = self.menu_max_height {
                menu.overlay_with_max(
                    layout.position() + translation,
                    *viewport,
                    bounds.height,
                    self.menu_height,
                    Some(max),
                )
            } else {
                menu.overlay(
                    layout.position() + translation,
                    *viewport,
                    bounds.height,
                    self.menu_height,
                )
            })
        } else {
            None
        }
    }
}

impl<'a, T, L, V, Message, Theme, Renderer>
    From<AdvancedDropdown<'a, T, L, V, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    T: Clone + ToString + PartialEq + 'a,
    L: BorrowMut<[MenuItem<'a, T, Message, Theme, Renderer>]> + 'a,
    V: Borrow<T> + 'a,
    Message: Clone + 'a,
    Theme: Catalog + 'a,
    Renderer: text::Renderer + 'a,
    for<'b> <Theme as text_input::Catalog>::Class<'b>:
        From<text_input::StyleFn<'b, Theme>>,
{
    fn from(
        dropdown: AdvancedDropdown<'a, T, L, V, Message, Theme, Renderer>,
    ) -> Self {
        Self::new(dropdown)
    }
}

/// Reconciles the icon [`Tree`]s with the entries of the dropdown.
///
/// Runs every frame. Existing trees are diffed against the current icon
/// widget (cheap tag comparison); missing trees are created.
fn ensure_icon_trees<'a, T, Message, Theme, Renderer>(
    icon_trees: &mut Vec<Option<Tree>>,
    options: &mut [MenuItem<'a, T, Message, Theme, Renderer>],
) where
    Renderer: renderer::Renderer,
{
    if icon_trees.len() != options.len() {
        icon_trees.resize_with(options.len(), || None);
    }

    for (i, entry) in options.iter_mut().enumerate() {
        match entry {
            MenuItem::Item(item) => {
                if let Some(icon) = item.icon.as_mut() {
                    let tree = &mut icon_trees[i];
                    let widget = icon.as_widget();

                    match tree {
                        Some(tree) => tree.diff(widget),
                        None => *tree = Some(Tree::new(widget)),
                    }
                } else {
                    icon_trees[i] = None;
                }
            }
            MenuItem::Label(_) | MenuItem::Separator => {
                icon_trees[i] = None;
            }
        }
    }
}

#[derive(Debug)]
struct State<P: text::Paragraph> {
    menu: menu::State,
    keyboard_modifiers: keyboard::Modifiers,
    is_open: bool,
    hovered_option: Option<usize>,
    options: Vec<paragraph::Plain<P>>,
    placeholder: paragraph::Plain<P>,
    icon_trees: Vec<Option<Tree>>,
}

impl<P: text::Paragraph> State<P> {
    /// Creates a new [`State`] for an [`AdvancedDropdown`].
    fn new() -> Self {
        Self {
            menu: menu::State::default(),
            keyboard_modifiers: keyboard::Modifiers::default(),
            is_open: bool::default(),
            hovered_option: Option::default(),
            options: Vec::new(),
            placeholder: paragraph::Plain::default(),
            icon_trees: Vec::new(),
        }
    }
}

impl<P: text::Paragraph> Default for State<P> {
    fn default() -> Self {
        Self::new()
    }
}

/// The handle to the right side of the [`AdvancedDropdown`].
#[derive(Debug, Clone, PartialEq)]
pub enum Handle<Font> {
    /// Displays an arrow icon (▼).
    ///
    /// This is the default.
    Arrow {
        /// Font size of the content.
        size: Option<Pixels>,
    },
    /// A custom static handle.
    Static(Icon<Font>),
    /// A custom dynamic handle.
    Dynamic {
        /// The [`Icon`] used when [`AdvancedDropdown`] is closed.
        closed: Icon<Font>,
        /// The [`Icon`] used when [`AdvancedDropdown`] is open.
        open: Icon<Font>,
    },
    /// No handle will be shown.
    None,
}

impl<Font> Default for Handle<Font> {
    fn default() -> Self {
        Self::Arrow { size: None }
    }
}

/// The icon of a [`Handle`].
#[derive(Debug, Clone, PartialEq)]
pub struct Icon<Font> {
    /// Font that will be used to display the `code_point`,
    pub font: Font,
    /// The unicode code point that will be used as the icon.
    pub code_point: char,
    /// Font size of the content.
    pub size: Option<Pixels>,
    /// Line height of the content.
    pub line_height: text::LineHeight,
    /// The shaping strategy of the icon.
    pub shaping: text::Shaping,
}

/// The possible status of an [`AdvancedDropdown`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The [`AdvancedDropdown`] can be interacted with.
    Active,
    /// The [`AdvancedDropdown`] is being hovered.
    Hovered,
    /// The [`AdvancedDropdown`] is open.
    Opened {
        /// Whether the [`AdvancedDropdown`] is hovered, while open.
        is_hovered: bool,
    },
}

/// The appearance of an advanced dropdown.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The text [`Color`] of the advanced dropdown.
    pub text_color: Color,
    /// The placeholder [`Color`] of the advanced dropdown.
    pub placeholder_color: Color,
    /// The handle [`Color`] of the advanced dropdown.
    pub handle_color: Color,
    /// The [`Background`] of the advanced dropdown.
    pub background: Background,
    /// The [`Border`] of the advanced dropdown.
    pub border: Border,
}

/// The theme catalog of an [`AdvancedDropdown`].
pub trait Catalog: menu::Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> <Self as Catalog>::Class<'a>;

    /// The default class for the menu of the [`AdvancedDropdown`].
    fn default_menu<'a>() -> <Self as menu::Catalog>::Class<'a> {
        <Self as menu::Catalog>::default()
    }

    /// The [`Style`] of a class with the given status.
    fn style(
        &self,
        class: &<Self as Catalog>::Class<'_>,
        status: Status,
    ) -> Style;
}

/// A styling function for an [`AdvancedDropdown`].
///
/// This is just a boxed closure: `Fn(&Theme, Status) -> Style`.
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme, Status) -> Style + 'a>;

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> StyleFn<'a, Self> {
        Box::new(default)
    }

    fn style(&self, class: &StyleFn<'_, Self>, status: Status) -> Style {
        class(self, status)
    }
}

/// The default style of the field of an [`AdvancedDropdown`].
pub fn default(theme: &Theme, status: Status) -> Style {
    let palette = theme.extended_palette();

    let active = Style {
        text_color: palette.background.weak.text,
        background: palette.background.weak.color.into(),
        placeholder_color: palette.secondary.base.color,
        handle_color: palette.background.weak.text,
        border: Border {
            radius: 2.0.into(),
            width: 1.0,
            color: palette.background.strong.color,
        },
    };

    match status {
        Status::Active => active,
        Status::Hovered | Status::Opened { .. } => Style {
            border: Border {
                color: palette.primary.strong.color,
                ..active.border
            },
            ..active
        },
    }
}