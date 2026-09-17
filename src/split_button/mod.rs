//! A split button with a dropdown menu.
//!
//! A [`SplitButton`] shows the currently selected option as a main action
//! area plus a narrow arrow zone (separated by a divider) that opens a
//! dropdown menu.
//!
//! Options are [`Item`]s with a value, a label and an optional icon — the
//! same model as [`AdvancedDropdown`](crate::advanced_dropdown). The selected
//! option's icon and label are shown on the main area; the menu renders every
//! option with its icon in a fixed column. [`MenuItem::Separator`] rows draw
//! dividers inside the menu.
//!
//! Selecting an entry in the menu only changes the selection — it does not
//! execute the main action. Clicking the main area executes the current
//! selection via `on_press`.
//!
//! # Example
//! ```no_run
//! use iced::widget::text;
//! use iced::Element;
//! use neverliie_iced_widgets::split_button::{Item, MenuItem, split_button};
//!
//! struct State {
//!    action: Option<Action>,
//! }
//!
//! #[derive(Debug, Clone, Copy, PartialEq, Eq)]
//! enum Action {
//!     Save,
//!     SaveAs,
//!     Export,
//! }
//!
//! #[derive(Debug, Clone)]
//! enum Message {
//!     ActionSelected(Action),
//!     ActionPressed(Action),
//! }
//!
//! fn view(state: &State) -> Element<'_, Message> {
//!     let options = [
//!         MenuItem::Item(
//!             Item::new(Action::Save, "Save").icon(text("💾").size(14)),
//!         ),
//!         MenuItem::Item(
//!             Item::new(Action::SaveAs, "Save As").icon(text("📝").size(14)),
//!         ),
//!         MenuItem::Separator,
//!         MenuItem::Item(
//!             Item::new(Action::Export, "Export").icon(text("📤").size(14)),
//!         ),
//!     ];
//!
//!     split_button(options, state.action, Message::ActionSelected)
//!         .placeholder("Choose an action...")
//!         .on_press(Message::ActionPressed)
//!         .into()
//! }
//!
//! fn update(state: &mut State, message: Message) {
//!     match message {
//!         Message::ActionSelected(action) => {
//!             // Menu pick: only changes what the main button shows/does.
//!             state.action = Some(action);
//!         }
//!         Message::ActionPressed(action) => {
//!             // Main-area click: executes the current selection.
//!             state.action = Some(action);
//!             // ... perform the action here ...
//!         }
//!     }
//! }
//!
//! impl std::fmt::Display for Action {
//!     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//!         f.write_str(match self {
//!             Self::Save => "Save",
//!             Self::SaveAs => "Save As",
//!             Self::Export => "Export",
//!         })
//!     }
//! }
//! ```

use iced::advanced::layout;
use iced::advanced::mouse;
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::text::paragraph;
use iced::advanced::text::{self, Text};
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget};
use iced::widget::text_input;
use iced::{
    alignment, border, keyboard, touch, window, Background, Border, Color,
    Element, Event, Length, Padding, Pixels, Point, Rectangle, Shadow, Size,
    Theme, Vector,
};

use std::borrow::{Borrow, BorrowMut};
use std::f32;

use crate::advanced_dropdown::menu::{self, Menu};
use crate::advanced_dropdown::Footer;

/// A selectable option of a [`SplitButton`].
///
/// Re-exported from [`advanced_dropdown`](crate::advanced_dropdown): an
/// option carries a value, a label and an optional icon (any [`Element`],
/// e.g. an image, an SVG or a glyph).
pub use crate::advanced_dropdown::{Item, MenuItem};

/// Width reserved for icons on the button face and inside the menu.
pub(crate) const ICON_WIDTH: f32 = 16.0;
/// Spacing between an icon and its label.
pub(crate) const ICON_SPACING: f32 = 6.0;
/// Inset of the face divider from the top/bottom button edges.
const DIVIDER_INSET: f32 = 6.0;

/// A split button: main action area + arrow zone opening a dropdown menu.
///
/// The menu renders [`Item`] icons in a fixed column, supports separators,
/// keyboard navigation and flips above the button when there is no room
/// below (same menu as [`AdvancedDropdown`](crate::advanced_dropdown)).
///
/// Main-area clicks publish `on_press(selected)`; menu picks publish
/// `on_select(value)` and only change the selection.
pub struct SplitButton<
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
    on_press: Option<Box<dyn Fn(T) -> Message + 'a>>,
    on_open: Option<Message>,
    on_close: Option<Message>,
    options: L,
    placeholder: Option<String>,
    selected: Option<V>,
    width: Length,
    padding: Padding,
    text_size: Option<Pixels>,
    text_line_height: text::LineHeight,
    text_shaping: text::Shaping,
    font: Option<Renderer::Font>,
    handle: Handle<Renderer::Font>,
    border_radius: Option<border::Radius>,
    menu_border_radius: Option<border::Radius>,
    class: <Theme as Catalog>::Class<'a>,
    menu_class: <Theme as menu::Catalog>::Class<'a>,
    last_status: Option<Status>,
    menu_height: Length,
    menu_max_height: Option<f32>,
}

impl<'a, T, L, V, Message, Theme, Renderer>
    SplitButton<'a, T, L, V, Message, Theme, Renderer>
where
    T: ToString + PartialEq + Clone,
    L: BorrowMut<[MenuItem<'a, T, Message, Theme, Renderer>]> + 'a,
    V: Borrow<T> + 'a,
    Message: Clone,
    Theme: Catalog,
    Renderer: text::Renderer,
{
    /// Creates a new [`SplitButton`] with the given options, the current
    /// selected value, and the message produced when a menu option is
    /// selected (selection only — no action is executed).
    pub fn new(
        options: L,
        selected: Option<V>,
        on_select: impl Fn(T) -> Message + 'a,
    ) -> Self {
        Self {
            on_select: Box::new(on_select),
            on_press: None,
            on_open: None,
            on_close: None,
            options,
            placeholder: None,
            selected,
            width: Length::Shrink,
            padding: iced::widget::button::DEFAULT_PADDING,
            text_size: None,
            text_line_height: text::LineHeight::default(),
            text_shaping: text::Shaping::default(),
            font: None,
            handle: Handle::default(),
            border_radius: None,
            menu_border_radius: None,
            class: <Theme as Catalog>::default(),
            menu_class: <Theme as Catalog>::default_menu(),
            last_status: None,
            menu_height: Length::Shrink,
            menu_max_height: None,
        }
    }

    /// Sets the message produced when the main action area is clicked.
    ///
    /// Called with the current selected value. When no value is selected,
    /// or when this is unset, the main area is inert (a main click opens
    /// the menu instead, so a selection can still be made).
    pub fn on_press(mut self, on_press: impl Fn(T) -> Message + 'a) -> Self {
        self.on_press = Some(Box::new(on_press));
        self
    }

    /// Sets the placeholder shown when nothing is selected.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Sets the width of the [`SplitButton`].
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the dropdown menu.
    pub fn menu_height(mut self, menu_height: impl Into<Length>) -> Self {
        self.menu_height = menu_height.into();
        self
    }

    /// Sets the max height of the dropdown menu. When set, the menu shrinks
    /// to content but never exceeds this height — the list scrolls instead.
    /// Takes precedence over [`menu_height`](Self::menu_height).
    pub fn menu_max_height(mut self, max_height: impl Into<Pixels>) -> Self {
        self.menu_max_height = Some(max_height.into().0);
        self
    }

    /// Sets the [`Padding`] of the [`SplitButton`].
    pub fn padding<P: Into<Padding>>(mut self, padding: P) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the text size of the [`SplitButton`].
    pub fn text_size(mut self, size: impl Into<Pixels>) -> Self {
        self.text_size = Some(size.into());
        self
    }

    /// Sets the text [`text::LineHeight`] of the [`SplitButton`].
    pub fn text_line_height(
        mut self,
        line_height: impl Into<text::LineHeight>,
    ) -> Self {
        self.text_line_height = line_height.into();
        self
    }

    /// Sets the [`text::Shaping`] strategy of the [`SplitButton`].
    pub fn text_shaping(mut self, shaping: text::Shaping) -> Self {
        self.text_shaping = shaping;
        self
    }

    /// Sets the font of the [`SplitButton`].
    pub fn font(mut self, font: impl Into<Renderer::Font>) -> Self {
        self.font = Some(font.into());
        self
    }

    /// Sets the [`Handle`] shown in the arrow zone.
    pub fn handle(mut self, handle: Handle<Renderer::Font>) -> Self {
        self.handle = handle;
        self
    }

    /// Sets the message produced when the dropdown is opened.
    pub fn on_open(mut self, on_open: Message) -> Self {
        self.on_open = Some(on_open);
        self
    }

    /// Sets the message produced when the dropdown is closed.
    pub fn on_close(mut self, on_close: Message) -> Self {
        self.on_close = Some(on_close);
        self
    }

    /// Sets the border radius of the button face.
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

    /// Sets the style of the [`SplitButton`].
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        <Theme as Catalog>::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Sets the style of the dropdown menu.
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

    /// Sets the style class of the [`SplitButton`].
    #[must_use]
    pub fn class(
        mut self,
        class: impl Into<<Theme as Catalog>::Class<'a>>,
    ) -> Self {
        self.class = class.into();
        self
    }

    /// Sets the style class of the dropdown menu.
    #[must_use]
    pub fn menu_class(
        mut self,
        class: impl Into<<Theme as menu::Catalog>::Class<'a>>,
    ) -> Self {
        self.menu_class = class.into();
        self
    }

    /// Width of the arrow zone (chevron cell) in logical pixels.
    ///
    /// Covers the handle glyph plus padding so the click target matches the
    /// reserved visual space.
    fn arrow_zone_width(&self, renderer: &Renderer) -> f32 {
        let text_size = self.text_size.unwrap_or_else(|| renderer.default_size());
        text_size.0 + self.padding.x()
    }

    /// Splits `bounds` into `(main, arrow)` zones.
    fn zones(
        &self,
        bounds: Rectangle,
        renderer: &Renderer,
    ) -> (Rectangle, Rectangle) {
        let arrow_w = self.arrow_zone_width(renderer).min(bounds.width);

        (
            Rectangle {
                width: bounds.width - arrow_w,
                ..bounds
            },
            Rectangle {
                x: bounds.x + bounds.width - arrow_w,
                width: arrow_w,
                ..bounds
            },
        )
    }

    /// Opens the dropdown menu.
    fn open(
        state: &mut State<Renderer::Paragraph>,
        options: &[MenuItem<'a, T, Message, Theme, Renderer>],
        selected: Option<&T>,
        on_open: &Option<Message>,
        shell: &mut Shell<'_, Message>,
    ) {
        state.is_open = true;
        state.hovered_option = options.iter().position(|entry| {
            matches!(
                entry,
                MenuItem::Item(item) if Some(item.value()) == selected
            )
        });

        if let Some(on_open) = on_open {
            shell.publish(on_open.clone());
        }

        shell.capture_event();
    }

    /// Closes the dropdown menu.
    fn close(
        state: &mut State<Renderer::Paragraph>,
        on_close: &Option<Message>,
        shell: &mut Shell<'_, Message>,
    ) {
        state.is_open = false;

        if let Some(on_close) = on_close {
            shell.publish(on_close.clone());
        }

        shell.capture_event();
    }
}

/// Creates a new [`SplitButton`] with the given options, the current selected
/// value, and the message produced when a menu option is selected.
pub fn split_button<'a, T, L, V, Message, Theme, Renderer>(
    options: L,
    selected: Option<V>,
    on_select: impl Fn(T) -> Message + 'a,
) -> SplitButton<'a, T, L, V, Message, Theme, Renderer>
where
    T: ToString + PartialEq + Clone + 'a,
    L: BorrowMut<[MenuItem<'a, T, Message, Theme, Renderer>]> + 'a,
    V: Borrow<T> + 'a,
    Message: Clone,
    Theme: Catalog + menu::Catalog,
    Renderer: text::Renderer,
{
    SplitButton::new(options, selected, on_select)
}

impl<'a, T, L, V, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for SplitButton<'a, T, L, V, Message, Theme, Renderer>
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
                    MenuItem::Item(item) if Some(item.value()) == Some(selected)
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
        renderer: &Renderer,
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
                    Self::close(state, &self.on_close, shell);
                } else if cursor.is_over(layout.bounds()) {
                    let bounds = layout.bounds();
                    let (_main, arrow) = self.zones(bounds, renderer);
                    let selected = self.selected.as_ref().map(Borrow::borrow);

                    if cursor.is_over(arrow) {
                        Self::open(
                            state,
                            self.options.borrow(),
                            selected,
                            &self.on_open,
                            shell,
                        );
                    } else if let (Some(on_press), Some(selected)) =
                        (self.on_press.as_ref(), selected)
                    {
                        shell.publish(on_press(selected.clone()));
                        shell.capture_event();
                    } else {
                        // No executable selection: fall back to opening the
                        // menu so a choice can still be made.
                        Self::open(
                            state,
                            self.options.borrow(),
                            selected,
                            &self.on_open,
                            shell,
                        );
                    }
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
                            MenuItem::Item(item) => Some(item.value()),
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
                Self::close(state, &self.on_close, shell);
            }
            _ => {}
        };

        let status = {
            let is_hovered = cursor.is_over(layout.bounds());

            if self.options.borrow().is_empty() {
                Status::Disabled
            } else if state.is_open {
                Status::Opened { is_hovered }
            } else if is_hovered {
                Status::Hovered
            } else {
                Status::Active
            }
        };

        // Track the hovered half: moving between main and arrow zones keeps
        // `Status::Hovered`, so without this no redraw would be requested and
        // the per-zone highlight would go stale.
        let bounds = layout.bounds();
        let (main, arrow) = self.zones(bounds, renderer);
        let hovered_zone = if cursor.is_over(arrow) {
            Some(Zone::Arrow)
        } else if cursor.is_over(main) {
            Some(Zone::Main)
        } else {
            None
        };

        if state.hovered_zone != hovered_zone {
            state.hovered_zone = hovered_zone;
            shell.request_redraw();
        }

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

        let status = self.last_status.unwrap_or(Status::Active);
        let mut widget_style = Catalog::style(theme, &self.class, status);

        if let Some(radius) = self.border_radius {
            widget_style.border.radius = radius;
        }

        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: widget_style.border,
                shadow: widget_style.shadow,
                ..renderer::Quad::default()
            },
            widget_style.background,
        );

        let (main, arrow) = self.zones(bounds, renderer);

        // Separate hover highlight per zone: the main area lightens subtly
        // while the arrow zone — a control of its own — highlights stronger.
        // Uses the tracked zone (see `update`): the live cursor alone would
        // go stale because crossing zones does not change `Status`.
        if status != Status::Disabled {
            let hovered = match state.hovered_zone {
                Some(Zone::Arrow) => Some((arrow, 0.25)),
                Some(Zone::Main) => Some((main, 0.10)),
                None => None,
            };

            if let Some((zone, alpha)) = hovered {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle {
                            x: zone.x + widget_style.border.width,
                            y: bounds.y + widget_style.border.width,
                            width: (zone.width - widget_style.border.width * 2.0)
                                .max(0.0),
                            height: (bounds.height
                                - widget_style.border.width * 2.0)
                                .max(0.0),
                        },
                        border: Border {
                            radius: widget_style.border.radius,
                            width: 0.0,
                            color: Color::TRANSPARENT,
                        },
                        ..renderer::Quad::default()
                    },
                    widget_style.border.color.scale_alpha(alpha),
                );
            }
        }

        // Divider between the main action and the arrow zone. Short and dim:
        // a crack across the full height looks broken.
        if bounds.width > 0.0 {
            let divider_top = bounds.y + DIVIDER_INSET;
            let divider_bottom = bounds.y + bounds.height - DIVIDER_INSET;

            if divider_bottom > divider_top {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle::new(
                            Point::new(arrow.x, divider_top),
                            Size::new(1.0, divider_bottom - divider_top),
                        ),
                        border: Border {
                            radius: 0.0.into(),
                            width: 0.0,
                            color: Color::TRANSPARENT,
                        },
                        ..renderer::Quad::default()
                    },
                    widget_style.border.color.scale_alpha(0.35),
                );
            }
        }

        let handle = match &self.handle {
            Handle::Arrow { size } => {
                // Matching chevron pair from the icon font: down when closed,
                // up while the menu is open.
                let code_point = if state.is_open {
                    Renderer::SCROLL_UP_ICON
                } else {
                    Renderer::SCROLL_DOWN_ICON
                };
                Some((
                    Renderer::ICON_FONT,
                    code_point,
                    *size,
                    text::LineHeight::default(),
                    text::Shaping::Basic,
                ))
            }
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
                    MenuItem::Item(item) if Some(item.value()) == Some(selected)
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
            let footers: Vec<Footer<'_, Message, Theme, Renderer>> = Vec::new();

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
            .font(font)
            .text_shaping(self.text_shaping)
            .searchable(false);

            if let Some(radius) = self.menu_border_radius {
                menu = menu.menu_border_radius(radius);
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
    From<SplitButton<'a, T, L, V, Message, Theme, Renderer>>
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
    fn from(split: SplitButton<'a, T, L, V, Message, Theme, Renderer>) -> Self {
        Self::new(split)
    }
}

/// Reconciles the icon [`Tree`]s with the option entries.
///
/// Runs every frame. Existing trees are diffed against the current icon
/// widget (cheap tag comparison); missing trees are created.
fn ensure_icon_trees<'a, T, Message, Theme, Renderer>(
    icon_trees: &mut Vec<Option<Tree>>,
    options: &mut [MenuItem<'a, T, Message, Theme, Renderer>],
) where
    Renderer: renderer::Renderer + text::Renderer,
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
    hovered_zone: Option<Zone>,
    options: Vec<paragraph::Plain<P>>,
    placeholder: paragraph::Plain<P>,
    icon_trees: Vec<Option<Tree>>,
}

impl<P: text::Paragraph> State<P> {
    /// Creates a new [`State`] for a [`SplitButton`].
    fn new() -> Self {
        Self {
            menu: menu::State::default(),
            keyboard_modifiers: keyboard::Modifiers::default(),
            is_open: bool::default(),
            hovered_option: Option::default(),
            hovered_zone: Option::default(),
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

/// Which half of the split button the cursor is over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Zone {
    /// The main action area (left).
    Main,
    /// The arrow zone opening the menu (right).
    Arrow,
}

/// The handle shown in the arrow zone of a [`SplitButton`].
#[derive(Debug, Clone, PartialEq)]
pub enum Handle<Font> {
    /// Displays a chevron that points down while the menu is closed and up
    /// while it is open.
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
        /// The [`Icon`] used when the dropdown is closed.
        closed: Icon<Font>,
        /// The [`Icon`] used when the dropdown is open.
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

/// The possible status of a [`SplitButton`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The [`SplitButton`] can be interacted with.
    Active,
    /// The [`SplitButton`] is being hovered.
    Hovered,
    /// The dropdown is open.
    Opened {
        /// Whether the button is hovered, while open.
        is_hovered: bool,
    },
    /// There are no options to choose from.
    Disabled,
}

/// The appearance of a split button.
///
/// Same idea as [`button::Style`](iced::widget::button::Style), plus
/// [`placeholder_color`](Style::placeholder_color) and
/// [`handle_color`](Style::handle_color) for the placeholder label and the
/// chevron.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The text [`Color`] of the split button.
    pub text_color: Color,
    /// The placeholder [`Color`] of the split button.
    pub placeholder_color: Color,
    /// The handle [`Color`] of the split button.
    pub handle_color: Color,
    /// The [`Background`] of the split button.
    pub background: Background,
    /// The [`Border`] of the split button. Only the radius is drawn; the
    /// color doubles as the divider/zone-highlight source.
    pub border: Border,
    /// The [`Shadow`] of the split button.
    pub shadow: Shadow,
}

/// The theme catalog of a [`SplitButton`].
pub trait Catalog: menu::Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> <Self as Catalog>::Class<'a>;

    /// The default class for the menu of the [`SplitButton`].
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

/// A styling function for a [`SplitButton`].
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

/// The default style of a [`SplitButton`].
///
/// Same as [`primary`]: a filled button look mirroring
/// [`primary`](iced::widget::button::primary).
pub fn default(theme: &Theme, status: Status) -> Style {
    primary(theme, status)
}

/// A primary split button; denoting the main action.
///
/// Mirrors [`primary`](iced::widget::button::primary): accent background,
/// idle uses the base pair, hover/open uses the strong background, disabled
/// is dimmed.
pub fn primary(theme: &Theme, status: Status) -> Style {
    let palette = theme.extended_palette();

    styled(
        palette.primary.base.color,
        palette.primary.base.text,
        palette.primary.strong.color,
        status,
    )
}

/// A secondary split button; denoting a complementary action.
///
/// Mirrors [`secondary`](iced::widget::button::secondary).
pub fn secondary(theme: &Theme, status: Status) -> Style {
    let palette = theme.extended_palette();

    styled(
        palette.secondary.base.color,
        palette.secondary.base.text,
        palette.secondary.strong.color,
        status,
    )
}

/// A success split button; denoting a good outcome.
///
/// Mirrors [`success`](iced::widget::button::success).
pub fn success(theme: &Theme, status: Status) -> Style {
    let palette = theme.extended_palette();

    styled(
        palette.success.base.color,
        palette.success.base.text,
        palette.success.strong.color,
        status,
    )
}

/// A warning split button; denoting a risky action.
///
/// Mirrors [`warning`](iced::widget::button::warning).
pub fn warning(theme: &Theme, status: Status) -> Style {
    let palette = theme.extended_palette();

    styled(
        palette.warning.base.color,
        palette.warning.base.text,
        palette.warning.strong.color,
        status,
    )
}

/// A danger split button; denoting a destructive action.
///
/// Mirrors [`danger`](iced::widget::button::danger).
pub fn danger(theme: &Theme, status: Status) -> Style {
    let palette = theme.extended_palette();

    styled(
        palette.danger.base.color,
        palette.danger.base.text,
        palette.danger.strong.color,
        status,
    )
}

fn styled(base: Color, base_text: Color, strong: Color, status: Status) -> Style {
    let active = Style {
        text_color: base_text,
        background: base.into(),
        placeholder_color: base_text.scale_alpha(0.7),
        handle_color: base_text,
        border: Border {
            radius: 2.0.into(),
            width: 0.0,
            color: base_text,
        },
        shadow: Shadow::default(),
    };

    match status {
        Status::Active => active,
        Status::Hovered | Status::Opened { .. } => Style {
            background: strong.into(),
            ..active
        },
        Status::Disabled => Style {
            text_color: active.text_color.scale_alpha(0.5),
            background: active.background.scale_alpha(0.5),
            placeholder_color: active.placeholder_color.scale_alpha(0.5),
            handle_color: active.handle_color.scale_alpha(0.5),
            ..active
        },
    }
}
