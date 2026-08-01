use iced::advanced::layout::{self, Layout};
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::text;
use iced::advanced::widget::{self, Operation, Tree, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::mouse;
use iced::widget::container;
use iced::{Element, Event, Length, Rectangle, Size, Vector};

use super::overlay::{DialogOverlay, DialogState};
use super::{Catalog, DialogButton, Style, StyleFn};

struct State {
    is_open: bool,
    dialog_state: DialogState,
}

/// A modal confirmation dialog that shows centered on the viewport.
///
/// `ConfirmationDialog` wraps a base content widget and shows a modal
/// dialog with a title, message, and configurable buttons when opened.
///
/// The dialog is controlled by the `is_open` parameter passed to
/// [`ConfirmationDialog::new`]. When the user clicks a button, the
/// button's action message is published. When the user dismisses the
/// dialog (click outside, Escape), the `on_dismiss` message is published.
///
/// # Example
///
/// ```no_run
/// use iced::widget::{button, text};
/// use iced::Element;
/// use neverliie_iced_widgets::confirmation_dialog::ConfirmationDialog;
///
/// enum Message {
///     Delete,
///     Cancel,
///     ShowDialog,
///     DismissDialog,
/// }
///
/// fn view(show_dialog: bool) -> Element<'_, Message> {
///     let content = button("Delete").on_press(Message::ShowDialog);
///
///     if show_dialog {
///         ConfirmationDialog::new(content, true, "Delete item?", "This cannot be undone.")
///             .on_confirm(Message::Delete)
///             .on_cancel(Message::Cancel)
///             .on_dismiss(Message::DismissDialog)
///             .into()
///     } else {
///         content.into()
///     }
/// }
/// ```
pub struct ConfirmationDialog<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Theme: Catalog,
{
    content: Element<'a, Message, Theme, Renderer>,
    is_open: bool,
    title: &'a str,
    message: &'a str,
    buttons: Vec<DialogButton<'a, Message>>,
    on_dismiss: Option<Message>,
    class: Theme::Class<'a>,
    blocking: bool,
    no_pointer: bool,
}

impl<'a, Message, Theme, Renderer> ConfirmationDialog<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
{
    /// Creates a new confirmation dialog wrapping the given content.
    ///
    /// - `content`: the base widget to render underneath the dialog
    /// - `is_open`: whether the dialog should be visible
    /// - `title`: the dialog title text
    /// - `message`: the dialog message/body text
    pub fn new(
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        is_open: bool,
        title: &'a str,
        message: &'a str,
    ) -> Self {
        Self {
            content: content.into(),
            is_open,
            title,
            message,
            buttons: Vec::new(),
            on_dismiss: None,
            class: Theme::default(),
            blocking: false,
            no_pointer: false,
        }
    }

    /// Sets the message to emit when the dialog is dismissed (clicked
    /// outside or pressed Escape).
    #[must_use]
    pub fn on_dismiss(mut self, message: Message) -> Self {
        self.on_dismiss = Some(message);
        self
    }

    /// Adds a "Confirm" button (convenience method).
    ///
    /// Shorthand for `.button(DialogButton::new("Confirm", message).style(ButtonStyle::Default))`.
    #[must_use]
    pub fn on_confirm(mut self, message: Message) -> Self {
        self.buttons.push(
            DialogButton::new("Confirm", message)
                .style(super::ButtonStyle::Default),
        );
        self
    }

    /// Adds a "Cancel" button (convenience method).
    ///
    /// Shorthand for `.button(DialogButton::new("Cancel", message).style(ButtonStyle::Secondary))`.
    #[must_use]
    pub fn on_cancel(mut self, message: Message) -> Self {
        self.buttons.push(
            DialogButton::new("Cancel", message)
                .style(super::ButtonStyle::Secondary),
        );
        self
    }

    /// Adds a custom button to the dialog.
    #[must_use]
    pub fn button(mut self, button: DialogButton<'a, Message>) -> Self {
        self.buttons.push(button);
        self
    }

    /// Makes the dialog blocking — the user must click a button to
    /// proceed. Cannot be dismissed by clicking outside or pressing Escape.
    #[must_use]
    pub fn blocking(mut self) -> Self {
        self.blocking = true;
        self
    }

    /// Prevents the dialog overlay from claiming the cursor interaction.
    ///
    /// When enabled, the overlay always returns the default cursor instead
    /// of a pointer, so the base cursor stays available to widgets
    /// underneath the dialog. This preserves hover detection (enter/exit
    /// events) in the widget tree below.
    ///
    /// The trade-off is that the mouse cursor won't change to a pointer
    /// when hovering dialog buttons. Button hover visual feedback (the
    /// brightened background) still works since it's drawn independently
    /// in the overlay.
    #[must_use]
    pub fn no_pointer(mut self) -> Self {
        self.no_pointer = true;
        self
    }

    /// Sets a custom styling function for the dialog.
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Sets the style class of the [`ConfirmationDialog`].
    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for ConfirmationDialog<'a, Message, Theme, Renderer>
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
            dialog_state: DialogState::new(),
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

        if self.is_open && !state.is_open {
            state.is_open = true;
            state.dialog_state = DialogState::new();
        } else if !self.is_open && state.is_open {
            state.is_open = false;
            state.dialog_state = DialogState::new();
        }

        if state.is_open && state.dialog_state.dismissed {
            state.is_open = false;
            if let Some(msg) = self.on_dismiss.clone() {
                shell.publish(msg);
            }
            state.dialog_state = DialogState::new();
            shell.request_redraw();
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
        _translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let content_overlay = self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            Vector::ZERO,
        );

        let state = tree.state.downcast_mut::<State>();

        if !self.is_open {
            return content_overlay;
        }

        // Sync state on first frame (overlay runs before update)
        if !state.is_open {
            state.is_open = true;
            state.dialog_state = DialogState::new();
        }

        let dialog_overlay = overlay::Element::new(Box::new(DialogOverlay::new(
            self.title,
            self.message,
            &self.buttons,
            &mut state.dialog_state,
            *viewport,
            &self.class,
            renderer.default_font(),
            self.blocking,
            self.no_pointer,
        )));

        match content_overlay {
            Some(existing) => Some(
                overlay::Group::with_children(vec![existing, dialog_overlay])
                    .overlay(),
            ),
            None => Some(dialog_overlay),
        }
    }
}

impl<'a, Message, Theme, Renderer>
    From<ConfirmationDialog<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: Catalog + container::Catalog + 'static + 'a,
    Renderer: renderer::Renderer + text::Renderer + 'a,
{
    fn from(dialog: ConfirmationDialog<'a, Message, Theme, Renderer>) -> Self {
        Element::new(dialog)
    }
}
