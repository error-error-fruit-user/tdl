//! The themes and splashes that come with tdl, worn by name (`tdl --theme dracula`), each a theme
//! and a splash of the same name. Their files are in `assets/themes/`; the built-in ones,
//! `hellfire`, are the defaults themselves.

/// Every theme that comes with tdl but the built-in one, by name.
pub const THEMES: &[(&str, &str)] = &[
    (
        "voidless",
        include_str!("../../assets/themes/voidless_theme.toml"),
    ),
    (
        "monokai",
        include_str!("../../assets/themes/monokai_theme.toml"),
    ),
    (
        "gruvbox",
        include_str!("../../assets/themes/gruvbox_theme.toml"),
    ),
    (
        "noctis",
        include_str!("../../assets/themes/noctis_theme.toml"),
    ),
    (
        "dracula",
        include_str!("../../assets/themes/dracula_theme.toml"),
    ),
    ("nord", include_str!("../../assets/themes/nord_theme.toml")),
    (
        "catppuccin",
        include_str!("../../assets/themes/catppuccin_theme.toml"),
    ),
    (
        "tokyo-night",
        include_str!("../../assets/themes/tokyo-night_theme.toml"),
    ),
    (
        "solarized",
        include_str!("../../assets/themes/solarized_theme.toml"),
    ),
    (
        "rose-pine",
        include_str!("../../assets/themes/rose-pine_theme.toml"),
    ),
    (
        "everforest",
        include_str!("../../assets/themes/everforest_theme.toml"),
    ),
];

/// Every splash that comes with tdl but the built-in one, by name.
pub const SPLASHES: &[(&str, &str)] = &[
    (
        "voidless",
        include_str!("../../assets/themes/voidless_splash.toml"),
    ),
    (
        "monokai",
        include_str!("../../assets/themes/monokai_splash.toml"),
    ),
    (
        "gruvbox",
        include_str!("../../assets/themes/gruvbox_splash.toml"),
    ),
    (
        "noctis",
        include_str!("../../assets/themes/noctis_splash.toml"),
    ),
    (
        "dracula",
        include_str!("../../assets/themes/dracula_splash.toml"),
    ),
    ("nord", include_str!("../../assets/themes/nord_splash.toml")),
    (
        "catppuccin",
        include_str!("../../assets/themes/catppuccin_splash.toml"),
    ),
    (
        "tokyo-night",
        include_str!("../../assets/themes/tokyo-night_splash.toml"),
    ),
    (
        "solarized",
        include_str!("../../assets/themes/solarized_splash.toml"),
    ),
    (
        "rose-pine",
        include_str!("../../assets/themes/rose-pine_splash.toml"),
    ),
    (
        "everforest",
        include_str!("../../assets/themes/everforest_splash.toml"),
    ),
];
