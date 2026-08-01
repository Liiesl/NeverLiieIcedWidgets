use iced::advanced::layout::{self, Layout};
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::widget::Operation;
use iced::advanced::Shell;
use iced::mouse;
use iced::advanced::text::{self, Paragraph, Text};
use iced::{Event, Pixels, Point, Rectangle, Size, Vector};

use super::{
    Catalog, Menu, MenuItem, SHORTCUT_SPACING, ITEM_PADDING_X, ITEM_PADDING_Y,
    SEPARATOR_HEIGHT,
};

/// Shared state for the menu overlay, updated during event processing.
pub(crate) struct MenuState {
    pub hovered_item: Option<usize>,
    pub open_submenu_index: Option<usize>,
    pub submenu_state: Option<Box<MenuState>>,
    pub dismissed: bool,
    /// Cached text measurements of the currently displayed [`Menu`].
    ///
    /// Text shaping is expensive, so the menu contents are measured once and
    /// the result is reused until the contents change (detected by a cheap
    /// fingerprint comparison).
    measured: Option<MeasuredMenu>,
}

impl MenuState {
    pub fn new() -> Self {
        Self {
            hovered_item: None,
            open_submenu_index: None,
            submenu_state: None,
            dismissed: false,
            measured: None,
        }
    }
}

/// Cached measurements of a [`Menu`].
///
/// Storing the measured text widths avoids re-shaping every label with
/// `cosmic-text` on every frame. The results are only valid for the menu
/// contents they were computed from; [`MeasuredMenu::matches`] performs a
/// cheap fingerprint check against the current menu.
struct MeasuredMenu {
    /// Content fingerprint: for each entry in `menu.items`, the label and
    /// shortcut strings (separators are `None`).
    labels: Vec<Option<(String, Option<String>)>>,
    /// Total menu width, including padding and submenu arrow space.
    menu_width: f32,
    /// Total menu height.
    total_height: f32,
    /// Text size (bit pattern) the measurements were computed with.
    text_size: u32,
}

impl MeasuredMenu {
    /// Returns `true` if these measurements still match the given menu.
    fn matches<Message>(
        &self,
        menu: &Menu<'_, Message>,
        text_size: f32,
    ) -> bool {
        if self.text_size != text_size.to_bits() {
            return false;
        }

        if self.labels.len() != menu.items.len() {
            return false;
        }

        for (cached, item) in self.labels.iter().zip(&menu.items) {
            match (cached, item) {
                (Some((label, shortcut)), MenuItem::Item(item)) => {
                    if label != item.label
                        || shortcut.as_deref() != item.shortcut
                    {
                        return false;
                    }
                }
                (None, MenuItem::Separator) => {}
                _ => return false,
            }
        }

        true
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
    #[allow(clippy::too_many_arguments)]
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

    /// Ensures the measurements cached in `state.measured` match the given
    /// menu, re-measuring (and re-shaping) the text only when the menu
    /// contents have changed.
    fn ensure_measured(&mut self, renderer: &Renderer, menu: &Menu<'b, Message>) {
        let needs_measure = self
            .state
            .measured
            .as_ref()
            .is_none_or(|m| !m.matches(menu, self.text_size));

        if needs_measure {
            self.state.measured = Some(self.measure_menu(renderer, menu));
        }
    }

    fn measure_menu(
        &self,
        renderer: &Renderer,
        menu: &Menu<'b, Message>,
    ) -> MeasuredMenu {
        let ih = self.item_height();
        let sh = self.separator_height();

        let mut labels = Vec::with_capacity(menu.items.len());
        let mut max_width = 0.0f32;
        let mut total_height = 0.0f32;
        let mut has_submenus = false;

        for item in &menu.items {
            match item {
                MenuItem::Item(item) => {
                    let width =
                        self.measure_item_width(renderer, item.label, item.shortcut);
                    max_width = max_width.max(width);
                    total_height += ih;
                    has_submenus |= item.has_submenu();
                    labels.push(Some((
                        item.label.to_owned(),
                        item.shortcut.map(str::to_owned),
                    )));
                }
                MenuItem::Separator => {
                    total_height += sh;
                    labels.push(None);
                }
            }
        }

        // Add right padding for submenu arrows
        let submenu_arrow_space = if has_submenus { 20.0 } else { 0.0 };

        MeasuredMenu {
            labels,
            menu_width: max_width + submenu_arrow_space + ITEM_PADDING_X * 2.0,
            total_height,
            text_size: self.text_size.to_bits(),
        }
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

impl<'a, 'b, Message, Theme, Renderer> overlay::Overlay<Message, Theme, Renderer>
    for MenuOverlay<'a, 'b, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: Catalog + 'b,
    Renderer: renderer::Renderer + text::Renderer,
{
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> layout::Node {
        self.ensure_measured(renderer, self.menu);

        let measured = self.state.measured.as_ref().unwrap();
        let menu_size = Size::new(measured.menu_width, measured.total_height);

        let mut position = self.position;

        let max_x = (bounds.width - menu_size.width).max(0.0);
        let max_y = (bounds.height - menu_size.height).max(0.0);

        // Move the spawn anchor to the opposite corner when the menu
        // would be clipped by the right or bottom edge of the viewport.
        if position.x + menu_size.width > bounds.width {
            position.x = if position.x - menu_size.width >= 0.0 {
                position.x - menu_size.width
            } else {
                max_x
            };
        }
        if position.y + menu_size.height > bounds.height {
            position.y = if position.y - menu_size.height >= 0.0 {
                position.y - menu_size.height
            } else {
                max_y
            };
        }

        // Safety clamp (menu larger than viewport, or translation pushed
        // the position off the top/left edge).
        position.x = position.x.clamp(0.0, max_x);
        position.y = position.y.clamp(0.0, max_y);

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
                                // Toggle: clicking the parent item closes its
                                // submenu, if open. Only that submenu is
                                // dismissed — the root menu stays open.
                                if self.state.open_submenu_index == Some(idx) {
                                    self.state.open_submenu_index = None;
                                    self.state.submenu_state = None;
                                    shell.request_redraw();
                                } else {
                                    self.state.open_submenu_index = Some(idx);
                                    shell.request_redraw();
                                }
                                shell.capture_event();
                            } else if let Some(action) = item.action.clone() {
                                self.state.dismissed = true;
                                shell.publish(action);
                                shell.capture_event();
                                shell.request_redraw();
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
                        if let Some(idx) = self.state.hovered_item
                            && let Some(MenuItem::Item(item)) =
                                self.menu.items.get(idx)
                            && item.has_submenu()
                        {
                            self.state.open_submenu_index = Some(idx);
                            shell.request_redraw();
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
                        if let Some(idx) = self.state.hovered_item
                            && let Some(MenuItem::Item(item)) =
                                self.menu.items.get(idx)
                        {
                            if item.has_submenu() {
                                self.state.open_submenu_index = Some(idx);
                                shell.request_redraw();
                            } else if item.is_enabled()
                                && let Some(action) = item.action.clone()
                            {
                                self.state.dismissed = true;
                                shell.publish(action);
                                shell.capture_event();
                                shell.request_redraw();
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
        renderer: &Renderer,
    ) -> Option<overlay::Element<'c, Message, Theme, Renderer>> {
        if let Some(parent_idx) = self.state.open_submenu_index
            && let Some(sub_menu) = self.submenu(parent_idx)
        {
            let bounds = layout.bounds();
            let y_off = self.item_y_offset(parent_idx);

            // Ensure submenu state exists
            if self.state.submenu_state.is_none() {
                self.state.submenu_state = Some(Box::new(MenuState::new()));
            }

            // Reuse the submenu's cached measurements when its contents
            // have not changed.
            let needs_measure = self
                .state
                .submenu_state
                .as_ref()
                .and_then(|s| s.measured.as_ref())
                .is_none_or(|m| !m.matches(sub_menu, self.text_size));

            let submenu_width = if needs_measure {
                let measured = self.measure_menu(renderer, sub_menu);
                let width = measured.menu_width;
                self.state.submenu_state.as_mut().unwrap().measured =
                    Some(measured);
                width
            } else {
                self.state
                    .submenu_state
                    .as_ref()
                    .unwrap()
                    .measured
                    .as_ref()
                    .unwrap()
                    .menu_width
            };

            let submenu_x = if bounds.x + bounds.width + submenu_width > self.viewport.width {
                bounds.x - submenu_width
            } else {
                bounds.x + bounds.width
            };
            let submenu_y = bounds.y + y_off;

            let sub_state = self.state.submenu_state.as_mut().unwrap();
            let dismiss = self.dismiss_message.clone();
            let font = self.font;
            let text_size = self.text_size;
            let viewport = self.viewport;

            let sub_menu_ref: &'c Menu<'b, Message> = sub_menu;
            let state_ref: &'c mut MenuState = sub_state;
            let class_ref: &'c <Theme as Catalog>::Class<'b> = self.class;

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
