# v2 Input: Shared Mouse & Keyboard Focus

The v2 appliance uses one focus model for pointer and keyboard input. The App
Home drawer is the dropdown for the Home menu; menus and client controls are
reachable from either source, and both paths activate the same action.

## Focus targets

`WmState.focus` holds one of:

| Focus | Meaning |
|---|---|
| `Client` | The active app surface owns focus. |
| `MenuBar { menu }` | A title-bar menu header is focused; no dropdown yet. |
| `Menu { menu, item }` | A dropdown/drawer is open and one item is selected. |

`menu` is one of `Home`, `App`, `View`, or `Testing`. The app menu title changes
with the active app (`System`, `Calc`, `Files`, `Mines`). There is no separate
dock focus state: the App Home drawer is simply `Menu { Home, .. }`.

`WmState.hover` is pointer-only preview state. Moving the mouse never steals
keyboard focus; clicking sets focus to the clicked target.

`focus_visible` is true after keyboard navigation and false after a click. This
lets mouse users avoid persistent focus rings while keyboard users always see
where they are. The focus ring is amber (`theme::FOCUS`); hover is a weaker
outline/fill (`theme::TEXT_SOFT` / `SURFACE_HOVER`).

## Keymap

### Global

| Key | Action |
|---|---|
| `F10` / `Alt+M` | Focus the Home header |
| `F9` / `Alt+D` | Open the App Home drawer |
| `Ctrl`/`Cmd` + `1..4` | Launch System / Calc / Files / Mines |
| `Esc` | Close one layer; from the client, focus Home without opening it |
| `Tab` / `Shift+Tab` | Traverse focusable controls in the active client |
| `Enter` / `Space` | Activate the focused control |
| `C` | Clear the calculator |

`Space` is not a dock toggle: it activates the focused item. Use `F9` to open
the App Home drawer.

### Escape order

| Current focus | `Esc` result |
|---|---|
| Any dropdown/drawer | Close it and focus that menu header |
| Home header | Return focus to the client |
| Another menu header | Focus the Home header |
| Client | Focus the Home header, drawer closed |

### Menu bar and dropdowns

| Key | Action |
|---|---|
| `Left` / `Right` | Previous / next header (Home, App, View, Testing) |
| `Tab` / `Shift+Tab` | Previous / next header |
| `Down` / `Enter` | Open the menu and select the first item |
| `Up` / `Down` | Move item selection |
| `Home` / `End` | First / last item |
| `Enter` / `Space` | Execute the selected `Command` |
| `Esc` | Close the dropdown, return to its header |

### App Home drawer

| Key | Action |
|---|---|
| `Up` / `Down` | Previous / next app tile |
| `Home` / `End` | First / last app tile |
| `Enter` / `Space` | Launch the focused app |
| `Esc` | Close the drawer, focus the Home header |

### Files app

| Key | Action |
|---|---|
| `Up` / `Down` | Move the browser row selection |
| `Home` / `End` | First / last browser row |
| `Left` / `Backspace` | Go to the parent folder |
| `Right` | Focus the right-pane controls |
| `Enter` / `Space` | Open a folder / pin a wallpaper / activate a control |
| `Tab` / `Shift+Tab` | Cycle `Browser -> Mode -> A+ -> A- -> TTS -> Prev -> Next` |
| `Left` / `Right` (control focused) | Previous / next control |
| `I` | Toggle the Raw / Context preview |
| `PageUp` / `PageDown` (or `[` / `]`) | Page the code preview |
| `{` / `}` (or `<` / `>`) | Horizontal code scroll |
| `J` / `K` | Vertical code scroll (also moves context scroll) |

With the browser focused, `Up`/`Down` move rows; `Right` enters the control
strip. With a control focused, `Left`/`Right` move along the strip and `Esc`,
`Up`, or `Down` return to the browser.

### System app

| Key | Action |
|---|---|
| `1..4` or `Left` / `Right` | Select About / Locale / Fonts / Icons |
| `Tab` / `Shift+Tab` | Cycle the section tabs |
| `Home` / `End` | First / last section |

## Casing

User-facing labels use Title Case (`System`, `View`, `About`, `Toggle Context`,
`Snapshot`, `Raw View`). Technical acronyms and data keep their canonical
casing (`UTF-8`, `RGBA8888`, `FNV-1a`, `TTS`, `gh://`), and source samples keep
their original text.

## Input results and browser defaults

`InputResult` values:

| Value | Name | Host behavior |
|---|---|---|
| `0` | `Noop` | Event was not consumed; keep the browser default. |
| `1` | `Changed` | Consumed, redraw. |
| `2` | `Snapshot` | Consumed, redraw, capture snapshot. |
| `3` | `Tts` | Consumed, redraw, toggle speech. |
| `4` | `Handled` | Consumed, no visual change. |

The host calls `app_key_mods(key, mods)` when available (falling back to
`app_key(key)`), and only calls `preventDefault()` when the guest consumes the
event. Modifier bits are:

| Bit | Modifier |
|---|---|
| `1` | Shift |
| `2` | Ctrl |
| `4` | Alt |
| `8` | Meta / Cmd |

## Pointer hover

The host coalesces `pointermove` to one guest update per animation frame and
calls `app_pointer_move(x, y)`. `pointerleave` calls `app_pointer_leave()`.
Hover changes redraw only when the hover target changes. Menus, Home tiles,
Files rows, Files controls, and System tabs all share this hover path.

## Test surface

- `app_focus()` returns `0` for client, `100 + menu` for the menu bar, and
  `200 + menu * 10 + item` for a dropdown/drawer.
- `app_files_focus()` returns the `FilesFocus` discriminant.
- `app_files_context_mode()` returns `0` for Raw and `1` for Context.
- `app_dock_visible()` reports whether the App Home drawer is open.
