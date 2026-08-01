use iced::advanced::layout::{self, Layout};
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::text;
use iced::advanced::widget::{self, Operation, Tree, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::mouse;
use iced::widget::container;
use iced::{Element, Event, Length, Point, Rectangle, Size, Vector};

use super::overlay::{MenuOverlay, MenuState};
use super::{Catalog, Menu, Style, StyleFn};

fn has_nested_dismiss(state: &MenuState) -> bool {
    state
        .submenu_state
        .as_ref()
        .is_some_and(|s| s.dismissed || has_nested_dismiss(s))
}

struct State {
    is_open: bool,
    cursor_position: Point,
    menu_state: MenuState,
}

/// A context menu that shows a floating menu on right-click.
///
/// `ContextMenu` wraps a base content widget and shows a menu when
/// the user right-clicks on it.
///
/// # Example
///
/// ```no_run
/// use iced::widget::{container, text};
/// use iced::Element;
/// use never_lie_iced_widgets::context_menu::{ContextMenu, Menu};
///
/// enum Message {
///     Copy,
///     Paste,
///     DismissMenu,
/// }
///
/// fn view() -> Element<'_, Message> {
///     let content = container(text("Right-click me"))
///         .center_x(200)
///         .center_y(200);
///
///     let menu = Menu::new()
///         .item("Copy", Message::Copy)
///         .item("Paste", Message::Paste);
///
///     ContextMenu::new(content, menu)
///         .on_dismiss(Message::DismissMenu)
///         .into()
/// }
/// ```
pub struct ContextMenu<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Theme: Catalog,
{
    content: Element<'a, Message, Theme, Renderer>,
    menu: Menu<'a, Message>,
    on_dismiss: Option<Message>,
    on_right_click: Option<Message>,
    class: Theme::Class<'a>,
    text_size: Option<f32>,
}

impl<'a, Message, Theme, Renderer> ContextMenu<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
{
    /// Creates a new context menu wrapping the given content.
    pub fn new(
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        menu: Menu<'a, Message>,
    ) -> Self {
        Self {
            content: content.into(),
            menu,
            on_dismiss: None,
            on_right_click: None,
            class: Theme::default(),
            text_size: None,
        }
    }

    /// Sets the message to emit when the menu is dismissed (clicked outside).
    #[must_use]
    pub fn on_dismiss(mut self, message: Message) -> Self {
        self.on_dismiss = Some(message);
        self
    }

    /// Sets the message to emit when the content is right-clicked, before
    /// the menu opens.
    #[must_use]
    pub fn on_right_click(mut self, message: Message) -> Self {
        self.on_right_click = Some(message);
        self
    }

    /// Sets a custom styling function for the menu.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Sets the style class of the [`ContextMenu`].
    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }

    /// Sets the text size for menu items.
    #[must_use]
    pub fn text_size(mut self, size: f32) -> Self {
        self.text_size = Some(size);
        self
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for ContextMenu<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: Catalog + container::Catalog + 'a,
    Renderer: renderer::Renderer + text::Renderer + 'a,
{
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content.as_widget_mut().layout(
            &mut tree.children[0],
            renderer,
            limits,
        )
    }

    fn tag(&self) -> widget::tree::Tag {
        struct Marker;
        widget::tree::Tag::of::<Marker>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(State {
            is_open: false,
            cursor_position: Point::ORIGIN,
            menu_state: MenuState::new(),
        })
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&self.content]);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        if shell.is_event_captured() {
            return;
        }

        let state = tree.state.downcast_mut::<State>();

        let should_dismiss = state.menu_state.dismissed
            || has_nested_dismiss(&state.menu_state);

        if state.is_open && should_dismiss {
            state.is_open = false;
            state.menu_state = MenuState::new();
            shell.request_redraw();
        }

        if let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) =
            event
            && cursor.is_over(layout.bounds())
        {
            state.is_open = true;
            state.menu_state = MenuState::new();
            state.cursor_position = cursor.position().unwrap_or(state.cursor_position);
            if let Some(message) = self.on_right_click.as_ref() {
                shell.publish(message.clone());
            }
            shell.request_redraw();
            shell.capture_event();
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
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds());
        operation.traverse(&mut |operation| {
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                layout,
                renderer,
                operation,
            );
        });
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let state = tree.state.downcast_mut::<State>();

        if !state.is_open {
            return self.content.as_widget_mut().overlay(
                &mut tree.children[0],
                layout,
                renderer,
                viewport,
                Vector::ZERO,
            );
        }

        let Some(dismiss_message) = self.on_dismiss.clone() else {
            return self.content.as_widget_mut().overlay(
                &mut tree.children[0],
                layout,
                renderer,
                viewport,
                Vector::ZERO,
            );
        };

        let menu_position = state.cursor_position + translation;

        Some(overlay::Element::new(Box::new(MenuOverlay::new(
            &self.menu,
            &mut state.menu_state,
            menu_position,
            *viewport,
            &self.class,
            self.text_size,
            dismiss_message,
            renderer.default_font(),
            true,
        ))))
    }
}

impl<'a, Message, Theme, Renderer>
    From<ContextMenu<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: Catalog + container::Catalog + 'static + 'a,
    Renderer: renderer::Renderer + text::Renderer + 'a,
{
    fn from(menu: ContextMenu<'a, Message, Theme, Renderer>) -> Self {
        Element::new(menu)
    }
}
