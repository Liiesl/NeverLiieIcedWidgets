use iced::advanced::layout::{self, Layout};
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::widget::Operation;
use iced::advanced::Shell;
use iced::mouse;
use iced::advanced::text::{self, Paragraph, Text};
use iced::{Event, Pixels, Point, Rectangle, Size, Vector};

use super::{
    Catalog, Menu, MenuItem, ITEM_PADDING_X, ITEM_PADDING_Y, SEPARATOR_HEIGHT,
};

/// Shared state for the menu overlay, updated during event processing.
pub(crate) struct MenuState {
    pub hovered_item: Option<usize>,
    pub open_submenu_index: Option<usize>,
    pub submenu_state: Option<Box<MenuState>>,
    pub dismissed: bool,
}

impl MenuState {
    pub fn new() -> Self {
        Self {
            hovered_item: None,
            open_submenu_index: None,
            submenu_state: None,
            dismissed: false,
        }
    }
}

/// Overlay that renders a context menu and handles interaction.
pub(crate) struct MenuOverlay<'a, 'b, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    menu: &'a Menu<'b, Message>,
    state: &'a mut MenuState,
    position: Point,
    viewport: Rectangle,
    class: &'a <Theme as Catalog>::Class<'b>,
    text_size: f32,
    dismiss_message: Message,
    font: Renderer::Font,
    is_root: bool,
}

impl<'a, 'b, Message, Theme, Renderer> MenuOverlay<'a, 'b, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    pub fn new(
        menu: &'a Menu<'b, Message>,
        state: &'a mut MenuState,
        position: Point,
        viewport: Rectangle,
        class: &'a <Theme as Catalog>::Class<'b>,
        text_size: Option<f32>,
        dismiss_message: Message,
        font: Renderer::Font,
        is_root: bool,
    ) -> Self {
        Self {
            menu,
            state,
            position,
            viewport,
            class,
            text_size: text_size.unwrap_or(13.0),
            dismiss_message,
            font,
            is_root,
        }
    }

    fn item_height(&self) -> f32 {
        let size = Pixels(self.text_size);
        let line_height = text::LineHeight::Absolute(Pixels(self.text_size * 1.4));
        f32::from(line_height.to_absolute(size)) + ITEM_PADDING_Y
    }

    fn separator_height(&self) -> f32 {
        SEPARATOR_HEIGHT
    }

    fn total_height(&self) -> f32 {
        let ih = self.item_height();
        let sh = self.separator_height();
        let mut total = 0.0;
        for item in &self.menu.items {
            match item {
                MenuItem::Item(_) => total += ih,
                MenuItem::Separator => total += sh,
            }
        }
        total
    }

    fn measure_item_width(&self, _renderer: &Renderer, label: &str, shortcut: Option<&str>) -> f32 {
        let size = Pixels(self.text_size);
        let line_height = text::LineHeight::Absolute(Pixels(self.text_size * 1.4));

        let label_width = Renderer::Paragraph::with_text(Text {
            content: label,
            bounds: Size::new(f32::INFINITY, f32::INFINITY),
            size,
            line_height,
            font: self.font,
            align_x: text::Alignment::Left,
            align_y: iced::alignment::Vertical::Top,
            shaping: text::Shaping::Basic,
            wrapping: text::Wrapping::None,
        })
        .min_width();

        let shortcut_width = shortcut
            .map(|s| {
                Renderer::Paragraph::with_text(Text {
                    content: s,
                    bounds: Size::new(f32::INFINITY, f32::INFINITY),
                    size: Pixels(self.text_size * 0.9),
                    line_height: text::LineHeight::Absolute(Pixels(self.text_size * 1.2)),
                    font: self.font,
                    align_x: text::Alignment::Left,
                    align_y: iced::alignment::Vertical::Top,
                    shaping: text::Shaping::Basic,
                    wrapping: text::Wrapping::None,
                })
                .min_width()
            })
            .unwrap_or(0.0);

        label_width + shortcut_width + SHORTCUT_SPACING
    }

    fn menu_width(&self, renderer: &Renderer) -> f32 {
        let mut max_width = 0.0f32;

        for item in &self.menu.items {
            if let MenuItem::Item(item) = item {
                let w = self.measure_item_width(renderer, item.label, item.shortcut);
                if w > max_width {
                    max_width = w;
                }
            }
        }

        // Add right padding for submenu arrows
        let has_submenus = self.menu.items.iter().any(|m| {
            matches!(m, MenuItem::Item(item) if item.has_submenu())
        });
        let submenu_arrow_space = if has_submenus { 20.0 } else { 0.0 };

        max_width + submenu_arrow_space + ITEM_PADDING_X * 2.0
    }

    fn item_at(&self, position: Point, bounds: Rectangle) -> Option<(usize, bool)> {
        if !bounds.contains(position) {
            return None;
        }
        let ih = self.item_height();
        let sh = self.separator_height();
        let mut y = 0.0f32;

        for (i, item) in self.menu.items.iter().enumerate() {
            match item {
                MenuItem::Item(menu_item) => {
                    if position.y >= bounds.y + y
                        && position.y < bounds.y + y + ih
                    {
                        return Some((i, menu_item.is_enabled()));
                    }
                    y += ih;
                }
                MenuItem::Separator => {
                    y += sh;
                }
            }
        }
        None
    }

    fn visible_item_indices(&self) -> Vec<usize> {
        self.menu
            .items
            .iter()
            .enumerate()
            .filter(|(_, m)| matches!(m, MenuItem::Item(_)))
            .map(|(i, _)| i)
            .collect()
    }

    fn item_y_offset(&self, target_idx: usize) -> f32 {
        let ih = self.item_height();
        let sh = self.separator_height();
        let mut y = 0.0f32;

        for (i, item) in self.menu.items.iter().enumerate() {
            if i == target_idx {
                return y;
            }
            match item {
                MenuItem::Item(_) => y += ih,
                MenuItem::Separator => y += sh,
            }
        }
        y
    }

    fn submenu(&self, parent_idx: usize) -> Option<&'a Menu<'b, Message>> {
        if let Some(MenuItem::Item(item)) = self.menu.items.get(parent_idx) {
            item.submenu.as_ref()
        } else {
            None
        }
    }
}

use super::SHORTCUT_SPACING;

impl<'a, 'b, Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer>
    for MenuOverlay<'a, 'b, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: Catalog + 'b,
    Renderer: renderer::Renderer + text::Renderer,
{
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> layout::Node {
        let menu_size = Size::new(self.menu_width(renderer), self.total_height());

        let mut position = self.position;

        // Clamp to viewport
        if position.x + menu_size.width > bounds.width {
            position.x = bounds.width - menu_size.width;
        }
        if position.y + menu_size.height > bounds.height {
            position.y = bounds.height - menu_size.height;
        }
        if position.x < 0.0 {
            position.x = 0.0;
        }
        if position.y < 0.0 {
            position.y = 0.0;
        }

        layout::Node::new(menu_size)
            .translate(Vector::new(position.x, position.y))
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
    ) {
        let style = Theme::style(theme, self.class);
        let bounds = layout.bounds();
        let ih = self.item_height();
        let sh = self.separator_height();
        let size = Pixels(self.text_size);

        // Draw background
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: style.border,
                shadow: style.shadow,
                ..renderer::Quad::default()
            },
            style.background,
        );

        // Draw items
        let mut y = 0.0f32;
        let hovered = self.state.hovered_item;

        for (full_idx, item) in self.menu.items.iter().enumerate() {
            match item {
                MenuItem::Item(item) => {
                    let item_bounds = Rectangle {
                        x: bounds.x,
                        y: bounds.y + y,
                        width: bounds.width,
                        height: ih,
                    };

                    let is_hovered = hovered == Some(full_idx);

                    // Draw hover highlight
                    if is_hovered {
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: Rectangle {
                                    x: item_bounds.x + style.border.width,
                                    width: item_bounds.width
                                        - style.border.width * 2.0,
                                    ..item_bounds
                                },
                                border: iced::Border::default().rounded(
                                    style.border.radius,
                                ),
                                ..renderer::Quad::default()
                            },
                            style.selected_background,
                        );
                    }

                    let text_color = if item.is_enabled() {
                        if is_hovered {
                            style.selected_text_color
                        } else {
                            style.text_color
                        }
                    } else {
                        style.disabled_text_color
                    };

                    // Draw label
                    renderer.fill_text(
                        Text {
                            content: item.label.to_string(),
                            bounds: Size::new(f32::INFINITY, ih),
                            size,
                            line_height: text::LineHeight::Absolute(
                                Pixels(self.text_size * 1.4),
                            ),
                            font: self.font,
                            align_x: text::Alignment::Default,
                            align_y: iced::alignment::Vertical::Center,
                            shaping: text::Shaping::Basic,
                            wrapping: text::Wrapping::None,
                        },
                        Point::new(
                            item_bounds.x + ITEM_PADDING_X,
                            item_bounds.center_y(),
                        ),
                        text_color,
                        bounds,
                    );

                    // Draw shortcut
                    if let Some(shortcut) = item.shortcut {
                        renderer.fill_text(
                            Text {
                                content: shortcut.to_string(),
                                bounds: Size::new(f32::INFINITY, ih),
                                size: Pixels(self.text_size * 0.9),
                                line_height: text::LineHeight::Absolute(
                                    Pixels(self.text_size * 1.2),
                                ),
                                font: self.font,
                                align_x: text::Alignment::Right,
                                align_y: iced::alignment::Vertical::Center,
                                shaping: text::Shaping::Basic,
                                wrapping: text::Wrapping::None,
                            },
                            Point::new(
                                item_bounds.x + item_bounds.width
                                    - ITEM_PADDING_X,
                                item_bounds.center_y(),
                            ),
                            style.shortcut_text_color,
                            bounds,
                        );
                    }

                    // Draw submenu arrow indicator (right-pointing triangle)
                    if item.has_submenu() {
                        let arrow_x =
                            item_bounds.x + item_bounds.width - ITEM_PADDING_X;
                        let arrow_y = item_bounds.center_y();

                        // Draw a small right-pointing triangle
                        let arrow_color = if is_hovered {
                            style.selected_text_color
                        } else {
                            style.text_color
                        };
                        // Use a simple quad as the arrow indicator
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: Rectangle::new(
                                    Point::new(arrow_x - 2.0, arrow_y - 4.0),
                                    Size::new(6.0, 8.0),
                                ),
                                border: iced::Border::default(),
                                ..renderer::Quad::default()
                            },
                            arrow_color,
                        );
                    }

                    y += ih;
                }
                MenuItem::Separator => {
                    let sep_y = bounds.y + y + sh / 2.0;
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: Rectangle::new(
                                Point::new(
                                    bounds.x + ITEM_PADDING_X,
                                    sep_y - 0.5,
                                ),
                                Size::new(
                                    bounds.width - ITEM_PADDING_X * 2.0,
                                    1.0,
                                ),
                            ),
                            border: iced::Border::default(),
                            ..renderer::Quad::default()
                        },
                        style.separator_color,
                    );
                    y += sh;
                }
            }
        }
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        let bounds = layout.bounds();

        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some(pos) = cursor.position() {
                    if let Some((idx, _enabled)) = self.item_at(pos, bounds) {
                        if self.state.hovered_item != Some(idx) {
                            self.state.hovered_item = Some(idx);

                            // If hovering over a submenu parent, open its submenu
                            if let Some(MenuItem::Item(item)) =
                                self.menu.items.get(idx)
                            {
                                if item.has_submenu() {
                                    self.state.open_submenu_index = Some(idx);
                                } else {
                                    self.state.open_submenu_index = None;
                                }
                            }

                            shell.request_redraw();
                        }
                    } else if self.state.hovered_item.is_some() {
                        self.state.hovered_item = None;
                        shell.request_redraw();
                    }
                } else if self.state.hovered_item.is_some() {
                    self.state.hovered_item = None;
                    shell.request_redraw();
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position() {
                    if let Some((idx, _enabled)) = self.item_at(pos, bounds) {
                        if let Some(MenuItem::Item(item)) =
                            self.menu.items.get(idx)
                        {
                            if item.has_submenu() {
                                // Open submenu (no toggle — closing happens
                                // when hovering a different item or outside)
                                if self.state.open_submenu_index != Some(idx) {
                                    self.state.open_submenu_index = Some(idx);
                                    shell.request_redraw();
                                }
                                shell.capture_event();
                            } else if let Some(action) = item.action.clone() {
                                shell.publish(action);
                                shell.capture_event();
                            } else {
                                // Disabled item — consume the click
                                shell.capture_event();
                            }
                        }
                    } else {
                        // Click outside menu or on padding
                        if self.is_root {
                            // Root menu: dismiss but don't capture, so the
                            // click propagates to widgets underneath.
                            self.state.dismissed = true;
                            shell.publish(self.dismiss_message.clone());
                        }
                    }
                } else {
                    // Cursor off screen — dismiss
                    self.state.dismissed = true;
                    shell.publish(self.dismiss_message.clone());
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Right,
            )) => {
                self.state.dismissed = true;
                shell.publish(self.dismiss_message.clone());
                // Don't capture — let the manager see the right-click
                // so it can open a new context menu.
            }
            Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) => {
                match key.as_ref() {
                    iced::keyboard::Key::Named(
                        iced::keyboard::key::Named::Escape,
                    ) => {
                        self.state.dismissed = true;
                        shell.publish(self.dismiss_message.clone());
                        shell.capture_event();
                    }
                    iced::keyboard::Key::Named(
                        iced::keyboard::key::Named::ArrowUp,
                    ) => {
                        self.move_hover_up();
                        shell.request_redraw();
                    }
                    iced::keyboard::Key::Named(
                        iced::keyboard::key::Named::ArrowDown,
                    ) => {
                        self.move_hover_down();
                        shell.request_redraw();
                    }
                    iced::keyboard::Key::Named(
        iced::keyboard::key::Named::ArrowRight,
    ) => {
                        // Open submenu of hovered item
                        if let Some(idx) = self.state.hovered_item {
                            if let Some(MenuItem::Item(item)) =
                                self.menu.items.get(idx)
                            {
                                if item.has_submenu() {
                                    self.state.open_submenu_index = Some(idx);
                                    shell.request_redraw();
                                }
                            }
                        }
                    }
                    iced::keyboard::Key::Named(
        iced::keyboard::key::Named::ArrowLeft,
    ) => {
                        // Close submenu
                        if self.state.open_submenu_index.is_some() {
                            self.state.open_submenu_index = None;
                            shell.request_redraw();
                        }
                    }
                    iced::keyboard::Key::Named(
                        iced::keyboard::key::Named::Enter,
                    ) => {
                        if let Some(idx) = self.state.hovered_item {
                            if let Some(MenuItem::Item(item)) =
                                self.menu.items.get(idx)
                            {
                                if item.has_submenu() {
                                    self.state.open_submenu_index = Some(idx);
                                    shell.request_redraw();
                                } else if item.is_enabled() {
                                    if let Some(action) = item.action.clone()
                                    {
                                        shell.publish(action);
                                        shell.capture_event();
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }

    fn operate(
        &mut self,
        _layout: Layout<'_>,
        _renderer: &Renderer,
        _operation: &mut dyn Operation,
    ) {
    }

    fn overlay<'c>(
        &'c mut self,
        layout: Layout<'c>,
        _renderer: &Renderer,
    ) -> Option<overlay::Element<'c, Message, Theme, Renderer>> {
        if let Some(parent_idx) = self.state.open_submenu_index {
            if let Some(sub_menu) = self.submenu(parent_idx) {
                let bounds = layout.bounds();
                let y_off = self.item_y_offset(parent_idx);

                let submenu_x = bounds.x + bounds.width;
                let submenu_y = bounds.y + y_off;

                // Ensure submenu state exists
                if self.state.submenu_state.is_none() {
                    self.state.submenu_state = Some(Box::new(MenuState::new()));
                }

                let sub_state = self.state.submenu_state.as_mut().unwrap();
                let dismiss = self.dismiss_message.clone();
                let font = self.font;
                let text_size = self.text_size;
                let viewport = self.viewport;

                let sub_menu_ref: &'c Menu<'b, Message> = &*sub_menu;
                let state_ref: &'c mut MenuState = sub_state;
                let class_ref: &'c <Theme as Catalog>::Class<'b> = &*self.class;

                return Some(overlay::Element::new(Box::new(
                    MenuOverlay {
                        menu: sub_menu_ref,
                        state: state_ref,
                        position: Point::new(submenu_x, submenu_y),
                        viewport,
                        class: class_ref,
                        text_size,
                        dismiss_message: dismiss,
                        font,
                        is_root: false,
                    },
                )));
            }
        }

        None
    }

    fn index(&self) -> f32 {
        100.0
    }
}

impl<'a, 'b, Message, Theme, Renderer> MenuOverlay<'a, 'b, Message, Theme, Renderer>
where
    Theme: Catalog + 'b,
    Renderer: text::Renderer,
{
    fn move_hover_up(&mut self) {
        let visible = self.visible_item_indices();
        if visible.is_empty() {
            return;
        }

        let new_idx = match self.state.hovered_item {
            Some(current) => {
                if let Some(pos) = visible.iter().position(|&i| i == current) {
                    if pos == 0 {
                        *visible.last().unwrap()
                    } else {
                        visible[pos - 1]
                    }
                } else {
                    *visible.last().unwrap()
                }
            }
            None => *visible.last().unwrap(),
        };

        self.state.hovered_item = Some(new_idx);

        // If new item has a submenu, open it; otherwise close any open submenu
        if let Some(MenuItem::Item(item)) = self.menu.items.get(new_idx) {
            if item.has_submenu() {
                self.state.open_submenu_index = Some(new_idx);
            } else {
                self.state.open_submenu_index = None;
            }
        }
    }

    fn move_hover_down(&mut self) {
        let visible = self.visible_item_indices();
        if visible.is_empty() {
            return;
        }

        let new_idx = match self.state.hovered_item {
            Some(current) => {
                if let Some(pos) = visible.iter().position(|&i| i == current) {
                    if pos == visible.len() - 1 {
                        visible[0]
                    } else {
                        visible[pos + 1]
                    }
                } else {
                    visible[0]
                }
            }
            None => visible[0],
        };

        self.state.hovered_item = Some(new_idx);

        if let Some(MenuItem::Item(item)) = self.menu.items.get(new_idx) {
            if item.has_submenu() {
                self.state.open_submenu_index = Some(new_idx);
            } else {
                self.state.open_submenu_index = None;
            }
        }
    }
}
