# Title Bar Widget

A cross-platform, theme-aware custom window frame. Ported from the [`iced-window-decorations`](https://github.com/A-Disruption/iced-window-decorations) crate (`iced-native-frame`) and retargeted from iced master to iced 0.14.

`NativeFrame` draws a title bar out of ordinary Iced widgets and wires it to the platform through iced's winit-backed `iced::window` API. The only native code is a small `WM_NCHITTEST` subclass on Windows, which is what the Windows 11 Snap Layouts flyout requires and what winit does not expose.

## Overview

| Type | Purpose |
|------|---------|
| `NativeFrame` | The frame facade; wraps application content, owns per-window state, exposes `subscription`/`update`/`view` |
| `NativeFrameConfig` | Look-and-feel configuration (heights, buttons, borders, shadows); builder pattern |
| `DecorationMode` | `Custom` (application draws the frame) or `System` (platform decorations) |
| `FrameAction` | Everything the frame asks the app to forward back into `NativeFrame::update` |
| `CaptionControl` | One of the three semantic caption controls: `Minimize`, `Maximize`, `Close` |

A single `NativeFrame` can drive any number of windows; every clone shares the same per-window state.

## Platform Behavior

| Capability | Windows | Linux (X11/Wayland) | macOS |
|------------|---------|---------------------|-------|
| Custom title bar | yes | yes | hybrid (native traffic lights kept) |
| Caption buttons | drawn, hit-tested natively | drawn + `mouse_area` input | not drawn (`leading_inset` reserves room) |
| Snap Layouts flyout | yes (`HTMAXBUTTON`) | — | — |
| Resize | native `WM_NCHITTEST`, 8 directions + cursors | invisible overlay handles, 8 directions | native decorations |
| System menu | yes | Wayland yes; X11 no-op | no |
| Window rounding | DWM via `corner_preference` | client-side radius | native |
| Border | 1px accent color while active, neutral otherwise | neutral theme color | none |

## Quick Start (single window)

Three touchpoints and no `window::Id` anywhere in your state:

```rust
use iced::{Element, Subscription, Task, window};
use neverliie_iced_widgets::title_bar::{DecorationMode, FrameAction, NativeFrame, NativeFrameConfig};

fn main() -> iced::Result {
    let frame = NativeFrame::new(
        NativeFrameConfig::platform_default()
            .decoration_mode(DecorationMode::Custom),
    );

    // 1. Let the frame apply its platform requirements to the window settings,
    //    and install itself on the window Iced opens for us.
    let settings = frame.window_settings(window::Settings {
        resizable: true,
        ..window::Settings::default()
    });

    iced::application(
        {
            let frame = frame.clone();
            move || {
                (
                    App { frame, value: 0 },
                    frame.install_latest().discard(),
                )
            }
        },
        App::update,
        App::view,
    )
    .window(settings)
    .subscription(App::subscription)
    .run()
}

struct App {
    frame: NativeFrame,
    value: i32,
}

#[derive(Debug, Clone)]
enum Message {
    Frame(FrameAction),
    Increment,
}

impl App {
    // 2. Forward the frame's window events so it can track maximization,
    //    focus and closed windows — then hand every action back to it.
    fn subscription(&self) -> Subscription<Message> {
        self.frame.subscription().map(Message::Frame)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Frame(action) => self.frame.update(action, Message::Frame),
            Message::Increment => {
                self.value += 1;
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let content = iced::widget::column![
            iced::widget::text(self.value).size(48),
            iced::widget::button("Increment").on_press(Message::Increment),
        ];

        // 3. Wrap the application content in the frame.
        self.frame.decorate("My App", content, Message::Frame)
    }
}
```

Until `install_latest` resolves (in practice the first frame only), `decorate` returns the content undecorated.

## Multiple Windows

Open windows by hand with `window::create`, then pair `install` with `view` per `window::Id`:

```rust
self.frame.install(window_id).discard();          // once, when the window opens
let _ = self.frame.uninstall(window_id);          // optionally, when it closes

let element = self.frame.view(
    window_id,
    "Window title",
    Some(icon_element),          // optional clickable application icon
    Some(title_content_element), // optional application-owned title-bar row
    body_element,
    Message::Frame,
);
```

`frame.primary_window()` returns the first installed id for `decorate`-style use.

## Title-Bar Content

The bar layout is:

```text
[leading inset][icon][title][application-owned fill row][caption controls]
```

- `title_content` receives all width between the title and the caption controls and may contain anything interactive (menus, buttons). Children that capture a click keep it; anything else falls through to the draggable region behind it.
- Dragging, double-click-to-maximize and right-click-for-system-menu work on every non-capturing pixel of the bar.
- A click on the optional icon publishes `FrameAction::WindowIconPressed`. Forwarding it to `NativeFrame::update` shows the system menu where the platform supports it; intercept the action instead to open an application menu (the fallback pattern used on X11/macOS).

## Configuration

Start from `NativeFrameConfig::platform_default()` and override what you need:

```rust
NativeFrameConfig::platform_default()
    .title_bar_height(34.0)      // logical pixels
    .caption_button_width(44.0)
    .caption_buttons(true)
    .resizable(true)
    .resize_border(6.0)
    .corner_radius(8.0)          // client-side rounding (Linux)
    .frame_border(true)          // 1px border; accent-colored while active on Windows
    .client_shadow(false)        // opt-in; sets outer_padding(12) automatically
    .native_rounding(true)       // Windows 11 DWM rounding
    .native_shadow(false)        // winit undecorated_shadow
    .window_icon_size(16.0)
    .title_spacing(8.0)
    .title_padding(8.0)
    .leading_inset(0.0)          // macOS traffic lights get 78.0
    .show_title(true)
```

Platform defaults differ: Windows uses a 32px bar, 46px buttons, native rounding; macOS uses a 28px bar, no caption buttons, no border, and reserves the traffic-light inset; Linux disables the client-side shadow because transparent padding leaks into tiled/maximized geometry on most Wayland compositors.

## Frame Actions

Every variant names its window, so one handler serves them all:

| Action | Default handling in `NativeFrame::update` |
|--------|-------------------------------------------|
| `Drag` / `Resize` | `window::drag` / `window::drag_resize` |
| `Minimize` / `ToggleMaximize` / `Close` | corresponding `window` task |
| `WindowIconPressed` / `ShowSystemMenu` | `window::show_system_menu` where supported |
| `Hover` / `Leave` | internal caption-button hover/press tracking |
| `MaximizedChanged` / `FocusChanged` / `SyncState` | mirror platform state into the frame |
| `WindowClosed` | drops the per-window state |

`FrameAction::Resize` re-exports `iced::window::Direction`.

## Windows Details

The `WM_NCHITTEST` subclass answers the one message Windows reads caption semantics from:

* the three caption regions return `HTMINBUTTON` / `HTMAXBUTTON` / `HTCLOSE` — `HTMAXBUTTON` is what makes the Snap Layouts flyout appear over the maximize button
* the eight resize edges return native hit results, giving real resize cursors and edge-snapping
* ordinary title-bar pixels stay `HTCLIENT`, so your own widgets keep receiving input
* maximized/focused state is mirrored authoritatively from `WM_SIZE`/`WM_NCACTIVATE`, covering Aero Snap and <kbd>Win</kbd>+<kbd>Up</kbd>
* the 1px surface border takes the user's accent color (read from the same DWM registry value the system uses for native windows) while the window is active, and falls back to a neutral theme color when inactive or maximized

Rounded corners and the optional undecorated drop shadow are requested through `window::Settings` (`corner_preference`, `undecorated_shadow`); the module makes no DWM calls of its own.

## Styling

All styling derives from the active iced `Theme` extended palette, so the frame follows whatever theme the application selects:

- title bar background: `background.weakest`
- surface background/border: `background.base` / `background.strong`
- caption hover: `background.weak`, pressed: `background.strong`
- close control keeps the conventional red highlight (`#C42B1C` hover, `#961414` press) regardless of palette
- inactive windows dim caption glyphs to `background.weak.text`

Caption glyphs are embedded Lucide SVGs compiled into the binary; there is no asset dependency.

## Demo

```sh
cargo run -p title-bar-test -- --decorations custom   # default
cargo run -p title-bar-test -- --decorations system   # platform decorations
TITLE_BAR_TEST_DECORATIONS=system cargo run -p title-bar-test
```

The demo exercises menus in the bar, a draggable fill spacer, the icon-click fallback menu, all eight resize directions and live theme switching.

## API Reference

### `NativeFrame`

```rust
NativeFrame::new(config)                            // or ::default()
    .config() -> NativeFrameConfig
    .decoration_mode() -> DecorationMode
    .supports_system_menu() -> bool
    .window_settings(window::Settings) -> window::Settings
    .install(window_id) -> Task<Result<(), String>>
    .install_latest() -> Task<Result<(), String>>
    .uninstall(window_id) -> Task<()>
    .subscription() -> Subscription<FrameAction>
    .update(action, map_action) -> Task<Message>     // map_action: Fn(FrameAction) -> Message
    .view(window_id, title, icon, title_content, content, map_action) -> Element
    .decorate(title, content, map_action) -> Element
    .primary_window() -> Option<window::Id>
    .is_maximized(window_id) -> bool
    .is_active(window_id) -> bool
    .tracked_windows() -> usize
```

### `NativeFrameConfig`

Builder setters mirror the struct's public fields one-to-one (`title_bar_height`, `caption_button_width`, `caption_buttons`, `resize_border`, `resizable`, `corner_radius`, `frame_border`, `client_shadow`, `outer_padding`, `native_rounding`, `native_shadow`, `window_icon_size`, `title_spacing`, `title_padding`, `leading_inset`, `show_title`), plus `decoration_mode(mode)`.

### `DecorationMode`

```rust
DecorationMode::Custom   // default; frame draws everything (macOS keeps native chrome)
DecorationMode::System   // decorations = true, no custom frame, no subclass
mode.uses_custom_frame() -> bool
```

### `FrameAction` / `CaptionControl`

See the tables above; both derive `Debug, Clone, Copy`.
