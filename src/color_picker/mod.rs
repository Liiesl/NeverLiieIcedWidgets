//! Use a color picker as an input element for picking colors.
//!
//! Ported from `iced_aw`'s `widget::color_picker` module, with the overlay
//! reworked to mirror the PySide6 `BetterColorDialog`:
//!
//! * a hue ring + saturation/value square picker on the left, next to
//!   RGB(A)/HSV tabbed gradient sliders, value fields and a hex input;
//! * on the right: Original/New preview panels, tabbed swatch sets (with
//!   an add-set name prompt and per-set close marks), an add-current-color
//!   button, a recent colors grid and the Reset/OK/Cancel buttons.
//!
//! Swatches and recent colors are kept in memory only (no persistence),
//! and all styling is derived from the active iced `Theme` palette.
//!
//! # Example
//! ```no_run
//! # use neverliie_iced_widgets::color_picker::{ColorPicker, color_picker};
//! # use neverliie_iced_widgets::overlay::Position;
//! # use iced::{Color, Element, widget::{button, Button, Text}};
//! #
//! #[derive(Clone, Debug)]
//! enum Message {
//!     Open,
//!     Cancel,
//!     Submit(Color),
//! }
//!
//! let color_picker = color_picker(
//!     true,
//!     Color::default(),
//!     button("Pick color").on_press(Message::Open),
//!     Message::Cancel,
//!     Message::Submit,
//! )
//! .position(Position::BottomRight);
//! # let _ = color_picker;
//! ```

mod color;
mod overlay;
pub mod style;
pub mod style_state;

use self::overlay::{ColorBarDragged, ColorPickerOverlay, ColorPickerOverlayButtons};
use self::style::{Status, Style, StyleFn};

use crate::overlay::Position;

use iced::{
    advanced::{
        layout::{Limits, Node},
        mouse::{self, Cursor},
        renderer,
        widget::{
            Operation,
            tree::{self, Tag, Tree},
        },
        Clipboard, Layout, Shell, Widget,
    },
    widget::Renderer,
    Color, Element, Event, Length, Point, Rectangle, Size, Vector,
};

//TODO: Remove ignore when Null is updated. Temp fix for Test runs
/// An input element for picking colors.
///
/// # Example
/// ```ignore
/// # use neverliie_iced_widgets::color_picker::ColorPicker;
/// # use iced::{Color, widget::{button, Button, Text}};
/// #
/// #[derive(Clone, Debug)]
/// enum Message {
///     Open,
///     Cancel,
///     Submit(Color),
/// }
///
/// let color_picker = ColorPicker::new(
///     true,
///     Color::default(),
///     Button::new(Text::new("Pick color"))
///         .on_press(Message::Open),
///     Message::Cancel,
///     Message::Submit,
/// );
/// ```
#[allow(missing_debug_implementations)]
pub struct ColorPicker<'a, Message, Theme = iced::Theme>
where
    Message: Clone,
    Theme: style::Catalog + iced::widget::button::Catalog,
{
    /// Show the picker.
    show_picker: bool,
    /// The color to show.
    color: Color,
    /// The underlying element.
    underlay: Element<'a, Message, Theme, Renderer>,
    /// The message that is sent if the cancel button of the [`ColorPickerOverlay`] is pressed.
    on_cancel: Message,
    /// The function that produces a message when the submit button of the [`ColorPickerOverlay`] is pressed.
    on_submit: Box<dyn Fn(Color) -> Message>,
    /// Optional function that produces a message when the color changes during selection (real-time updates).
    on_color_change: Option<Box<dyn Fn(Color) -> Message>>,
    /// The style of the [`ColorPickerOverlay`].
    class: <Theme as style::Catalog>::Class<'a>,
    /// The position of the [`ColorPickerOverlay`]; `None` centers the dialog
    /// over the underlay.
    position: Option<Position>,
    /// The buttons of the overlay.
    overlay_state: Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message, Theme> ColorPicker<'a, Message, Theme>
where
    Message: 'a + Clone,
    Theme: 'a
        + style::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
{
    /// Creates a new [`ColorPicker`] wrapping around the given underlay.
    ///
    /// It expects:
    ///     * if the overlay of the color picker is visible.
    ///     * the initial color to show.
    ///     * the underlay [`Element`] on which this [`ColorPicker`]
    ///         will be wrapped around.
    ///     * a message that will be send when the cancel button of the [`ColorPicker`]
    ///         is pressed.
    ///     * a function that will be called when the submit button of the [`ColorPicker`]
    ///         is pressed, which takes the picked [`Color`] value.
    pub fn new<U, F>(
        show_picker: bool,
        color: Color,
        underlay: U,
        on_cancel: Message,
        on_submit: F,
    ) -> Self
    where
        U: Into<Element<'a, Message, Theme, Renderer>>,
        F: 'static + Fn(Color) -> Message,
    {
        Self {
            show_picker,
            color,
            underlay: underlay.into(),
            on_cancel,
            on_submit: Box::new(on_submit),
            on_color_change: None,
            class: <Theme as style::Catalog>::default(),
            position: None,
            overlay_state: ColorPickerOverlayButtons::default().into(),
        }
    }

    /// Sets a callback that will be called whenever the color changes during selection (real-time updates).
    #[must_use]
    pub fn on_color_change<F>(mut self, on_color_change: F) -> Self
    where
        F: 'static + Fn(Color) -> Message,
    {
        self.on_color_change = Some(Box::new(on_color_change));
        self
    }

    /// Sets the position of the [`ColorPickerOverlay`].
    ///
    /// Uses the same [`Position`] strategies as the overlay widget:
    /// parent-relative, viewport-relative, cursor-following or absolute.
    /// Defaults to centering the dialog over the underlay.
    ///
    /// ```ignore
    /// color_picker(true, color, underlay, Message::Cancel, Message::Submit)
    ///     .position(Position::BottomRight)
    /// ```
    #[must_use]
    pub fn position(mut self, position: Position) -> Self {
        self.position = Some(position);
        self
    }

    /// Sets the style of the [`ColorPicker`].
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        <Theme as style::Catalog>::Class<'a>: From<StyleFn<'a, Theme, Style>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme, Style>).into();
        self
    }

    /// Sets the class of the input of the [`ColorPicker`].
    #[must_use]
    pub fn class(
        mut self,
        class: impl Into<<Theme as style::Catalog>::Class<'a>>,
    ) -> Self {
        self.class = class.into();
        self
    }
}

/// The state of the [`ColorPicker`].
#[derive(Debug, Default)]
pub struct State {
    /// The state of the overlay.
    pub(crate) overlay_state: overlay::State,
    /// Was overlay shown during the previous render?
    pub(crate) old_show_picker: bool,
    /// The last known cursor position, for cursor-following [`Position`]s.
    pub(crate) last_cursor_position: Point,
}

impl State {
    /// Creates a new [`State`].
    #[must_use]
    pub fn new(color: Color) -> Self {
        Self {
            overlay_state: overlay::State::new(color),
            old_show_picker: false,
            last_cursor_position: Point::ORIGIN,
        }
    }

    /// Resets the color of the state.
    pub fn reset(&mut self) {
        self.overlay_state.color = Color::from_rgb(0.5, 0.25, 0.25);
        self.overlay_state.color_bar_dragged = ColorBarDragged::None;
        self.overlay_state.sync_display();
    }

    /// Synchronize with the provided color when the picker is (re)opened.
    ///
    /// Keep the overlay state in sync. While the picker is open it "owns" the
    /// value (there is no other way the user can update its value): the initial
    /// color must stay frozen at the open-time color so the Original preview
    /// panel does not track live `on_color_change` updates. When it is reopened,
    /// reset the color to the provided one.
    fn synchronize(&mut self, show_picker: bool, color: Color) {
        if show_picker && !self.old_show_picker {
            self.overlay_state.force_synchronize(color);
        }
        self.old_show_picker = show_picker;
    }
}

impl<'a, Message, Theme> Widget<Message, Theme, Renderer> for ColorPicker<'a, Message, Theme>
where
    Message: 'static + Clone,
    Theme: 'a
        + style::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
{
    fn tag(&self) -> Tag {
        Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::new(self.color))
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.underlay), Tree::new(&self.overlay_state)]
    }

    fn diff(&self, tree: &mut Tree) {
        let color_picker_state = tree.state.downcast_mut::<State>();

        color_picker_state.synchronize(self.show_picker, self.color);

        tree.diff_children(&[&self.underlay, &self.overlay_state]);
    }

    fn size(&self) -> Size<Length> {
        self.underlay.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) -> Node {
        self.underlay
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn update(
        &mut self,
        state: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        // Track the cursor position for cursor-following positions, and
        // request redraws so the dialog keeps following the mouse.
        if let Event::Mouse(mouse::Event::CursorMoved { position }) = event {
            state.state.downcast_mut::<State>().last_cursor_position = *position;
            if self
                .position
                .is_some_and(|p| matches!(p, Position::Cursor { .. } | Position::FollowCursor))
            {
                shell.request_redraw();
            }
        }

        self.underlay.as_widget_mut().update(
            &mut state.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        state: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.underlay.as_widget().mouse_interaction(
            &state.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn draw(
        &self,
        state: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        self.underlay.as_widget().draw(
            &state.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn operate<'b>(
        &'b mut self,
        state: &'b mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation<()>,
    ) {
        self.underlay
            .as_widget_mut()
            .operate(&mut state.children[0], layout, renderer, operation);
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, Message, Theme, Renderer>> {
        let picker_state: &mut State = tree.state.downcast_mut();

        if !self.show_picker {
            return self.underlay.as_widget_mut().overlay(
                &mut tree.children[0],
                layout,
                renderer,
                viewport,
                translation,
            );
        }

        let bounds = layout.bounds();
        let fallback_center = Point::new(bounds.center_x(), bounds.center_y());
        let parent_bounds = bounds + translation;
        let cursor_position = picker_state.last_cursor_position;

        Some(
            ColorPickerOverlay::new(
                picker_state,
                self.on_cancel.clone(),
                &self.on_submit,
                self.on_color_change.as_deref(),
                self.position,
                parent_bounds,
                fallback_center,
                cursor_position,
                &self.class,
                &mut tree.children[1],
                *viewport,
            )
            .overlay(),
        )
    }
}

impl<'a, Message, Theme> From<ColorPicker<'a, Message, Theme>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'static + Clone,
    Theme: 'a
        + style::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
{
    fn from(color_picker: ColorPicker<'a, Message, Theme>) -> Self {
        Element::new(color_picker)
    }
}

/// Shortcut helper to create a [`ColorPicker`] Widget.
///
/// [`ColorPicker`]: crate::color_picker::ColorPicker
pub fn color_picker<'a, Message, Theme, F>(
    show_picker: bool,
    color: Color,
    underlay: impl Into<Element<'a, Message, Theme, Renderer>>,
    on_cancel: Message,
    on_submit: F,
) -> ColorPicker<'a, Message, Theme>
where
    Message: 'a + Clone,
    Theme: 'a
        + style::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
    F: 'static + Fn(Color) -> Message,
{
    ColorPicker::new(show_picker, color, underlay, on_cancel, on_submit)
}

/// Shortcut helper to create a [`ColorPicker`] Widget with real-time color change
/// callback.
///
/// [`ColorPicker`]: crate::color_picker::ColorPicker
pub fn color_picker_with_change<'a, Message, Theme, F, G>(
    show_picker: bool,
    color: Color,
    underlay: impl Into<Element<'a, Message, Theme, Renderer>>,
    on_cancel: Message,
    on_submit: F,
    on_color_change: G,
) -> ColorPicker<'a, Message, Theme>
where
    Message: 'a + Clone,
    Theme: 'a
        + style::Catalog
        + iced::widget::button::Catalog
        + iced::widget::text::Catalog
        + iced::widget::text_input::Catalog,
    F: 'static + Fn(Color) -> Message,
    G: 'static + Fn(Color) -> Message,
{
    ColorPicker::new(show_picker, color, underlay, on_cancel, on_submit)
        .on_color_change(on_color_change)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    enum TestMessage {
        Cancel,
        Submit(Color),
    }

    type TestColorPicker<'a> = ColorPicker<'a, TestMessage, iced::Theme>;

    fn create_test_button() -> iced::widget::Button<'static, TestMessage, iced::Theme> {
        iced::widget::button(iced::widget::Text::new("Pick"))
    }

    #[test]
    fn color_picker_new_with_picker_hidden() {
        let color = Color::from_rgb(0.5, 0.5, 0.5);
        let button = create_test_button();

        let picker = TestColorPicker::new(
            false,
            color,
            button,
            TestMessage::Cancel,
            TestMessage::Submit,
        );

        assert!(!picker.show_picker);
        assert_eq!(picker.color, color);
    }

    #[test]
    fn color_picker_new_with_picker_shown() {
        let color = Color::from_rgb(0.3, 0.6, 0.9);
        let button = create_test_button();

        let picker = TestColorPicker::new(
            true,
            color,
            button,
            TestMessage::Cancel,
            TestMessage::Submit,
        );

        assert!(picker.show_picker);
        assert_eq!(picker.color, color);
    }

    #[test]
    fn color_picker_default_position_is_none() {
        let button = create_test_button();
        let picker = TestColorPicker::new(
            false,
            Color::from_rgb(0.5, 0.5, 0.5),
            button,
            TestMessage::Cancel,
            TestMessage::Submit,
        );

        assert!(picker.position.is_none());
    }

    #[test]
    fn color_picker_position_builder_stores_position() {
        let button = create_test_button();
        let picker = TestColorPicker::new(
            false,
            Color::from_rgb(0.5, 0.5, 0.5),
            button,
            TestMessage::Cancel,
            TestMessage::Submit,
        )
        .position(Position::BottomRight);

        assert_eq!(picker.position, Some(Position::BottomRight));
    }

    #[test]
    fn color_picker_state_new() {
        let color = Color::from_rgb(0.5, 0.5, 0.5);
        let state = State::new(color);

        assert!(!state.old_show_picker);
        assert_eq!(state.last_cursor_position, Point::ORIGIN);
    }

    #[test]
    fn color_picker_state_default() {
        let state = State::default();

        assert!(!state.old_show_picker);
    }

    #[test]
    fn color_picker_state_reset() {
        let color = Color::from_rgb(0.5, 0.5, 0.5);
        let mut state = State::new(color);

        state.reset();
        // State should still be valid after reset
        assert!(!state.old_show_picker);
    }

    #[test]
    fn synchronize_freeze_initial_color_while_open() {
        let open_color = Color::from_rgb(0.3, 0.6, 0.9);
        let live_color = Color::from_rgb(1.0, 0.0, 0.0);
        let mut state = State::new(open_color);

        state.synchronize(true, open_color);
        assert_eq!(state.overlay_state.color, open_color);
        assert_eq!(state.overlay_state.initial_color, open_color);

        // Live `on_color_change` re-renders must not clobber the initial
        // color while the picker is open.
        state.synchronize(true, live_color);
        assert_eq!(state.overlay_state.color, open_color);
        assert_eq!(state.overlay_state.initial_color, open_color);

        // Reopening with a different color resets both.
        state.synchronize(false, open_color);
        state.synchronize(true, live_color);
        assert_eq!(state.overlay_state.color, live_color);
        assert_eq!(state.overlay_state.initial_color, live_color);
    }
}