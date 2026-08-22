//! A single example that exercises every feature of the `title_bar` widget on
//! Windows, X11, Wayland and macOS without editing a line of source.
//!
//! ```text
//! cargo run -p title-bar-test -- --decorations custom
//! cargo run -p title-bar-test -- --decorations system
//! ```
//!
//! The mode can also come from the environment, which is handy when the
//! command line belongs to a launcher:
//!
//! ```text
//! TITLE_BAR_TEST_DECORATIONS=system cargo run -p title-bar-test
//! ```
//!
//! It demonstrates:
//!
//! 1. custom decorations
//! 2. system decorations
//! 3. title-bar menus and buttons
//! 4. a fill spacer that still drags the window
//! 5. a clickable application icon with an application-owned fallback menu
//! 6. all eight resize directions
//! 7. switching between the built-in Iced themes

use iced::widget::{button, column, container, pick_list, row, rule, scrollable, space, text};
use iced::{Center, Element, Fill, Subscription, Task, Theme, window};

use neverliie_iced_widgets::title_bar::{
    DecorationMode, FrameAction, NativeFrame, NativeFrameConfig,
};

const APP_ICON: &[u8] = include_bytes!("../assets/app-icon.svg");

fn main() -> iced::Result {
    let mode = match decoration_mode_from_args() {
        Ok(mode) => mode,
        Err(message) => {
            eprintln!("{message}");
            eprintln!("usage: title-bar-test [--decorations custom|system]");

            std::process::exit(2);
        }
    };

    let frame = NativeFrame::new(NativeFrameConfig::platform_default().decoration_mode(mode));

    // The one helper that replaces every platform `#[cfg]` an application
    // would otherwise need.
    let settings = frame.window_settings(window::Settings {
        size: iced::Size::new(940.0, 620.0),
        min_size: Some(iced::Size::new(480.0, 320.0)),
        resizable: true,
        ..window::Settings::default()
    });

    iced::application(move || App::new(frame.clone()), App::update, App::view)
        .window(settings)
        .title(App::title)
        .theme(App::theme)
        .subscription(App::subscription)
        .run()
}

fn decoration_mode_from_args() -> Result<DecorationMode, String> {
    let mut args = std::env::args().skip(1);
    let mut raw = std::env::var("TITLE_BAR_TEST_DECORATIONS").ok();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--decorations" => {
                raw = Some(
                    args.next()
                        .ok_or_else(|| "--decorations needs a value".to_owned())?,
                );
            }

            other => {
                if let Some(value) = other.strip_prefix("--decorations=") {
                    raw = Some(value.to_owned());
                } else {
                    return Err(format!("unknown argument: {other}"));
                }
            }
        }
    }

    match raw.as_deref() {
        None => Ok(DecorationMode::Custom),
        Some("custom") => Ok(DecorationMode::Custom),
        Some("system") => Ok(DecorationMode::System),
        Some(other) => Err(format!("unknown decoration mode: {other}")),
    }
}

struct App {
    frame: NativeFrame,
    window_id: Option<window::Id>,
    theme: Theme,
    install_error: Option<String>,
    icon_menu_open: bool,
    log: Vec<String>,
}

#[derive(Debug, Clone)]
enum Message {
    WindowFound(Option<window::Id>),
    FrameInstalled(Result<(), String>),
    Frame(FrameAction),

    ThemeSelected(Theme),
    MenuPressed(&'static str),
    IconMenuDismissed,
}

impl App {
    fn new(frame: NativeFrame) -> (Self, Task<Message>) {
        (
            Self {
                frame,
                window_id: None,
                theme: Theme::GruvboxDark,
                install_error: None,
                icon_menu_open: false,
                log: Vec::new(),
            },
            window::latest().map(Message::WindowFound),
        )
    }

    fn title(&self) -> String {
        String::from("NeverLie Title Bar")
    }

    fn subscription(&self) -> Subscription<Message> {
        self.frame.subscription().map(Message::Frame)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::WindowFound(Some(window_id)) => {
                self.window_id = Some(window_id);

                self.frame.install(window_id).map(Message::FrameInstalled)
            }

            Message::WindowFound(None) => Task::none(),

            Message::FrameInstalled(Ok(())) => Task::none(),

            Message::FrameInstalled(Err(error)) => {
                eprintln!("Title bar installation failed: {error}");
                self.install_error = Some(error);

                Task::none()
            }

            // The application owns the icon click. Where the platform has a
            // real system menu we hand it to the default helper; where it does
            // not — X11, macOS — we show our own popup instead.
            Message::Frame(FrameAction::WindowIconPressed(id)) => {
                if self.frame.supports_system_menu() {
                    self.frame
                        .update(FrameAction::WindowIconPressed(id), Message::Frame)
                } else {
                    self.icon_menu_open = !self.icon_menu_open;

                    Task::none()
                }
            }

            Message::Frame(action) => {
                self.icon_menu_open = false;

                self.frame.update(action, Message::Frame)
            }

            Message::ThemeSelected(theme) => {
                self.theme = theme;

                Task::none()
            }

            Message::MenuPressed(name) => {
                self.icon_menu_open = false;
                self.log.push(format!("{name} pressed"));
                self.log.truncate(12);

                Task::none()
            }

            Message::IconMenuDismissed => {
                self.icon_menu_open = false;

                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let body = self.body();

        let Some(window_id) = self.window_id else {
            return body;
        };

        let icon: Element<'_, Message> =
            iced::widget::svg(iced::widget::svg::Handle::from_memory(APP_ICON))
                .width(Fill)
                .height(Fill)
                .into();

        self.frame.view(
            window_id,
            "NeverLie Title Bar",
            Some(icon),
            Some(self.title_content()),
            body,
            Message::Frame,
        )
    }

    /// The application-owned title-bar row.
    ///
    /// `space::horizontal()` between the menus and the trailing button does
    /// not capture input, so dragging it moves the window — while every
    /// button around it stays clickable.
    fn title_content(&self) -> Element<'_, Message> {
        row![
            menu_button("File"),
            menu_button("Edit"),
            menu_button("View"),
            space::horizontal(),
            menu_button("Settings"),
        ]
        .spacing(2)
        .align_y(Center)
        .width(Fill)
        .into()
    }

    fn body(&self) -> Element<'_, Message> {
        let heading = text("neverliie title_bar").size(28);

        let mode = match self.frame.decoration_mode() {
            DecorationMode::Custom => "Custom",
            DecorationMode::System => "System",
        };

        let facts = column![
            fact("Decoration mode", mode.to_owned()),
            fact(
                "Maximized",
                self.window_id
                    .map(|id| self.frame.is_maximized(id).to_string())
                    .unwrap_or_else(|| "unknown".to_owned()),
            ),
            fact("Tracked windows", self.frame.tracked_windows().to_string()),
            fact(
                "window::show_system_menu",
                if self.frame.supports_system_menu() {
                    "supported".to_owned()
                } else {
                    "no-op on this platform".to_owned()
                },
            ),
            fact(
                "Title bar height",
                format!("{}", self.frame.config().title_bar_height)
            ),
            fact(
                "Resize border",
                format!("{}", self.frame.config().resize_border)
            ),
        ]
        .spacing(4);

        let theme_picker = row![
            text("Theme").width(150),
            pick_list(Theme::ALL, Some(&self.theme), Message::ThemeSelected),
        ]
        .spacing(12)
        .align_y(Center);

        let instructions = column![
            text("Try it").size(18),
            text("• Drag the empty space between the menus to move the window."),
            text("• Double-click that space to maximize and restore."),
            text("• Right-click it to request the system menu."),
            text("• Click the application icon in the corner."),
            text("• Drag any of the eight edges and corners to resize."),
            text("• Switch themes; the title bar follows the palette."),
        ]
        .spacing(4);

        let log: Element<'_, Message> = if self.log.is_empty() {
            text("No title-bar activity yet.").into()
        } else {
            column(self.log.iter().map(|entry| text(entry).into()))
                .spacing(2)
                .into()
        };

        let mut content = column![heading];

        if let Some(error) = &self.install_error {
            content = content.push(text(format!("Frame install failed: {error}")));
        }

        if self.icon_menu_open {
            content = content.push(self.icon_menu());
        }

        content = content
            .push(facts)
            .push(rule::horizontal(1))
            .push(theme_picker)
            .push(rule::horizontal(1))
            .push(instructions)
            .push(rule::horizontal(1))
            .push(log);

        scrollable(container(content.spacing(20)).padding(24).width(Fill))
            .height(Fill)
            .into()
    }

    /// The application-owned stand-in for a system menu, used where
    /// `window::show_system_menu` does nothing.
    fn icon_menu(&self) -> Element<'_, Message> {
        let Some(window_id) = self.window_id else {
            return space::horizontal().into();
        };

        container(
            column![
                text("Application menu").size(14),
                button(text("Minimize"))
                    .style(button::text)
                    .on_press(Message::Frame(FrameAction::Minimize(window_id))),
                button(text("Maximize / Restore"))
                    .style(button::text)
                    .on_press(Message::Frame(FrameAction::ToggleMaximize(window_id))),
                button(text("Close"))
                    .style(button::text)
                    .on_press(Message::Frame(FrameAction::Close(window_id))),
                button(text("Dismiss"))
                    .style(button::text)
                    .on_press(Message::IconMenuDismissed),
            ]
            .spacing(2),
        )
        .padding(12)
        .style(container::bordered_box)
        .into()
    }

    fn theme(&self) -> Theme {
        self.theme.clone()
    }
}

fn menu_button(label: &'static str) -> Element<'static, Message> {
    button(text(label).size(12.0))
        .on_press(Message::MenuPressed(label))
        .style(button::text)
        .padding([3, 7])
        .into()
}

fn fact(label: &'static str, value: String) -> Element<'static, Message> {
    row![text(label).width(150), text(value)].spacing(12).into()
}
