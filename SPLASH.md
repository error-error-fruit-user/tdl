# The splash screen

The splash screen (the logo over the Doom fire, with a quip, a tagline and "press any key") dresses up in two files of its own: `splash.toml` for how it always looks, and `pride.toml` for the pride splash (`--with-pride`, or **pride** in the settings). The rest of tdl wears its theme instead: see [THEME.md](THEME.md).

## Wearing one

- **In the settings** (`O` in the launcher): the **splash look** row for `splash.toml`, and **pride flags** for `pride.toml`. `J`/`K` or `←`/`→` go round the built-in one and every one with a [name](#by-name): the ones that come with tdl (the `void` splash) and your own `<name>_splash.toml` or `<name>_pride.toml` files in the standard config dir that can be worn. `Enter` asks for **[B]uilt-in**, **[N]ew** or **[S]elect**. **New** writes the built-in one out to `splash.toml` or `pride.toml` in the standard config dir (`~/.config/tdl/` on Linux), every field explained in comments, for you to change; a file already there is kept as it is. **Select** picks any `.toml` file. `Backspace` goes back to the built-in one. The path is kept in `uac.rite`, and the change shows the next time the splash plays.
- **For one run**: `tdl --splash my.toml` (or a name, below) plays the splash in that look (even if the **splash** setting skips it; `--no-splash` still wins) and asks whether to keep it; `--keep-splash my.toml` keeps it without asking, and with `--session` nothing is asked or kept (see the README's [Keeping what you pass](README.md#keeping-what-you-pass)).

Change the file, and the change shows from the next start. A file tdl can't wear (a misspelled name, a color it doesn't know, a fire with no flames, a file from a newer tdl) is said at startup with what is wrong, and the built-in one is worn instead; `--splash` with such a file stops tdl before it starts.

### By name

`--splash` and `--keep-splash` take a name as well as a path: `tdl --splash void`. A name is letters, digits, `-` and `_` (so `void.toml` or `./void` is a path). tdl looks for `<name>_splash.toml` in the standard config dir first, so a splash of your own saved as `~/.config/tdl/ember_splash.toml` is `--splash ember`; if there is none, it is one of the splashes that come with tdl:

| Name | What it is |
| --- | --- |
| `hellfire` | The built-in splash: the TDL logo cooling from flame to blood over the fire of the PlayStation port of Doom. |
| `void` | A white fire burning in the dark, its embers turning slowly from violet to teal and back, under the logo going from white through violet to teal. Goes with `--theme void`. |
| `monokai` | Monokai's colors: a fire whose flames take turns in its signature colors, rising to white hot, under the logo in its stripes. |
| `gruvbox` | Gruvbox (dark)'s colors: a fire whose flames take turns in its signature colors, rising to white hot, under the logo in its stripes. |
| `noctis` | Noctis's colors: a fire whose flames take turns in its signature colors, rising to white hot, under the logo in its stripes. |
| `dracula` | Dracula's colors: a fire whose flames take turns in its signature colors, rising to white hot, under the logo in its stripes. |
| `nord` | Nord's colors: a fire whose flames take turns in its signature colors, rising to white hot, under the logo in its stripes. |
| `catppuccin` | Catppuccin (Mocha)'s colors: a fire whose flames take turns in its signature colors, rising to white hot, under the logo in its stripes. |
| `tokyo-night` | Tokyo Night's colors: a fire whose flames take turns in its signature colors, rising to white hot, under the logo in its stripes. |
| `solarized` | Solarized (dark)'s colors: a fire whose flames take turns in its signature colors, rising to white hot, under the logo in its stripes. |
| `rose-pine` | Rosé Pine's colors: a fire whose flames take turns in its signature colors, rising to white hot, under the logo in its stripes. |
| `everforest` | Everforest (dark)'s colors: a fire whose flames take turns in its signature colors, rising to white hot, under the logo in its stripes. |

A kept name is kept as the name. A name tdl can't find stops it before it starts, and says which names it has. Each goes with the theme of the same name, and is written out in [assets/themes/](assets/themes/) as `<name>_splash.toml`, every field set. In the settings, `←`/`→` on the **splash look** row go round them all.

Both files only need what they change: anything left out is the built-in one's, and an empty file is the built-in one. A color is written as in a theme: `"#rrggbb"`, a terminal color's name (`"red"`, `"lightmagenta"`, `"darkgray"`…), a palette number (`"0"` to `"255"`), or `"reset"` for the terminal's own.

## Editions

Each file starts with `edition = 1`, the edition of its format, which is what this tdl writes. A file with no `edition` is edition 1. When a later tdl changes the format, it still reads older editions; a file of an edition newer than tdl knows is refused rather than misread, and tdl says to update.

## The fire

The fire is made of **flames**, as many as you like, and burns with them in turn:

- **A single color glows**: `"#FF7010"`. The flames rise out of the background into that color, and whiten where they are hottest, like a real fire of that color. `"white"` is a fire of grays going white.
- **A list of colors is shaded**: `["black", "white"]`. The flames go through the colors from the coldest (the tips of the wisps) to the hottest (where it burns at the bottom), spread evenly however many there are, and blended in between. Two colors are a smooth ramp, `["black", "white"]` a fire of grays and `["#1A0033", "#8000FF", "white"]` a violet one; the built-in fire is the 37 shades of the PlayStation port of Doom's.
- **With more than one flame the fire cycles** through them in order, blending each into the next, then starts again from the first: `flames = ["#FF7010", "#3A7BD5", ["black", "white"]]`. **`speed`** is how many flames it goes through a second: `0.5` is a new one every two seconds, `2` two a second, and `0` stays on the first. One flame never changes.

Wherever the fire has no heat at all it is the background itself, so there is no seam where it meets the screen.

## `splash.toml`

```toml
edition = 1

[logo]
art = ["██  ██", "██████", "██  ██"]
colors = ["#FFC846", "#BA1A12"]
shadow = "#5C0E0A"

[tagline]
text = "t e r m i n a l   d o o m   l a u n c h e r"
narrow = "terminal doom launcher"
color = "#E4D8BE"
bold = true

[hint]
text = "press any key"
colors = ["#80746A", "#E4D8BE"]
period = 1.6

[quip]
colors = ["#FF7010", "#FFC846"]
period = 0.7
bold = true

[fire]
flames = [["black", "white"]]
speed = 0.5
reach = 80
top = 45
```

### `[logo]`

| Field | Built in | What it is |
| --- | --- | --- |
| `art` | the TDL logo | The logo, one string a row, up to 16 rows of up to 120 characters. Full blocks (`█`) are painted in `colors`; every other character is drawn in `shadow`; spaces let the fire show through. Rows of different lengths are fine: the logo is centered by its widest row. |
| `colors` | flame, then three steps to blood | Stripes from top to bottom, as many as you like, spread evenly over the rows from the first with a block in it to the last. Each row holds two stripes (its top and bottom half), so a 5-row logo shows up to 10; with more stripes than that, some are skipped. One color paints the whole logo. |
| `shadow` | gore `#5C0E0A` | The color of every character in the art that isn't a block (the built-in logo's box-drawing strokes). |

### `[tagline]`

| Field | Built in | What it is |
| --- | --- | --- |
| `text` | `"t e r m i n a l   d o o m   l a u n c h e r"` | The words under the logo (under the quip). At most 120 characters. |
| `narrow` | `"terminal doom launcher"` | The words shown instead when the terminal is too narrow for `text`. |
| `color` | bone `#E4D8BE` | Their color. |
| `bold` | `true` | Whether they are bold. |

### `[hint]`

| Field | Built in | What it is |
| --- | --- | --- |
| `text` | `"press any key"` | The words under the tagline. Empty for none. At most 120 characters. |
| `colors` | ash, bone | Two colors it breathes between: from the first to the second and back. |
| `period` | `1.6` | Seconds for one breath, 0.05 to 60. |

### `[quip]`

The quip itself is picked from the built-in ones, or set with `--splash-quip` or the **splash quip** setting; this is only how it looks.

| Field | Built in | What it is |
| --- | --- | --- |
| `colors` | hellfire, flame | Two colors it flares between. |
| `period` | `0.7` | Seconds for one flare, 0.05 to 60. |
| `bold` | `true` | Whether it is bold. |

### `[fire]`

| Field | Built in | What it is |
| --- | --- | --- |
| `flames` | the PlayStation port's 37 shades | The flames (see [The fire](#the-fire)): at least one. |
| `speed` | `0.5` | Flames a second, 0 or more. |
| `reach` | `80` | How far up the flames reach, as a percentage of the fire's height, 1 to 100. |
| `top` | `45` | Where the fire starts, as a percentage of the way down the screen, 0 to 90. On a short terminal it starts lower, to leave room for the logo, the quip, the tagline and the hint above it. |

## `pride.toml`

The pride splash plays one flag at a time: the logo wears its stripes over a fire that burns through the rainbow. The built-in one has the transgender flag in it, and every other flag below in a comment; the file **New** writes has them all, so you can put flags in and leave them out by taking the `#` off the start of their lines or putting it back. **When more than one flag is in, each splash picks one of them at random.**

```toml
edition = 1

[fire]
flames = ["#FF0000", "#FF8000", "#FFFF00", "#00FF00", "#0080FF", "#8000FF"]
speed = 1.0

[[flag]]
name = "transgender"
stripes = ["#5BCEFA", "#F5A9B8", "#FFFFFF", "#F5A9B8", "#5BCEFA"]

[[flag]]
name = "rainbow"
stripes = ["#E40303", "#FF8C00", "#FFED00", "#008026", "#004CFF", "#732982"]
fire = { flames = ["#E40303", "#FF8C00", "#FFED00", "#008026", "#004CFF", "#732982"], speed = 2 }
```

### `[fire]`

| Field | Built in | What it is |
| --- | --- | --- |
| `flames` | red, orange, yellow, green, blue, violet, each glowing | The fire behind every flag without one of its own, written as in `splash.toml`. |
| `speed` | `1.0` | Flames a second, so the built-in rainbow goes round every six seconds. |

The fire's `reach` and `top` are `splash.toml`'s.

### `[[flag]]`

Each `[[flag]]` is one flag; as many as you like (up to 256). With none at all, the pride splash keeps `splash.toml`'s logo colors and only burns the pride fire.

| Field | What it is |
| --- | --- |
| `name` | The flag's name. Only for you: tdl never shows it. |
| `stripes` | Its stripes from top to bottom, at least one, spread over the logo the way `splash.toml`'s `colors` are: two to a row, so a 6-stripe flag fits a 5-row logo whole. |
| `fire` | Optional: `{ flames = [...], speed = ... }`, a fire of the flag's own in place of the pride fire. |

### The flags in the built-in file

In: **transgender**. In comments, ready to put in: **rainbow** (six stripes), **lesbian**, **gay** (men's), **bisexual**, **pansexual**, **polysexual**, **asexual**, **aromantic**, **aroace**, **demisexual**, **nonbinary**, **genderfluid**, **genderqueer**, **agender** and **intersex**. Flags drawn with more than stripes (a chevron, a circle, a triangle) are their stripes only.
