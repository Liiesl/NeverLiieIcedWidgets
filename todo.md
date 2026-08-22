# NeverLiie Iced Widgets - Todo

## Input
- [x] TextInput - single-line text input (iced: `text_input` - complete, supports secure/clipboard/IME)
- [x] GhostTextInput - text input with animated ghost trail cursor (custom widget with cubic-bezier eased cursor animation)
- [x] TextArea - multi-line text input (iced: `text_editor` - **needs polish**: it's a full code editor, too complex for simple textarea use case)
- [ ] NumberInput - numeric input with increment/decrement (missing)
- [x] PasswordInput - masked text input (iced: `text_input().secure(true)` - works but no dedicated widget)

## Buttons
- [x] Button - clickable action trigger (iced: `button` - complete, multiple styles: primary/secondary/success/warning/danger/text/subtle)
- [ ] ToggleButton - on/off state button (missing)
- [ ] IconButton - button with icon only (missing, can be done with `button(icon)` but no dedicated type)
- [ ] SplitButton - button with dropdown menu (missing)

## Selection
- [x] Checkbox - multi-select toggle (iced: `checkbox` - complete, multiple styles, customizable icon)
- [x] Radio - single-select from options (iced: `radio` - complete)
- [x] Switch - toggle on/off (iced: `toggler` - complete, named "toggler" not "switch")
- [x] Select/Dropdown - choose from list (iced: `pick_list` - complete, no search)
- [x] Select/Dropdown (searchable) - iced: `combo_box` - **needs polish**: API more complex than needed for simple cases

## Sliders & Progress
- [x] Slider - value selection via drag (iced: `slider` - complete, keyboard support)
- [ ] RangeSlider - dual-handle range selection (missing)
- [x] ProgressBar - linear progress indicator (iced: `progress_bar` - complete, supports vertical)
- [ ] CircularProgress - ring/spinner progress (missing, only loading_spinners example)

## Containers & Layout
- [x] Scrollable - overflow scroll container (iced: `scrollable` - very mature ~2400 lines)
- [ ] Tabs - tabbed content switcher (missing)
- [ ] Accordion - collapsible sections (missing)
- [x] SplitPane - resizable split view (iced: `pane_grid` - complete but more complex than simple split pane)

## Lists & Data
- [ ] ListView - simple item list (missing)
- [x] TableView - data grid/table (iced: `table` - complete with sorting)
- [ ] TreeView - hierarchical data (missing)

## Navigation
- [ ] MenuBar - application menu (missing)
- [ ] Pagination - page navigation (missing)
- [ ] Breadcrumbs - path navigation (missing)

## Feedback
- [x] Tooltip - hover hint (iced: `tooltip` - complete with positioning/delay)
- [ ] Modal - overlay dialog (missing, only example exists)
- [ ] Toast/Alert - notification popup (missing, only example exists)

## Misc
- [ ] Badge - status/count indicator (missing)
- [x] Divider - separator line (iced: `rule` - basic, no theming)
- [x] Spacer - flexible spacing (iced: `space` - basic)

## Advanced/Complex
- [ ] ContextMenu - right-click floating menu 
- [x] Overlay/Popover - arbitrary positioned floating content (`src/overlay/` - OverlayManager + Floating + Position/Anchor system)
- [ ] Resizer - resizable container panes (missing, pane_grid has resize but no standalone resizer)
- [ ] ColorPicker - color selection widget (missing)
- [ ] MultiSelect - select multiple items from list (missing)
- [ ] VirtualList - lazy rendering for large datasets (missing)
- [x] TitleBar/WindowFrame - custom cross-platform title bar (`src/title_bar/` - ported from iced-native-frame, WM_NCHITTEST subclass on Windows, overlay resize handles elsewhere)

---

## Summary
- **Existing (18/30):** TextInput, TextArea, PasswordInput, Button, Checkbox, Radio, Switch, PickList, ComboBox, Slider, ProgressBar, Scrollable, PaneGrid, TableView, Tooltip, Rule, Space, Overlay/Popover, LazyIcon
- **Missing (19/36):** NumberInput, ToggleButton, IconButton, SplitButton, RangeSlider, CircularProgress, Tabs, Accordion, ListView, TreeView, MenuBar, Pagination, Breadcrumbs, Modal, Toast/Alert, Badge, ContextMenu, Resizer, ColorPicker, MultiSelect, VirtualList
- **Needs Polish (3):** TextArea (too complex), ComboBox (complex API), Divider/Spacer (too basic)
