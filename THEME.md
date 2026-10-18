# Themes

Every color, border and glyph tdl draws (everywhere but the splash screen) comes from its **theme**: the built-in one (hot metal, old blood and toxic waste), or a `theme.toml` of your own. The splash screen dresses up apart, in `splash.toml` and `pride.toml`: see [SPLASH.md](SPLASH.md).

## Wearing a theme

- **In the settings** (`O` in the launcher), the **theme** row: `Enter` asks for **[B]uilt-in**, **[N]ew** or **[S]elect**. **New** writes the built-in theme out to `theme.toml` in the standard config dir (`~/.config/tdl/theme.toml` on Linux), with every field below in it, and wears it; a `theme.toml` already there is kept as it is. **Select** picks any `.toml` file. `Backspace` goes back to the built-in theme. The theme's path is kept in `uac.rite`, so it is worn every time tdl starts.
- **For one run**: `tdl --theme my.toml` wears it and asks whether to keep wearing it; `--keep-theme my.toml` keeps it without asking, and with `--session` nothing is asked or kept (see the README's [Keeping what you pass](README.md#keeping-what-you-pass)).

Change the file, and tdl wears the change from the next start (or pick it again in the settings to wear it now). A theme tdl can't wear (a misspelled name, a color it doesn't know, a file from a newer tdl) is said at startup, with the line the mistake is on, and the built-in theme is worn instead; `--theme` with such a file stops tdl before it starts.

## Writing one

A theme only needs what it changes: anything it leaves out is the built-in theme's. An empty file is the built-in theme.

```toml
edition = 2

[colors]
background = "reset"        # the terminal's own background
screen = "#3a7bd5"          # the launcher's and the editor's border
key = "lightcyan"

[styles]
cursor = { fg = "black", bg = "#3a7bd5", bold = true }

[borders]
window = "rounded"

[glyphs]
broken_mark = "x"
```

- **A color** is `"#rrggbb"`, a terminal color's name (`"black"`, `"red"`, `"green"`, `"yellow"`, `"blue"`, `"magenta"`, `"cyan"`, `"gray"`, `"darkgray"`, `"lightred"`, `"lightgreen"`, `"lightyellow"`, `"lightblue"`, `"lightmagenta"`, `"lightcyan"`, `"white"`), a terminal palette number (`"0"` to `"255"`), or `"reset"` for the terminal's own. Names don't care about case, `-` or `_`.
- **A style** is `{ fg = color, bg = color, bold = true }`, each part optional: no `fg` or `bg` keeps whatever is under it.
- **A border** is `plain`, `rounded`, `double`, `thick`, `quadrant_inside` or `quadrant_outside`.
- **A glyph** is any text, usually one character.

## Editions

`edition` says which edition of the format the file is written in. This tdl writes edition **2**. A file with no `edition` is edition 1, the theme of tdl 1.0.0, which also had `[splash]` and `[pride]` sections: they are left out now (the splash screen is in `splash.toml` and `pride.toml`), and everything else reads as it was. A file of an edition newer than tdl knows is refused rather than misread, and tdl says to update.

## Every field

The built-in palette the defaults are picked from: void `#0E0808`, blood `#BA1A12`, gore `#5C0E0A`, hellfire `#FF7010`, flame `#FFC846`, bone `#E4D8BE`, ash `#80746A`, iron `#463C3A`, nukage `#7CC82E`, soul `#609CFF`.

### `[colors]`

| Field | Built in | What it colors |
| --- | --- | --- |
| `background` | void | Behind everything. `"reset"` keeps the terminal's own (the fire then fades into black). |
| `text` | bone | Ordinary text. |
| `muted` | ash | Labels, hints, and anything that is unset rather than wrong. |
| `title` | flame | Window titles. |
| `key` | flame | Keys you can press, wherever they are shown, and the focused field's label in a form. |
| `screen` | blood | The border of the main screens: the launcher, the editor, the wads screen, the settings. |
| `multiplayer` | hellfire | The border of the multiplayer screens: hosting, joining, picking and naming profiles. |
| `netplay` | lightmagenta | The border of the windows shown while hosting, joining and downloading. |
| `dialog` | bone | The border of yes/no questions and other small popups. |
| `warning` | blood | The border of warnings. |
| `legend` | ash | The border of the key legend (`?`/`F1`). |
| `panel` | iron | The border of panels that only report, like the ones under the launcher. |
| `busy` | hellfire | The border of windows that show something in progress. |
| `hashing` | nukage | The border and bar of the window that shows wads being hashed. |
| `unsealing` | soul | The border and bar of the window that shows a scroll being unsealed. |
| `quip` | flame | A quip in a window that is busy. |
| `good` | nukage | Something that works (the status mark in the panels). |
| `bad` | blood | Something that stops a profile from running. |
| `changed` | hellfire | Something that still runs but deserves a look. |
| `volatile` | soul | A volatile wad. |
| `divider` | iron | Lines that only divide things, like the one under the editor's base wad. |
| `gauge` | iron | The empty part of a progress bar. |
| `gauge_label` | bone | The words on a progress bar. |

### `[styles]`

| Field | Built in | What it styles |
| --- | --- | --- |
| `cursor` | void on hellfire, bold | The cursor row of a list. |
| `broken_cursor` | bone on blood, bold | The cursor row when what it points at is broken. |
| `selected` | bone on iron | Rows picked with `Shift+V` or `M`. |
| `broken_row` | bone on gore | A broken row not under the cursor. |
| `header` | ash, bold | Column headings. |
| `broken_badge` | bone on blood, bold | The `!` badge on a broken row. |
| `changed_badge` | void on hellfire, bold | The `~` badge on a changed row. |
| `volatile_badge` | void on soul, bold | The `*` badge on a volatile row. |

### `[borders]`

| Field | Built in | What it borders |
| --- | --- | --- |
| `window` | `thick` | Full screens and panels. |
| `popup` | `plain` | Small popups, kept plain so they stay out of the way. |

### `[glyphs]`

| Field | Built in | Where it is drawn |
| --- | --- | --- |
| `status` | `■` | In front of a line in the panels; its color says how things are. |
| `unset` | `□` | In front of a line for something that isn't set. |
| `broken_mark` | `!` | The badge on a broken row. |
| `changed_mark` | `~` | The badge on a changed row. |
| `volatile_mark` | `*` | The badge on a volatile row. |

### `[help]`

The colors of `tdl --help` (and of clap's messages about a mistyped command line). The help wears the theme kept in the settings; `--theme` comes too late for it.

| Field | Built in | What it colors |
| --- | --- | --- |
| `heading` | hellfire | Headings and the usage line (bold). |
| `literal` | flame | What is typed as it is: the flags and commands (bold). |
| `placeholder` | bone | What to fill in, like `<FILE>`. |
| `error` | blood | What went wrong (bold). |
| `valid` | nukage | In a suggestion, the part that was right. |
| `invalid` | flame | The part that wasn't. |
