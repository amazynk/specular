# Accessibility: screen reader and IME test matrix

Specular keeps a screen-reader and IME test matrix for all three OSes. This file
is that matrix.

- **Linux:** the rows were filled by automated checks on 2026-10-08 (see *Automation*).
- **Windows and macOS:** the rows are **to test**. Each one has steps a tester can follow
  without knowing the code.
- **NVDA, Narrator, VoiceOver and the Windows IME** are deferred to a later stage (decided
  2026-10-08). Their rows stay here, marked **deferred**. Orca and macOS input sources are still in scope.

Fill in **Result** (Pass, Fail plus a short note, or Partial plus a note), **Date** and
**Tester** for each row you run. When a row fails, open an issue and put its number in the
note.

## Where things live

| What | Where |
|---|---|
| Accessibility tree of the chrome (toolbar, dialogs, palette, Settings, info panel, live region) | `crates/specular-widgets/src/a11y.rs` (`Chrome::accessibility_tree`) |
| Gallery grid and filter bar nodes | `app/specular/src/app/gallery/mod.rs` (`Gallery::a11y`), `gallery/filterbar.rs` |
| AccessKit adapter (activation, actions, sending updates) | `app/specular/src/a11y.rs` |
| Text fields: palette search, Settings search, go-to-file | `crates/specular-widgets/src/ctx.rs` (`Ctx::field`), `toolbar.rs` (go-to field) |
| Gallery filter field | `app/specular/src/app/gallery/{draw,input,filterbar}.rs` |
| winit IME events into the widgets: preedit, commit, cursor area | `app/specular/src/app.rs` (`WindowEvent::Ime`, the IME area after drawing), `gallery/input.rs` (`ime_preedit`) |
| Unit tests: IME, tree consistency, what AT-SPI can read | `crates/specular-widgets/src/tests.rs` (`mod ime`, `mod tree_updates`) |
| Text fields of the app's sheets: rename, Convert's name, DICOMweb and OMERO forms | `app/specular/src/app/fileops/view.rs`, `convert.rs`, `dicomweb/view.rs` (composition: `specular_widgets::line_edit::compose`) |
| Keyboard crosshair, value under the cursor | `app/specular/src/app/cursor.rs`, `crates/specular-widgets/src/cursor_readout.rs` |
| Screen-reader audit (names, roles, state, order) | `crates/specular-widgets/src/a11y_audit.rs`; run over every chrome screenshot case (`tests/shots.rs`) and the app's sheets (`app/a11y_audit.rs`) |
| Develop: panel, scopes, mask and spot handles, labels over the picture | `crates/specular-widgets/src/develop_panel.rs`, `scopes.rs`, `mask_handles.rs`; `app/specular/src/app/develop.rs` (`a11y`, `labels_a11y`) |
| Sound page: choices, meters, numbers, lyrics, statuses, keyboard | `app/specular/src/app/sound_page.rs` (`a11y`, `SoundPage::key`), tests in `sound_page/keyboard.rs` |
| Linux automation | `tools/a11y/` |

## How to run (all OSes)

Use a release build. Open a folder with at least 4 images, one with a long name, plus one
broken file. To make a broken file, save a text file as `zz-broken.jpg`.

These shortcuts are used in the steps below. On macOS, check the cheat sheet (`?`) for each
one's spelling there.

| Shortcut | Action |
|---|---|
| `Ctrl+K` | command palette |
| `Ctrl+,` | Settings |
| `Ctrl+G` | go to image |
| `i` | info panel |
| `?` | cheat sheet |
| `g` | gallery |
| `/` | gallery filter |
| `+` | zoom in |
| `f` | fit |
| `End` / `Home` | last / first image |
| `Left` / `Right` | previous / next |

## Screen readers

**Expected behaviour** is what the reader must say or do. Wording varies by reader: what has
to come across is the name, the role and the state.

### Orca (Linux)

The automated results (AT-SPI, "auto") use the same tree and events that Orca reads. They
were not spoken aloud: Orca is not installed on the test machine. One run with live speech is
still to do. Its steps are the same as for NVDA, using Orca (`Super+Alt+S`) in a GNOME
session.

| Row | Expected behaviour | Result | Date | Tester |
|---|---|---|---|---|
| Launch and title | App registers. There is one frame (window), named after the image. The app has a name. | Pass (auto) | 2026-10-08 | atspi-check |
| Toolbar buttons | A tool bar with at least 8 controls. Every control is named and its shortcut is exposed. Buttons have a press action, and pressing Next shows the next image. | Pass (auto). Note: the tool bar itself has no name (needs a new string); Orca says "tool bar". | 2026-10-08 | atspi-check |
| Image name and position | An image node named with the file name and size. A button named "Image 1 of 4" whose description is "Go to image… (Ctrl+G)". | Pass (auto) | 2026-10-08 | atspi-check |
| Settings navigation | A modal dialog named Settings with focus on its search field. Sections are page tabs. Switches are named and show their state. Every control is named. The toolbar behind the dialog is hidden. Typed text is the field's text with the caret at its end. Escape closes the dialog. | Pass (auto) | 2026-10-08 | atspi-check |
| Palette search results | A modal dialog with focus on a named search field. Results are named list items. Typing filters them, and the typed text with its caret is readable. One result is selected, and Down moves the selection. Escape closes the palette. | Pass (auto) | 2026-10-08 | atspi-check |
| Gallery grid navigation and item announcement | The grid is a named list box. Tiles are named after file name, size and date. One tile has focus, and Right moves the focus with a focus event. Names carry no bidi isolation marks. | Pass (auto) | 2026-10-08 | atspi-check |
| Gallery filter | `/` focuses a named search field. Typed text and caret are readable. The result count is announced. | Pass (auto) | 2026-10-08 | atspi-check |
| Info panel | A panel named "Image information" with two page tabs (Summary, All fields), one of them selected, and a named Close button. | Pass (auto) | 2026-10-08 | atspi-check |
| Dialogs | The cheat sheet is a named modal dialog. The go-to field is a named entry, its typed text is readable, and Enter goes to the image. | Pass (auto) | 2026-10-08 | atspi-check |
| Live: zoom level | Zooming announces the new level ("150%"). The zoom combo box carries the level. Opening another image at its own zoom does not announce. | Pass (auto + unit test) | 2026-10-08 | atspi-check |
| Live: errors | A broken file shows an alert named after the error, announced assertively ("Can’t open zz-broken.jpg: unsupported format"). | Pass (auto) | 2026-10-08 | atspi-check |
| Keyboard crosshair | `x` turns it on and says how to use it. `Alt+R` reads the pixel under it. With "Read the value under the cursor" on, a resting crosshair's value is read once, not at every step. `Esc` turns it off. | Pass (auto) | 2026-10-24 | atspi-check |
| Every screen walked | No unnamed control, no tick box or switch without its state, no tab that cannot be selected, on each of the 18 trees the run saves. | Pass (auto) | 2026-10-24 | atspi-check |
| Live speech (all of the above, spoken) | Orca speaks every row above without saying "unlabelled" and without reading stray symbols. | to test | | |

### NVDA (Windows 10/11) — deferred

Install NVDA 2024.x or newer and start it (`Ctrl+Alt+N`). Then start Specular.

| Row | Steps | Expected behaviour | Result | Date | Tester |
|---|---|---|---|---|---|
| Launch and title | Open an image. Press `NVDA+T`. | Says the window title (the file name), then "Specular". | deferred | | |
| Toolbar buttons | `Tab` through the toolbar. | Each button is spoken as "*name*, button" with its shortcut, e.g. "Next image, button, Right". Nothing is "unlabelled". `Space` or `Enter` on Next shows the next image. | deferred | | |
| Image name and position | `NVDA+Tab` on the image. Then `Tab` to the position button. | "beach.jpg, *w* × *h* pixels", then "Image 1 of 4, button, Go to image…". | deferred | | |
| Settings navigation | `Ctrl+,`. Type `zoom`. `Tab` through the dialog, and toggle a switch with `Space`. Press `Escape` twice. | "Settings, dialog", then "Search settings, edit". The typed letters are echoed. Section tabs are read as "tab, *n* of *m*". Switches are read as "toggle button, pressed / not pressed". Escape closes the dialog. | deferred | | |
| Palette search results | `Ctrl+K`. Type `rotate`. Use `Down`, `Up`, `Escape`. | "edit" for the field, and the typed text is echoed. Each `Down` reads the selected result. Escape closes the palette. | deferred | | |
| Gallery grid | `g`, then arrow keys. | "images, 4 items, list". Each tile is read as name, size and date. There are no stray symbols between words: the isolation marks are stripped. | deferred | | |
| Gallery filter | `/`, then type `snow`. | "Filter by name or field, edit". After typing, the result count is spoken ("1 item matches"). | deferred | | |
| Info panel | `i`, then `Tab` to the tabs. | "Image information" panel. "Summary, tab, selected" / "All fields, tab". "Close, button". | deferred | | |
| Dialogs | `?`, then `Escape`. `Ctrl+G`, `3`, `Enter`. | Cheat sheet is read as a dialog. The go-to field echoes "3", and the third image opens (its name is spoken). | deferred | | |
| Live: zoom level | `+` twice, then `f`. | Each new level ("150%", …) spoken without moving focus. Watch whether the level is spoken twice (once from the combo box value, once from its description). If it is, note it. | deferred | | |
| Live: errors | `End` (the broken file). | The error ("Can’t open zz-broken.jpg: unsupported format") is spoken immediately (assertive). | deferred | | |

### Narrator (Windows 10/11) — deferred

Start Narrator with `Win+Ctrl+Enter`. Use the same steps as NVDA, but with `Narrator+T` for
the title and Scan mode off.

| Row | Expected behaviour | Result | Date | Tester |
|---|---|---|---|---|
| Launch and title | Title and app name spoken. | deferred | | |
| Toolbar buttons | Named buttons with shortcuts. Invoke works. | deferred | | |
| Image name and position | Image name and size, and "Image 1 of 4" button. | deferred | | |
| Settings navigation | Dialog, search edit, tabs, toggle states. Escape closes. | deferred | | |
| Palette search results | Edit with echo. Selected result read on `Down`. | deferred | | |
| Gallery grid | List with items read on arrow keys. | deferred | | |
| Gallery filter | Edit, then the result count spoken. | deferred | | |
| Info panel | Panel, tabs, Close button. | deferred | | |
| Dialogs | Cheat sheet dialog. Go-to edit echoes and navigates. | deferred | | |
| Live: zoom level | Level spoken on `+`. Note any double speech. | deferred | | |
| Live: errors | Error spoken immediately. | deferred | | |

### VoiceOver (macOS 13+) — deferred

Start VoiceOver with `Cmd+F5`. Navigate with `VO` (`Ctrl+Option`) plus the arrow keys. Check
the app's own shortcuts in the cheat sheet.

| Row | Steps | Expected behaviour | Result | Date | Tester |
|---|---|---|---|---|---|
| Launch and title | Open an image. Press `VO+F2` twice. | Window title (file name), then "Specular". | deferred | | |
| Toolbar buttons | `VO+Right` across the toolbar. Press `VO+Space` on Next. | "*name*, button" for each. Next shows the next image. | deferred | | |
| Image name and position | `VO+Right` to the image, then to the position. | "beach.jpg, …, image", then "Image 1 of 4, button". | deferred | | |
| Settings navigation | Settings shortcut, type `zoom`, `VO+Right` through, `VO+Space` on a switch. | "Settings, dialog". "Search settings, search text field". Typed text is echoed. Tabs and switch states are read. Escape closes. | deferred | | |
| Palette search results | Palette shortcut, type `rotate`, `Down`. | Search field echo. The selected result is read on `Down`. | deferred | | |
| Gallery grid | `g`, then arrow keys. | "images, 4 items, list". Each focused tile is read. | deferred | | |
| Gallery filter | `/`, type `snow`. | Search field, then the result count is announced. | deferred | | |
| Info panel | `i`, then `VO+Right`. | "Image information" group, Summary/All fields tabs (one selected), Close button. | deferred | | |
| Dialogs | `?`. Go-to shortcut, `3`, `Return`. | Cheat sheet dialog. Go-to field echoes and the third image opens. | deferred | | |
| Live: zoom level | `+`. | The new level ("150%") spoken. Note any double speech. | deferred | | |
| Live: errors | `End`. | Error spoken immediately. | deferred | | |

## Input methods

Every row covers all four text fields: the palette search (`Ctrl+K`), the Settings search
(`Ctrl+,`), the go-to field (`Ctrl+G`, digits only, so CJK commit is not expected there; check
only that a composition doesn't close it), and the gallery filter (`g`, then `/`).

**Expected behaviour**

| Row | Expected behaviour |
|---|---|
| CJK preedit display | While composing, the composition shows inline at the caret, underlined, and is not yet part of the field's text (filtering doesn't run on it). |
| Candidate window follows the caret | The IME's candidate window opens just below the caret, not at the field's start or the window corner. It moves right as the text grows. In RTL text it sits at the caret on the left side. |
| Commit | Choosing a candidate (`Space`, a number, or `Enter`) inserts it once, at the caret, with the caret after it. There is no double insertion, and `Enter` while composing does not also run the field's action. |
| Cancel | `Escape` while composing drops the composition and nothing is inserted. The dialog or field stays open. A second `Escape` closes it as usual. Arrow and `Tab` keys while composing go to the IME, not the app. |
| RTL (Hebrew/Arabic) | Text runs right to left and the caret sits at the logical end (on the left). Mixed with Latin, the order is correct. A screen reader reads the field's text. |
| Emoji | The emoji picker or input source inserts the emoji, including ZWJ sequences and skin tones (👍🏽, 👩‍💻), as one cluster. The caret moves over it as one unit, and Backspace deletes it whole. |

### IBus (Linux)

The automated rows used IBus 1.5.34 over XIM, with the Cangjie 5 table engine and xdotool, on
X11. Wayland text-input-v3 needs a compositor with an input-method relay plus
ibus-wayland, neither of which was available headless. That run is the human Wayland row
below.

| Row | Expected behaviour | Result | Date | Tester |
|---|---|---|---|---|
| CJK preedit display | As above. | Pass (auto: palette, Settings, gallery filter). `Preedit("日")` reaches the app, the field's text stays empty until commit, and the unit tests check inline display and underline. | 2026-10-08 | ime-check |
| Candidate window follows the caret | As above. | Pass (auto). The IME area starts at the caret and widens over the composition. Unit tests check the RTL position. | 2026-10-08 | ime-check |
| Commit | As above. | Pass (auto). `Space` commits 日, the field's text is "日", and the caret is at 1. Seen once in about 8 runs: the commit was lost after the preedit was cleared (Settings field). Watch for it in human runs. | 2026-10-08 | ime-check |
| Cancel | As above. | Pass (auto). `Escape` clears the preedit with no commit, and the field stays focused with only the earlier text. Unit tests check that named keys are swallowed while composing. | 2026-10-08 | ime-check |
| RTL (Hebrew) | As above. | Pass (auto). IBus Hebrew engine on the `il` layout: "שלום" typed, read back over AT-SPI with the caret at the end. | 2026-10-08 | ime-check |
| Emoji | As above. | Pass (auto). 👍🏽 (base plus skin tone) typed as two keys and read back as one field text. The IBus emoji picker (`Ctrl+.`) is still to test. | 2026-10-08 | ime-check |
| Rename field (F2) | The composition shows in the field at the caret, the candidate window follows it, `Space` commits over the selected stem (the extension stays), `Escape` cancels the composition and the sheet stays open. | Pass (auto) | 2026-10-24 | ime-check |
| All rows on GNOME Wayland (text-input-v3) | Same as the X11 rows. | to test | | |
| IBus emoji picker, Arabic layout | Steps: `Ctrl+.` in the palette, pick 👩‍💻. Switch to Arabic and type. | to test | | |

### Fcitx5 (Linux)

Fcitx5 was not available on the test machine. Install `fcitx5`, `fcitx5-chinese-addons` and
`fcitx5-mozc`, and set `XMODIFIERS=@im=fcitx`.

| Row | Steps | Expected behaviour | Result | Date | Tester |
|---|---|---|---|---|---|
| CJK preedit display | Pinyin: in the palette, type `nihao`. | Inline underlined "nihao" or "你好" at the caret. | to test | | |
| Candidate window follows the caret | Type more after a commit. | The candidate window follows the caret. | to test | | |
| Commit | `Space`. | "你好" is inserted once. | to test | | |
| Cancel | Type `ni`, then `Escape`. | Nothing is inserted and the palette stays open. | to test | | |
| RTL | Add the Hebrew keyboard and type `שלום abc`. | Correct order, and the caret is at the logical end. | to test | | |
| Emoji | Fcitx5 emoji (`Ctrl+Alt+.` or the unicode addon). | One cluster, deleted whole. | to test | | |
| Mozc (Japanese) | Type `nihon`, `Space` to convert, `Enter`. | 日本 is committed, and `Enter` does not also run a palette command. | to test | | |

### Windows IME (Windows 10/11) — deferred

Add Microsoft Pinyin, Microsoft IME (Japanese) and the Hebrew keyboard in Settings → Time &
language. Switch between them with `Win+Space`.

| Row | Steps | Expected behaviour | Result | Date | Tester |
|---|---|---|---|---|---|
| CJK preedit display | Pinyin: `Ctrl+K`, type `nihao`. | Inline underlined composition at the caret, with no floating composition window at the screen corner. | deferred | | |
| Candidate window follows the caret | Type `nihao`, then commit, then `shijie`. | The candidate list sits under the caret both times. | deferred | | |
| Commit | `Space` or `1`. Japanese: `nihon`, `Space`, `Enter`. | Inserted once, and `Enter` doesn't run a command. | deferred | | |
| Cancel | `ni`, then `Escape`. | Nothing is inserted and the palette stays open. | deferred | | |
| RTL | Hebrew keyboard: type `שלום abc` in the gallery filter. | Correct order, the caret is at the logical end. (NVDA reading it is deferred.) | deferred | | |
| Emoji | `Win+.`, then pick 👍🏽 and 👩‍💻. | Each is inserted as one cluster and deleted whole. | deferred | | |

### macOS input sources (macOS 13+)

Add Pinyin – Simplified, Japanese (Romaji) and Hebrew in System Settings → Keyboard → Input
Sources. Switch between them with `Ctrl+Space`.

| Row | Steps | Expected behaviour | Result | Date | Tester |
|---|---|---|---|---|---|
| CJK preedit display | Pinyin: open the palette, type `nihao`. | Inline underlined marked text at the caret. | to test | | |
| Candidate window follows the caret | As for Windows. | The candidate panel sits under the caret. | to test | | |
| Commit | `Space` or `1`. Japanese: `nihon`, `Space`, `Return`. | Inserted once, and `Return` doesn't run a command. | to test | | |
| Cancel | `ni`, then `Escape`. | Nothing is inserted and the palette stays open. | to test | | |
| RTL | Hebrew: `שלום abc`. | Correct order and caret position. (VoiceOver reading it is deferred.) | to test | | |
| Emoji | `Ctrl+Cmd+Space` (Character Viewer), pick 👍🏽 and 👩‍💻. Also try press-and-hold accents: hold `e`, then pick `é`. | One cluster each. The accent popup appears at the caret and replaces the `e`. | to test | | |

## Automation (Linux)

Both checks run against a debug or release build, entirely in private sessions. Nothing
touches the desktop, the user's D-Bus, the clipboard or `~/.config`. `HOME` and every XDG
folder point into `$CARGO_TARGET_DIR/a11y-check/` (or `ime-check/`).

```sh
cargo build -p specular
tools/a11y/atspi-check.sh          # screen-reader rows: 77 checks
tools/a11y/atspi-check.sh --ime    # IME rows: 43 checks
```

**Screen-reader rows** (`atspi_check.py`) run these pieces:

- `dbus-run-session` with a private session bus (`session-bus.conf.in`).
- A private accessibility bus, with `a11y_bus.py` serving `org.a11y.Bus` and setting
  `IsEnabled`/`ScreenReaderEnabled` the way Orca would.
- `at-spi2-registryd`.
- A headless sway (pixman).
- The app on lavapipe.
- wtype for keys.

The checks walk the tree with libatspi, the library Orca uses. The trees are saved in
`trees/*.txt` and the results in `results.json`.

It needs:

- sway, wtype
- dbus-daemon, at-spi2-core
- python3-gi with Atspi 2.0
- mesa-vulkan-drivers

**IME rows** (`ime_check.py`) run these pieces:

- The same accessibility bus.
- A private Xvfb with no TCP, no socket file and no lock file.
- `ibus-daemon --xim` on a private address, with the Cangjie 5 table engine. Its Hebrew and
  US engines use `setxkbmap`.
- The app on X11 with `XMODIFIERS=@im=ibus` and `SPECULAR_LOG=input:debug`.
- xdotool.

The checks read winit's `Ime` events and the IME cursor area from the app's input log, and the
field's text and caret over AT-SPI.

It needs:

- Xvfb, xdotool, x11-xkb-utils
- ibus, ibus-x11, ibus-table with `cangjie5.db`

Unit tests that back the rows (`cargo test -p specular-widgets`):

- `ime::*`: preedit display, caret area in LTR and RTL, named keys swallowed while
  composing, preedit cleared when the field goes away.
- `tree_updates::every_pair_of_steps`: every pair of 13 UI steps produces trees that
  AccessKit's consumer accepts. This guards against the crash found here.
- `tree_updates::zoom_changes_are_announced`
- `tree_updates::what_atspi_can_read`
- `tree_updates::names_without_isolation_marks`
- `a11y_audit::*` and the audit in `tests/shots.rs`: every chrome screenshot case (Settings,
  palette, dialogs, help, editors, panels, in LTR and Hebrew) has named controls, the state
  they need and an order that follows what is seen. The app's sheets are audited in their own
  screenshot tests (`cargo test -p specular --bin specular`).
- `accessibility::the_value_under_the_cursor_is_read_once_it_rests`,
  `cursor_readout::*`: the debounce and the live region.

## Known gaps

- **Views added since 2026-10-25 not run over AT-SPI:** Develop, the sound page, People and
  the model manager are checked on the AccessKit tree by unit tests (audit, focus, live
  statuses). `atspi-check.sh` has no rows for them yet.
- **Retouch spots from the keyboard:** done. The handles of the selected spot move by
  keyboard, and the spot crosshair (`x` in Develop) places new spots; each step is read out.
  Brush areas are still painted with the pointer.
- **Collapsed state over AT-SPI:** AccessKit's AT-SPI adapter passes "expanded" on but not
  "collapsed", so Orca cannot tell a closed combo box from one that never opens. The audit
  checks it on the AccessKit tree instead.
- **Flaky IME rows:** in about one run in three, one unrelated row fails (the Enabled event
  of the first field, or the palette not opening after the layout switch). Rerun before
  reporting a failure.
- **Preedit caret:** winit's preedit cursor index is ignored, so the caret is always at the end
  of the composition. This matters for IMEs that move within the preedit, such as Japanese
  segment editing.
- **Preedit not exposed to screen readers:** the accessible value of a field excludes the
  preedit. Screen readers read the composition from the IME itself.
- **Unnamed tool bar:** it needs a new translated string such as "Toolbar".
- **Window title:** it is just the file name. Consider adding "— Specular" for title-reading
  commands.
- **Zoom spoken twice:** the zoom combo box has the level as both its value and its
  description, for AT-SPI's sake. Windows and macOS readers may say it twice; the NVDA,
  Narrator and VoiceOver rows ask testers to note this.
- **First CJK glyph delay:** the first CJK glyph took up to about 0.75 s to show on lavapipe,
  because of the fallback font lookup. Later glyphs are immediate.
