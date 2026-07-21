use iced::advanced::layout::{self, Layout};
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::widget::{self, Operation, Tree, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::mouse;
use iced::widget::container;
use iced::{Element, Event, Length, Point, Rectangle, Size, Vector};

use super::{check_dismiss, clamp_to_viewport, DismissTrigger, Floating, Position};

struct State {
    cursor_position: Point,
    floating_bounds: Vec<Rectangle>,
}

/// A wrapper widget that renders floating children as overlays.
///
/// `OverlayManager` wraps a base content widget and renders floating
/// children on top using iced's overlay system. The floating content
/// does not affect the base layout.
///
/// # Example
///
/// ```ignore
/// use never_lie_iced_widgets::overlay::{Floating, OverlayManager, Position};
///
/// OverlayManager::new(content)
///     .overlay(
///         Floating::new(popup)
///             .position(Position::Bottom),
///     )
///     .on_dismiss(Message::Dismiss)
///     .into()
/// ```
///
/// # Multiple Overlays
///
/// You can add multiple floating children:
///
/// ```ignore
/// OverlayManager::new(content)
///     .overlay(Floating::new(tooltip).position(Position::Top))
///     .overlay(Floating::new(badge).position(Position::TopRight))
///     .into()
/// ```
pub struct OverlayManager<
    'a,
    Message,
    Theme = iced::Theme,
    Renderer = iced::Renderer,
> {
    content: Element<'a, Message, Theme, Renderer>,
    floating: Vec<Floating<'a, Message, Theme, Renderer>>,
    on_dismiss: Option<Message>,
    dismiss_trigger: DismissTrigger,
}

impl<'a, Message, Theme, Renderer> OverlayManager<'a, Message, Theme, Renderer> {
    /// Creates a new [`OverlayManager`] wrapping the given content.
    pub fn new(
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        Self {
            content: content.into(),
            floating: Vec::new(),
            on_dismiss: None,
            dismiss_trigger: DismissTrigger::default(),
        }
    }

    /// Adds a floating child element.
    #[must_use]
    pub fn overlay(
        mut self,
        floating: Floating<'a, Message, Theme, Renderer>,
    ) -> Self {
        self.floating.push(floating);
        self
    }

    /// Sets the message to emit when clicking outside all floating content.
    ///
    /// Uses [`DismissTrigger::AnyClickOutside`] by default — any mouse
    /// button press outside the floating content triggers dismissal.
    /// Use [`on_dismiss_left`] for left-click only, or
    /// [`on_dismiss_trigger`] for full control.
    ///
    /// [`on_dismiss_left`]: Self::on_dismiss_left
    /// [`on_dismiss_trigger`]: Self::on_dismiss_trigger
    #[must_use]
    pub fn on_dismiss(mut self, message: Message) -> Self {
        self.on_dismiss = Some(message);
        self.dismiss_trigger = DismissTrigger::AnyClickOutside;
        self
    }

    /// Sets the message to emit when left-clicking outside all floating
    /// content.
    #[must_use]
    pub fn on_dismiss_left(mut self, message: Message) -> Self {
        self.on_dismiss = Some(message);
        self.dismiss_trigger = DismissTrigger::LeftClickOutside;
        self
    }

    /// Sets the message to emit when the specified dismiss trigger fires
    /// outside all floating content.
    #[must_use]
    pub fn on_dismiss_trigger(
        mut self,
        message: Message,
        trigger: DismissTrigger,
    ) -> Self {
        self.on_dismiss = Some(message);
        self.dismiss_trigger = trigger;
        self
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for OverlayManager<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: container::Catalog,
    Renderer: renderer::Renderer,
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
            cursor_position: Point::ORIGIN,
            floating_bounds: Vec::new(),
        })
    }

    fn children(&self) -> Vec<Tree> {
        std::iter::once(Tree::new(&self.content))
            .chain(
                self.floating
                    .iter()
                    .map(|f| Tree::new(&f.content)),
            )
            .collect()
    }

    fn diff(&self, tree: &mut Tree) {
        let mut widgets: Vec<&dyn Widget<Message, Theme, Renderer>> =
            Vec::with_capacity(1 + self.floating.len());
        widgets.push(self.content.as_widget());
        for f in &self.floating {
            widgets.push(f.content.as_widget());
        }
        tree.diff_children(&widgets);
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
        // Track cursor position for Cursor follow mode
        if let Event::Mouse(mouse::Event::CursorMoved { position }) = event {
            let state = tree.state.downcast_mut::<State>();
            state.cursor_position = *position;
            if self.floating.iter().any(|f| {
                matches!(
                    f.position,
                    Position::Cursor { .. } | Position::FollowCursor
                )
            }) {
                shell.request_redraw();
            }
        }

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
        if self.floating.is_empty() {
            return self.content.as_widget_mut().overlay(
                &mut tree.children[0],
                layout,
                renderer,
                viewport,
                translation,
            );
        }

        let parent_bounds = layout.bounds() + translation;
        let cursor_position =
            tree.state.downcast_ref::<State>().cursor_position;
        let on_dismiss = self.on_dismiss.clone();
        let dismiss_trigger = self.dismiss_trigger;

        let (content_children, floating_children) =
            tree.children.split_at_mut(1);

        let content_overlay = self.content.as_widget_mut().overlay(
            &mut content_children[0],
            layout,
            renderer,
            viewport,
            translation,
        );

        // Compute floating layouts sequentially for inter-floating positioning
        let state = tree.state.downcast_mut::<State>();
        state.floating_bounds.clear();

        // Pre-compute all floating bounds first
        for (floating, tree_state) in
            self.floating.iter_mut().zip(floating_children.iter_mut())
        {
            let limits = layout::Limits::new(
                Size::ZERO,
                Size::new(viewport.width, viewport.height),
            );
            let content_layout = floating.content.as_widget_mut().layout(
                tree_state,
                renderer,
                &limits,
            );
            let content_bounds = content_layout.bounds();

            let position = floating.position.resolve(
                parent_bounds,
                cursor_position,
                *viewport,
                content_bounds,
                &state.floating_bounds,
            );
            let clamped =
                clamp_to_viewport(position, content_bounds.size(), *viewport);

            state.floating_bounds
                .push(Rectangle::new(clamped, content_bounds.size()));
        }

        // Now create overlays using the pre-computed bounds
        let floating_bounds_clone = state.floating_bounds.clone();

        let floating_overlays: Vec<
            overlay::Element<'b, Message, Theme, Renderer>,
        > = self
            .floating
            .iter_mut()
            .zip(floating_children.iter_mut())
            .map(|(floating, tree_state)| {
                overlay::Element::new(Box::new(Overlay {
                    floating: &mut floating.content,
                    tree: tree_state,
                    position: floating.position,
                    parent_bounds,
                    cursor_position,
                    on_dismiss: on_dismiss.clone(),
                    dismiss_trigger,
                    floating_bounds_owned: floating_bounds_clone.clone(),
                    index: floating.index,
                }))
            })
            .collect();

        let mut overlays: Vec<
            overlay::Element<'b, Message, Theme, Renderer>,
        > = content_overlay.into_iter().collect();
        overlays.extend(floating_overlays);

        (!overlays.is_empty())
            .then(|| overlay::Group::with_children(overlays).overlay())
    }
}

impl<'a, Message, Theme, Renderer>
    From<OverlayManager<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: container::Catalog + 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(manager: OverlayManager<'a, Message, Theme, Renderer>) -> Self {
        Element::new(manager)
    }
}

// --- Internal Overlay ---

struct Overlay<'a, 'b, Message, Theme, Renderer> {
    floating: &'b mut Element<'a, Message, Theme, Renderer>,
    tree: &'b mut Tree,
    position: Position,
    parent_bounds: Rectangle,
    cursor_position: Point,
    on_dismiss: Option<Message>,
    dismiss_trigger: DismissTrigger,
    floating_bounds_owned: Vec<Rectangle>,
    index: f32,
}

impl<Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer>
    for Overlay<'_, '_, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: container::Catalog,
    Renderer: renderer::Renderer,
{
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> layout::Node {
        let viewport = Rectangle::with_size(bounds);

        let content_layout = self.floating.as_widget_mut().layout(
            self.tree,
            renderer,
            &layout::Limits::new(Size::ZERO, bounds),
        );

        let content_bounds = content_layout.bounds();

        let position = self.position.resolve(
            self.parent_bounds,
            self.cursor_position,
            viewport,
            content_bounds,
            &self.floating_bounds_owned,
        );
        let clamped =
            clamp_to_viewport(position, content_bounds.size(), viewport);

        layout::Node::with_children(content_bounds.size(), vec![content_layout])
            .translate(Vector::new(clamped.x, clamped.y))
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        let viewport = layout.bounds();
        if let Some(child_layout) = layout.children().next() {
            self.floating.as_widget().draw(
                self.tree,
                renderer,
                theme,
                style,
                child_layout,
                cursor,
                &viewport,
            );
        }
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        let viewport = layout.bounds();

        if let Some(child_layout) = layout.children().next() {
            self.floating.as_widget_mut().update(
                self.tree,
                event,
                child_layout,
                cursor,
                renderer,
                clipboard,
                shell,
                &viewport,
            );
        }

        if let Some(msg) = check_dismiss(
            event,
            cursor,
            layout.bounds(),
            self.dismiss_trigger,
            &self.on_dismiss,
        ) {
            shell.publish(msg);
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let viewport = layout.bounds();
        if let Some(child_layout) = layout.children().next() {
            self.floating.as_widget().mouse_interaction(
                self.tree,
                child_layout,
                cursor,
                &viewport,
                renderer,
            )
        } else {
            mouse::Interaction::None
        }
    }

    fn operate(
        &mut self,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds());
        operation.traverse(&mut |operation| {
            if let Some(child_layout) = layout.children().next() {
                self.floating.as_widget_mut().operate(
                    self.tree,
                    child_layout,
                    renderer,
                    operation,
                );
            }
        });
    }

    fn index(&self) -> f32 {
        self.index
    }
}
