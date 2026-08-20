//! Build and show dropdown menus with items, icons, labels and separators.
use iced::advanced::layout::{self, Layout};
use iced::advanced::mouse;
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::text::{self, Text};
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{Clipboard, Shell, Widget};
use iced::border::{self, Border};
use iced::widget::scrollable::{self, Scrollable};
use iced::widget::text_input::{self, TextInput};
use iced::{
    alignment, keyboard, touch, window, Background, Color, Element, Event,
    Length, Padding, Pixels, Point, Rectangle, Shadow, Size, Theme, Vector,
};

use super::{MenuItem, ICON_SPACING, ICON_WIDTH, SEPARATOR_HEIGHT};

/// The local message produced by the search box of a [`Menu`].
#[derive(Debug, Clone)]
enum SearchMessage {
    /// The search query changed.
    TextChanged(String),
}

/// A list of selectable options.
pub struct Menu<
    'a,
    'b,
    T,
    Message,
    Theme = iced::Theme,
    Renderer = iced::Renderer,
> where
    Theme: Catalog,
    Renderer: text::Renderer,
    'b: 'a,
{
    state: &'a mut State,
    options: &'a mut [MenuItem<'b, T, Message, Theme, Renderer>],
    hovered_option: &'a mut Option<usize>,
    icon_trees: &'a mut Vec<Option<Tree>>,
    on_selected: Box<dyn FnMut(T) -> Message + 'a>,
    on_option_hovered: Option<&'a dyn Fn(T) -> Message>,
    on_new_item: Option<Message>,
    new_item_label: Option<String>,
    new_item_icon: Option<Element<'b, Message, Theme, Renderer>>,
    width: f32,
    padding: Padding,
    text_size: Option<Pixels>,
    text_line_height: text::LineHeight,
    text_shaping: text::Shaping,
    font: Option<Renderer::Font>,
    class: &'a <Theme as Catalog>::Class<'b>,
    searchable: bool,
}

impl<'a, 'b, T, Message, Theme, Renderer>
    Menu<'a, 'b, T, Message, Theme, Renderer>
where
    T: ToString + Clone,
    Message: Clone + 'a,
    Theme: Catalog + 'a,
    Renderer: text::Renderer + 'a,
    'b: 'a,
{
    /// Creates a new [`Menu`] with the given [`State`], a list of options,
    /// the message to produced when an option is selected, and its [`Style`].
    pub fn new(
        state: &'a mut State,
        options: &'a mut [MenuItem<'b, T, Message, Theme, Renderer>],
        hovered_option: &'a mut Option<usize>,
        icon_trees: &'a mut Vec<Option<Tree>>,
        on_selected: impl FnMut(T) -> Message + 'a,
        on_option_hovered: Option<&'a dyn Fn(T) -> Message>,
        on_new_item: Option<Message>,
        new_item_label: Option<String>,
        new_item_icon: Option<Element<'b, Message, Theme, Renderer>>,
        class: &'a <Theme as Catalog>::Class<'b>,
    ) -> Self {
        Menu {
            state,
            options,
            hovered_option,
            icon_trees,
            on_selected: Box::new(on_selected),
            on_option_hovered,
            on_new_item,
            new_item_label,
            new_item_icon,
            width: 0.0,
            padding: Padding::ZERO,
            text_size: None,
            text_line_height: text::LineHeight::default(),
            text_shaping: text::Shaping::default(),
            font: None,
            class,
            searchable: false,
        }
    }

    /// Sets the width of the [`Menu`].
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Sets the [`Padding`] of the [`Menu`].
    pub fn padding<P: Into<Padding>>(mut self, padding: P) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the text size of the [`Menu`].
    pub fn text_size(mut self, text_size: impl Into<Pixels>) -> Self {
        self.text_size = Some(text_size.into());
        self
    }

    /// Sets the text [`text::LineHeight`] of the [`Menu`].
    pub fn text_line_height(
        mut self,
        line_height: impl Into<text::LineHeight>,
    ) -> Self {
        self.text_line_height = line_height.into();
        self
    }

    /// Sets the [`text::Shaping`] strategy of the [`Menu`].
    pub fn text_shaping(mut self, shaping: text::Shaping) -> Self {
        self.text_shaping = shaping;
        self
    }

    /// Sets the font of the [`Menu`].
    pub fn font(mut self, font: impl Into<Renderer::Font>) -> Self {
        self.font = Some(font.into());
        self
    }

    /// Enables or disables the search box at the top of the [`Menu`].
    pub fn searchable(mut self, searchable: bool) -> Self {
        self.searchable = searchable;
        self
    }

    /// Turns the [`Menu`] into an overlay [`Element`] at the given target
    /// position.
    ///
    /// The `target_height` will be used to display the menu either on top
    /// of the target or under it, depending on the screen position and the
    /// dimensions of the [`Menu`].
    pub fn overlay(
        self,
        position: Point,
        viewport: Rectangle,
        target_height: f32,
        menu_height: Length,
    ) -> overlay::Element<'a, Message, Theme, Renderer>
    where
        <Theme as text_input::Catalog>::Class<'a>:
            From<text_input::StyleFn<'a, Theme>>,
    {
        overlay::Element::new(Box::new(Overlay::new(
            position,
            viewport,
            self,
            target_height,
            menu_height,
        )))
    }
}

/// The local state of a [`Menu`].
#[derive(Debug)]
pub struct State {
    tree: Tree,
    /// The current search query, if the [`Menu`] is searchable.
    pub(crate) search: String,
    /// The widget tree of the search box.
    pub(crate) search_tree: Option<Tree>,
    /// Whether the search box should grab focus the next time it is built.
    pub(crate) search_focus: bool,
    /// The widget tree of the new item footer icon.
    pub(crate) new_item_icon_tree: Option<Tree>,
    /// Whether the new item footer row is hovered.
    pub(crate) new_item_hovered: bool,
}

impl State {
    /// Creates a new [`State`] for a [`Menu`].
    pub fn new() -> Self {
        Self {
            tree: Tree::empty(),
            search: String::new(),
            search_tree: None,
            search_focus: false,
            new_item_icon_tree: None,
            new_item_hovered: false,
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

struct Overlay<'a, 'b, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    position: Point,
    viewport: Rectangle,
    tree: &'a mut Tree,
    list: Scrollable<'a, Message, Theme, Renderer>,
    search: Option<&'a mut String>,
    search_input: Option<TextInput<'a, SearchMessage, Theme, Renderer>>,
    search_tree: Option<&'a mut Option<Tree>>,
    new_item: Option<NewItem<'a, 'b, Message, Theme, Renderer>>,
    padding: Padding,
    text_size: Option<Pixels>,
    text_line_height: text::LineHeight,
    text_shaping: text::Shaping,
    font: Option<Renderer::Font>,
    width: f32,
    target_height: f32,
    class: &'a <Theme as Catalog>::Class<'b>,
}

/// The pinned "+ New Item" footer row of an [`Overlay`].
struct NewItem<'a, 'b, Message, Theme, Renderer> {
    label: String,
    icon: Option<Element<'b, Message, Theme, Renderer>>,
    icon_tree: &'a mut Option<Tree>,
    hovered: &'a mut bool,
    on_press: Option<Message>,
}

impl<'a, 'b, Message, Theme, Renderer> Overlay<'a, 'b, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: Catalog + scrollable::Catalog + 'a,
    Renderer: text::Renderer + 'a,
    'b: 'a,
{
    pub fn new<T>(
        position: Point,
        viewport: Rectangle,
        menu: Menu<'a, 'b, T, Message, Theme, Renderer>,
        target_height: f32,
        menu_height: Length,
    ) -> Self
    where
        T: Clone + ToString,
        <Theme as text_input::Catalog>::Class<'a>:
            From<text_input::StyleFn<'a, Theme>>,
    {
        let Menu {
            state,
            options,
            hovered_option,
            icon_trees,
            on_selected,
            on_option_hovered,
            on_new_item,
            new_item_label,
            mut new_item_icon,
            width,
            padding,
            font,
            text_size,
            text_line_height,
            text_shaping,
            class,
            searchable,
        } = menu;

        let tree = &mut state.tree;
        let search = searchable.then(|| &mut state.search);
        let mut search_tree = if searchable {
            Some(&mut state.search_tree)
        } else {
            None
        };

        let should_focus_search = state.search_focus;
        state.search_focus = false;

        let search_input = if searchable {
            let mut input = TextInput::new(
                "Search...",
                search.as_ref().map(|s| s.as_str()).unwrap_or(""),
            )
            .on_input(SearchMessage::TextChanged)
            .padding(Padding::new(4.0))
            .width(Length::Fill)
            .style(|theme, _status| {
                let menu_style = Catalog::style(theme, class);

                let selected = match menu_style.selected_background {
                    Background::Color(color) => color,
                    Background::Gradient(_) => menu_style.text_color,
                };

                text_input::Style {
                    background: menu_style.background,
                    border: Border {
                        radius: 4.0.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    icon: menu_style.text_color,
                    placeholder: menu_style.label_color,
                    value: menu_style.text_color,
                    selection: selected,
                }
            });

            if let Some(font) = font {
                input = input.font(font);
            }
            if let Some(text_size) = text_size {
                input = input.size(text_size);
            }

            if let Some(search_tree) = search_tree.as_mut() {
                let tree = search_tree
                    .get_or_insert_with(|| Tree::new(&input as &dyn Widget<_, _, _>));
                tree.diff(&input as &dyn Widget<_, _, _>);

                if should_focus_search {
                    tree.state
                        .downcast_mut::<text_input::State<Renderer::Paragraph>>()
                        .focus();
                }
            }
            Some(input)
        } else {
            None
        };

        let list = Scrollable::new(List {
            options,
            hovered_option,
            icon_trees,
            on_selected,
            on_option_hovered,
            font,
            text_size,
            text_line_height,
            text_shaping,
            padding,
            class,
            search: search.as_ref().map(|s| s.as_str()).unwrap_or("").to_owned(),
        })
        .height(menu_height);

        tree.diff(&list as &dyn Widget<_, _, _>);

        let new_item = if on_new_item.is_some() {
            let label = new_item_label.unwrap_or_else(|| "+ New Item".to_string());
            let icon_tree = &mut state.new_item_icon_tree;

            if let Some(icon) = new_item_icon.as_mut() {
                let widget = icon.as_widget();

                match icon_tree {
                    Some(tree) => tree.diff(widget),
                    None => *icon_tree = Some(Tree::new(widget)),
                }
            } else {
                *icon_tree = None;
            }

            Some(NewItem {
                label,
                icon: new_item_icon,
                icon_tree: &mut state.new_item_icon_tree,
                hovered: &mut state.new_item_hovered,
                on_press: on_new_item,
            })
        } else {
            None
        };

        Self {
            position,
            viewport,
            tree,
            list,
            search,
            search_input,
            search_tree,
            new_item,
            padding,
            text_size,
            text_line_height,
            text_shaping,
            font,
            width,
            target_height,
            class,
        }
    }
}

impl<Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer>
    for Overlay<'_, '_, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: Catalog,
    Renderer: text::Renderer,
{
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> layout::Node {
        let space_below =
            bounds.height - (self.position.y + self.target_height);
        let space_above = self.position.y;

        let mut input_height = 0.0;
        let input_node = if let Some(input) = self.search_input.as_mut() {
            let limits = layout::Limits::new(
                Size::ZERO,
                Size::new(bounds.width - self.position.x, f32::INFINITY),
            )
            .width(self.width);

            let mut node = input.layout(
                self.search_tree
                    .as_deref_mut()
                    .unwrap()
                    .as_mut()
                    .unwrap(),
                renderer,
                &limits,
                None,
            );
            input_height = node.size().height;
            node.move_to_mut(Point::new(0.0, 0.0));
            Some(node)
        } else {
            None
        };

        let limits = layout::Limits::new(
            Size::ZERO,
            Size::new(
                bounds.width - self.position.x,
                if space_below > space_above {
                    (space_below - input_height).max(0.0)
                } else {
                    (space_above - input_height).max(0.0)
                },
            ),
        )
        .width(self.width);

        let mut list_node = self.list.layout(self.tree, renderer, &limits);
        let list_size = list_node.size();
        list_node.move_to_mut(Point::new(0.0, input_height));

        let mut footer_height = 0.0;
        let footer_node = if let Some(new_item) = self.new_item.as_mut() {
            let text_size =
                self.text_size.unwrap_or_else(|| renderer.default_size());
            let item_height = f32::from(
                self.text_line_height.to_absolute(text_size),
            ) + self.padding.y();
            let height = SEPARATOR_HEIGHT + item_height;
            footer_height = height;

            let mut children = Vec::new();

            if let Some(icon) = new_item.icon.as_mut()
                && let Some(icon_tree) = new_item.icon_tree.as_mut()
            {
                let icon_limits = layout::Limits::new(
                    Size::ZERO,
                    Size::new(ICON_WIDTH, item_height),
                );
                let mut icon_node = icon
                    .as_widget_mut()
                    .layout(icon_tree, renderer, &icon_limits);
                let icon_size = icon_node.size();

                let x = self.padding.left + (ICON_WIDTH - icon_size.width) / 2.0;
                let cy = SEPARATOR_HEIGHT + (item_height - icon_size.height) / 2.0;
                icon_node.move_to_mut(Point::new(x, cy));
                children.push(icon_node);
            }

            Some(
                layout::Node::with_children(
                    Size::new(list_size.width, height),
                    children,
                )
                .move_to(Point::new(0.0, input_height + list_size.height)),
            )
        } else {
            None
        };

        let size = Size::new(
            list_size
                .width
                .max(input_node.as_ref().map(|n| n.size().width).unwrap_or(0.0)),
            input_height + list_size.height + footer_height,
        );

        let children = input_node
            .into_iter()
            .chain([list_node])
            .chain(footer_node)
            .collect();

        layout::Node::with_children(size, children).move_to(
            if space_below > space_above {
                self.position + Vector::new(0.0, self.target_height)
            } else {
                self.position - Vector::new(0.0, size.height)
            },
        )
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
        let bounds = layout.bounds();

        if let Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Escape),
            ..
        }) = event
        {
            // Let the event bubble down to the dropdown so it can close
            // without being captured by the search box.
            return;
        }

        let mut children = layout.children();

        if let Some(input) = self.search_input.as_mut()
            && let Some(input_layout) = children.next()
            && let Some(input_tree) = self
                .search_tree
                .as_deref_mut()
                .and_then(Option::as_mut)
        {
            let input_bounds = input_layout.bounds();

            let mut local_messages = Vec::new();
            let mut local_shell = Shell::new(&mut local_messages);

            input.update(
                input_tree,
                event,
                input_layout,
                cursor,
                renderer,
                clipboard,
                &mut local_shell,
                &input_bounds,
            );

            if local_shell.is_event_captured() {
                shell.capture_event();
            }

            shell.request_redraw_at(local_shell.redraw_request());
            shell.request_input_method(local_shell.input_method());

            for message in local_messages {
                match message {
                    SearchMessage::TextChanged(value) => {
                        if let Some(search) = self.search.as_deref_mut() {
                            *search = value;
                        }

                        shell.request_redraw();
                    }
                }
            }
        }

        if let Some(list_layout) = children.next() {
            self.list.update(
                self.tree,
                event,
                list_layout,
                cursor,
                renderer,
                clipboard,
                shell,
                &bounds,
            );
        }

        if let Some(footer_layout) = children.next()
            && let Some(new_item) = self.new_item.as_mut()
        {
            let footer_bounds = footer_layout.bounds();

            match event {
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                | Event::Touch(touch::Event::FingerPressed { .. }) => {
                    if cursor.is_over(footer_bounds) {
                        // Not captured: the click bubbles down to the
                        // dropdown, which closes the open menu.
                        if let Some(message) = &new_item.on_press {
                            shell.publish(message.clone());
                        }
                    }
                }
                Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                    let is_hovered = cursor.is_over(footer_bounds);

                    if is_hovered != *new_item.hovered {
                        *new_item.hovered = is_hovered;
                        shell.request_redraw();
                    }
                }
                _ => {}
            }
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let mut children = layout.children();

        if let Some(input) = self.search_input.as_ref()
            && let Some(input_layout) = children.next()
            && cursor.is_over(input_layout.bounds())
        {
            return Widget::mouse_interaction(
                input,
                self.search_tree
                    .as_deref()
                    .unwrap()
                    .as_ref()
                    .unwrap(),
                input_layout,
                cursor,
                &self.viewport,
                renderer,
            );
        }

        let _ = children.next();

        if let Some(footer_layout) = children.next()
            && self.new_item.is_some()
            && cursor.is_over(footer_layout.bounds())
        {
            return mouse::Interaction::Pointer;
        }

        self.list
            .mouse_interaction(self.tree, layout, cursor, &self.viewport, renderer)
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        defaults: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        let bounds = layout.bounds();

        let style = Catalog::style(theme, self.class);

        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: style.border,
                shadow: style.shadow,
                ..renderer::Quad::default()
            },
            style.background,
        );

        let mut children = layout.children();

        if let Some(input) = self.search_input.as_ref()
            && let Some(input_layout) = children.next()
        {
            input.draw(
                self.search_tree
                    .as_deref()
                    .unwrap()
                    .as_ref()
                    .unwrap(),
                renderer,
                theme,
                input_layout,
                cursor,
                None,
                &bounds,
            );
        }

        if let Some(list_layout) = children.next() {
            self.list.draw(
                self.tree,
                renderer,
                theme,
                defaults,
                list_layout,
                cursor,
                &bounds,
            );
        }

        if let Some(footer_layout) = children.next()
            && let Some(new_item) = self.new_item.as_ref()
        {
            let footer_bounds = footer_layout.bounds();
            let is_hovered = *new_item.hovered;
            let text_size =
                self.text_size.unwrap_or_else(|| renderer.default_size());

            let sep_y = footer_bounds.y + SEPARATOR_HEIGHT / 2.0;

            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle::new(
                        Point::new(
                            footer_bounds.x + self.padding.left,
                            sep_y - 0.5,
                        ),
                        Size::new(
                            footer_bounds.width - self.padding.x(),
                            1.0,
                        ),
                    ),
                    border: border::rounded(0.0),
                    ..renderer::Quad::default()
                },
                style.separator_color,
            );

            let row_bounds = Rectangle {
                x: footer_bounds.x,
                y: footer_bounds.y + SEPARATOR_HEIGHT,
                width: footer_bounds.width,
                height: footer_bounds.height - SEPARATOR_HEIGHT,
            };

            if is_hovered {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle {
                            x: row_bounds.x + style.border.width,
                            width: row_bounds.width - style.border.width * 2.0,
                            ..row_bounds
                        },
                        border: border::rounded(style.border.radius),
                        ..renderer::Quad::default()
                    },
                    style.selected_background,
                );
            }

            let mut footer_children = footer_layout.children();

            if let Some(icon) = new_item.icon.as_ref()
                && let Some(icon_layout) = footer_children.next()
                && let Some(icon_tree) = new_item.icon_tree.as_ref()
            {
                icon.as_widget().draw(
                    icon_tree,
                    renderer,
                    theme,
                    defaults,
                    icon_layout,
                    cursor,
                    &bounds,
                );
            }

            let label_x_offset = if new_item.icon.is_some() {
                ICON_WIDTH + ICON_SPACING
            } else {
                0.0
            };

            renderer.fill_text(
                Text {
                    content: new_item.label.clone(),
                    bounds: Size::new(f32::INFINITY, row_bounds.height),
                    size: text_size,
                    line_height: self.text_line_height,
                    font: self.font.unwrap_or_else(|| renderer.default_font()),
                    align_x: text::Alignment::Default,
                    align_y: alignment::Vertical::Center,
                    shaping: self.text_shaping,
                    wrapping: text::Wrapping::default(),
                },
                Point::new(
                    row_bounds.x + self.padding.left + label_x_offset,
                    row_bounds.center_y(),
                ),
                if is_hovered {
                    style.selected_text_color
                } else {
                    style.text_color
                },
                bounds,
            );
        }
    }
}

struct List<'a, 'b, T, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    options: &'a mut [MenuItem<'b, T, Message, Theme, Renderer>],
    hovered_option: &'a mut Option<usize>,
    icon_trees: &'a mut Vec<Option<Tree>>,
    on_selected: Box<dyn FnMut(T) -> Message + 'a>,
    on_option_hovered: Option<&'a dyn Fn(T) -> Message>,
    padding: Padding,
    text_size: Option<Pixels>,
    text_line_height: text::LineHeight,
    text_shaping: text::Shaping,
    font: Option<Renderer::Font>,
    class: &'a <Theme as Catalog>::Class<'b>,
    search: String,
}

struct ListState {
    is_hovered: Option<bool>,
    mask: Vec<bool>,
    no_matches: bool,
    last_search: String,
}

impl<T, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for List<'_, '_, T, Message, Theme, Renderer>
where
    T: Clone + ToString,
    Theme: Catalog,
    Renderer: text::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<ListState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(ListState {
            is_hovered: None,
            mask: Vec::new(),
            no_matches: false,
            last_search: String::new(),
        })
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Shrink,
        }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        use std::f32;

        let text_size =
            self.text_size.unwrap_or_else(|| renderer.default_size());
        let item_height =
            f32::from(self.text_line_height.to_absolute(text_size))
                + self.padding.y();

        let state = tree.state.downcast_mut::<ListState>();

        state.mask = self.compute_mask();
        state.no_matches =
            !self.search.is_empty() && !state.mask.iter().any(|&v| v);
        state.last_search.clone_from(&self.search);

        if let Some(hovered) = self.hovered_option {
            if !state.mask.get(*hovered).copied().unwrap_or(false) {
                *self.hovered_option = None;
            }
        }

        let size = {
            let intrinsic = Size::new(
                0.0,
                self.total_height(&state.mask, state.no_matches, item_height),
            );

            limits.resolve(Length::Fill, Length::Shrink, intrinsic)
        };

        let mut children = Vec::new();
        let mut y = 0.0f32;

        for i in 0..self.options.len() {
            match &mut self.options[i] {
                MenuItem::Item(item) => {
                    if state.mask[i] {
                        if let Some(icon) = item.icon.as_mut()
                            && let Some(icon_tree) = self.icon_trees[i].as_mut()
                        {
                            let icon_limits = layout::Limits::new(
                                Size::ZERO,
                                Size::new(ICON_WIDTH, item_height),
                            );
                            let mut icon_node = icon
                                .as_widget_mut()
                                .layout(icon_tree, renderer, &icon_limits);
                            let icon_size = icon_node.size();

                            let x = self.padding.left
                                + (ICON_WIDTH - icon_size.width) / 2.0;
                            let cy = y + (item_height - icon_size.height) / 2.0;
                            icon_node.move_to_mut(Point::new(x, cy));
                            children.push(icon_node);
                        }
                        y += item_height;
                    }
                }
                MenuItem::Label(_) => {
                    if state.mask[i] {
                        y += item_height;
                    }
                }
                MenuItem::Separator => {
                    if state.mask[i] {
                        y += SEPARATOR_HEIGHT;
                    }
                }
            }
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
        let state = tree.state.downcast_mut::<ListState>();

        if state.last_search != self.search {
            state.mask = self.compute_mask();
            state.no_matches =
                !self.search.is_empty() && !state.mask.iter().any(|&v| v);
            state.last_search.clone_from(&self.search);

            if let Some(hovered) = self.hovered_option {
                if !state.mask.get(*hovered).copied().unwrap_or(false) {
                    *self.hovered_option = None;
                }
            }
        }

        let text_size =
            self.text_size.unwrap_or_else(|| renderer.default_size());

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(cursor_position) = cursor.position()
                    && let Some(index) = self.item_at(
                        cursor_position,
                        layout.bounds(),
                        &state.mask,
                        text_size,
                    )
                    && let Some(MenuItem::Item(item)) = self.options.get(index)
                {
                    shell.publish((self.on_selected)(item.value.clone()));
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some(cursor_position) = cursor.position() {
                    if let Some(index) = self.item_at(
                        cursor_position,
                        layout.bounds(),
                        &state.mask,
                        text_size,
                    ) {
                        if *self.hovered_option != Some(index) {
                            if let Some(MenuItem::Item(item)) =
                                self.options.get(index)
                                && let Some(on_option_hovered) =
                                    self.on_option_hovered
                            {
                                shell.publish(on_option_hovered(
                                    item.value.clone(),
                                ));
                            }

                            shell.request_redraw();
                        }

                        *self.hovered_option = Some(index);
                    } else if self.hovered_option.is_some() {
                        *self.hovered_option = None;
                        shell.request_redraw();
                    }
                }
            }
            Event::Touch(touch::Event::FingerPressed { .. }) => {
                if let Some(cursor_position) = cursor.position()
                    && let Some(index) = self.item_at(
                        cursor_position,
                        layout.bounds(),
                        &state.mask,
                        text_size,
                    )
                    && let Some(MenuItem::Item(item)) = self.options.get(index)
                {
                    shell.publish((self.on_selected)(item.value.clone()));
                    shell.capture_event();
                }
            }
            Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) => {
                match key.as_ref() {
                    keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                        self.move_hover(&state.mask, 1);
                        shell.request_redraw();
                        shell.capture_event();
                    }
                    keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                        self.move_hover(&state.mask, -1);
                        shell.request_redraw();
                        shell.capture_event();
                    }
                    keyboard::Key::Named(keyboard::key::Named::Enter) => {
                        if let Some(index) = *self.hovered_option
                            && state.mask.get(index).copied().unwrap_or(false)
                            && let Some(MenuItem::Item(item)) =
                                self.options.get(index)
                        {
                            shell.publish((self.on_selected)(item.value.clone()));
                            shell.capture_event();
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        if let Event::Window(window::Event::RedrawRequested(_now)) = event {
            state.is_hovered = Some(cursor.is_over(layout.bounds()));
        } else if state.is_hovered.is_some_and(|is_hovered| {
            is_hovered != cursor.is_over(layout.bounds())
        }) {
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
        let is_mouse_over = cursor.is_over(layout.bounds());

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
        let menu_style = Catalog::style(theme, self.class);
        let bounds = layout.bounds();

        let state = tree.state.downcast_ref::<ListState>();

        let text_size =
            self.text_size.unwrap_or_else(|| renderer.default_size());
        let item_height =
            f32::from(self.text_line_height.to_absolute(text_size))
                + self.padding.y();

        let has_icons = self.options.iter().any(|entry| {
            matches!(entry, MenuItem::Item(item) if item.icon.is_some())
        });
        let label_x_offset = if has_icons {
            ICON_WIDTH + ICON_SPACING
        } else {
            0.0
        };

        let mut children = layout.children();
        let mut y = 0.0f32;

        for (i, entry) in self.options.iter().enumerate() {
            match entry {
                MenuItem::Item(item) => {
                    if !state.mask[i] {
                        continue;
                    }

                    let row_bounds = Rectangle {
                        x: bounds.x,
                        y: bounds.y + y,
                        width: bounds.width,
                        height: item_height,
                    };

                    if row_bounds.y < viewport.y + viewport.height
                        && row_bounds.y + row_bounds.height > viewport.y
                    {
                        let is_selected = *self.hovered_option == Some(i);

                        if is_selected {
                            renderer.fill_quad(
                                renderer::Quad {
                                    bounds: Rectangle {
                                        x: row_bounds.x + menu_style.border.width,
                                        width: row_bounds.width
                                            - menu_style.border.width * 2.0,
                                        ..row_bounds
                                    },
                                    border: border::rounded(
                                        menu_style.border.radius,
                                    ),
                                    ..renderer::Quad::default()
                                },
                                menu_style.selected_background,
                            );
                        }

                        if let Some(icon) = item.icon.as_ref()
                            && let Some(icon_layout) = children.next()
                            && let Some(icon_tree) = self.icon_trees[i].as_ref()
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

                        renderer.fill_text(
                            Text {
                                content: item.label(),
                                bounds: Size::new(f32::INFINITY, row_bounds.height),
                                size: text_size,
                                line_height: self.text_line_height,
                                font: self
                                    .font
                                    .unwrap_or_else(|| renderer.default_font()),
                                align_x: text::Alignment::Default,
                                align_y: alignment::Vertical::Center,
                                shaping: self.text_shaping,
                                wrapping: text::Wrapping::default(),
                            },
                            Point::new(
                                row_bounds.x + self.padding.left + label_x_offset,
                                row_bounds.center_y(),
                            ),
                            if is_selected {
                                menu_style.selected_text_color
                            } else {
                                menu_style.text_color
                            },
                            *viewport,
                        );
                    }

                    y += item_height;
                }
                MenuItem::Label(label) => {
                    if !state.mask[i] {
                        continue;
                    }

                    let row_bounds = Rectangle {
                        x: bounds.x,
                        y: bounds.y + y,
                        width: bounds.width,
                        height: item_height,
                    };

                    if row_bounds.y < viewport.y + viewport.height
                        && row_bounds.y + row_bounds.height > viewport.y
                    {
                        renderer.fill_text(
                            Text {
                                content: (*label).to_string(),
                                bounds: Size::new(f32::INFINITY, row_bounds.height),
                                size: text_size * 0.8,
                                line_height: self.text_line_height,
                                font: self
                                    .font
                                    .unwrap_or_else(|| renderer.default_font()),
                                align_x: text::Alignment::Default,
                                align_y: alignment::Vertical::Center,
                                shaping: self.text_shaping,
                                wrapping: text::Wrapping::default(),
                            },
                            Point::new(
                                row_bounds.x + self.padding.left + label_x_offset,
                                row_bounds.center_y(),
                            ),
menu_style.label_color,
                            *viewport,
                        );
                    }

                    y += item_height;
                }
                MenuItem::Separator => {
                    if !state.mask[i] {
                        continue;
                    }

                    let sep_y = bounds.y + y + SEPARATOR_HEIGHT / 2.0;

                    if sep_y - 0.5 < viewport.y + viewport.height
                        && sep_y + 0.5 > viewport.y
                    {
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: Rectangle::new(
                                    Point::new(
                                        bounds.x + self.padding.left,
                                        sep_y - 0.5,
                                    ),
                                    Size::new(
                                        bounds.width - self.padding.x(),
                                        1.0,
                                    ),
                                ),
                                border: border::rounded(0.0),
                                ..renderer::Quad::default()
                            },
                            menu_style.separator_color,
                        );
                    }

                    y += SEPARATOR_HEIGHT;
                }
            }
        }

        if state.no_matches {
            let row_y = bounds.y + y;

            if row_y < viewport.y + viewport.height
                && row_y + item_height > viewport.y
            {
                renderer.fill_text(
                    Text {
                        content: "No matches".to_string(),
                        bounds: Size::new(f32::INFINITY, item_height),
                        size: text_size,
                        line_height: self.text_line_height,
                        font: self
                            .font
                            .unwrap_or_else(|| renderer.default_font()),
                        align_x: text::Alignment::Default,
                        align_y: alignment::Vertical::Center,
                        shaping: self.text_shaping,
                        wrapping: text::Wrapping::default(),
                    },
                    Point::new(
                        bounds.x + self.padding.left + label_x_offset,
                        row_y + item_height / 2.0,
                    ),
                    menu_style.label_color,
                    *viewport,
                );
            }
        }
    }
}

impl<'a, 'b, T, Message, Theme, Renderer>
    From<List<'a, 'b, T, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    T: ToString + Clone,
    Message: 'a,
    Theme: 'a + Catalog,
    Renderer: 'a + text::Renderer,
    'b: 'a,
{
    fn from(list: List<'a, 'b, T, Message, Theme, Renderer>) -> Self {
        Element::new(list)
    }
}

impl<T, Message, Theme, Renderer> List<'_, '_, T, Message, Theme, Renderer>
where
    T: ToString + Clone,
    Theme: Catalog,
    Renderer: text::Renderer,
{
    fn compute_mask(&self) -> Vec<bool> {
        let query = self.search.to_lowercase();

        let mut mask = vec![false; self.options.len()];
        let mut pending = false;

        for (i, entry) in self.options.iter().enumerate().rev() {
            match entry {
                MenuItem::Item(item) => {
                    if query.is_empty()
                        || item.label().to_lowercase().contains(&query)
                    {
                        mask[i] = true;
                        pending = true;
                    }
                }
                MenuItem::Label(_) | MenuItem::Separator => {
                    mask[i] = pending;
                    pending = false;
                }
            }
        }

        mask
    }

    fn total_height(
        &self,
        mask: &[bool],
        no_matches: bool,
        item_height: f32,
    ) -> f32 {
        let mut total = 0.0;

        for (i, entry) in self.options.iter().enumerate() {
            match entry {
                MenuItem::Item(_) => {
                    if mask[i] {
                        total += item_height;
                    }
                }
                MenuItem::Label(_) => {
                    if mask[i] {
                        total += item_height;
                    }
                }
                MenuItem::Separator => {
                    if mask[i] {
                        total += SEPARATOR_HEIGHT;
                    }
                }
            }
        }

        if no_matches {
            total += item_height;
        }

        total
    }

    fn item_at(
        &self,
        position: Point,
        bounds: Rectangle,
        mask: &[bool],
        text_size: Pixels,
    ) -> Option<usize> {
        if !bounds.contains(position) {
            return None;
        }

        let item_height =
            f32::from(self.text_line_height.to_absolute(text_size))
                + self.padding.y();

        let mut y = 0.0f32;

        for (i, entry) in self.options.iter().enumerate() {
            match entry {
                MenuItem::Item(_) => {
                    if mask[i] {
                        if position.y >= bounds.y + y
                            && position.y < bounds.y + y + item_height
                        {
                            return Some(i);
                        }

                        y += item_height;
                    }
                }
                MenuItem::Label(_) => {
                    if mask[i] {
                        y += item_height;
                    }
                }
                MenuItem::Separator => {
                    if mask[i] {
                        y += SEPARATOR_HEIGHT;
                    }
                }
            }
        }

        None
    }

    fn move_hover(&mut self, mask: &[bool], delta: isize) {
        let visible: Vec<usize> = mask
            .iter()
            .enumerate()
            .filter(|(_, visible)| **visible)
            .map(|(i, _)| i)
            .collect();

        if visible.is_empty() {
            return;
        }

        let new_index = match *self.hovered_option {
            Some(current) => visible
                .iter()
                .position(|&i| i == current)
                .map(|pos| {
                    let len = visible.len() as isize;
                    visible[((pos as isize + delta).rem_euclid(len)) as usize]
                })
                .unwrap_or_else(|| {
                    if delta > 0 {
                        visible[0]
                    } else {
                        *visible.last().unwrap()
                    }
                }),
            None => {
                if delta > 0 {
                    visible[0]
                } else {
                    *visible.last().unwrap()
                }
            }
        };

        *self.hovered_option = Some(new_index);
    }
}

/// The appearance of a [`Menu`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The [`Background`] of the menu.
    pub background: Background,
    /// The [`Border`] of the menu.
    pub border: Border,
    /// The text [`Color`] of the menu.
    pub text_color: Color,
    /// The text [`Color`] of a selected option in the menu.
    pub selected_text_color: Color,
    /// The background [`Color`] of a selected option in the menu.
    pub selected_background: Background,
    /// The text [`Color`] of the group labels in the menu.
    pub label_color: Color,
    /// The [`Color`] of the separator lines in the menu.
    pub separator_color: Color,
    /// The [`Border`] of the search box at the top of the menu.
    pub search_border: Border,
    /// The [`Shadow`] of the menu.
    pub shadow: Shadow,
}

/// The theme catalog of a [`Menu`].
pub trait Catalog: scrollable::Catalog + text_input::Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> <Self as Catalog>::Class<'a>;

    /// The default class for the scrollable of the [`Menu`].
    fn default_scrollable<'a>() -> <Self as scrollable::Catalog>::Class<'a> {
        <Self as scrollable::Catalog>::default()
    }

    /// The [`Style`] of a class with the given status.
    fn style(&self, class: &<Self as Catalog>::Class<'_>) -> Style;
}

/// A styling function for a [`Menu`].
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme) -> Style + 'a>;

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> StyleFn<'a, Self> {
        Box::new(default)
    }

    fn style(&self, class: &StyleFn<'_, Self>) -> Style {
        class(self)
    }
}

/// The default style of the list of a [`Menu`].
pub fn default(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        background: palette.background.weak.color.into(),
        border: Border {
            width: 1.0,
            radius: 0.0.into(),
            color: palette.background.strong.color,
        },
        text_color: palette.background.weak.text,
        selected_text_color: palette.primary.strong.text,
        selected_background: palette.primary.strong.color.into(),
        label_color: palette.background.weak.text.scale_alpha(0.5),
        separator_color: palette.background.strong.color,
        search_border: Border {
            width: 1.0,
            radius: 4.0.into(),
            color: palette.background.weak.text.scale_alpha(0.7),
        },
        shadow: Shadow::default(),
    }
}