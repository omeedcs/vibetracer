use ratatui::style::Color;

/// All semantic colors used throughout the TUI.
pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub fg_dim: Color,
    pub fg_muted: Color,
    pub separator: Color,
    pub accent_warm: Color,
    pub accent_green: Color,
    pub accent_red: Color,
    pub accent_blue: Color,
    pub accent_purple: Color,
    pub bar_filled: Color,
    pub bar_empty: Color,
}

impl Theme {
    pub fn from_preset(name: &str) -> Self {
        match name {
            "catppuccin" => Self::catppuccin(),
            "gruvbox" => Self::gruvbox(),
            "light" => Self::light(),
            _ => Self::dark(),
        }
    }

    pub fn dark() -> Self {
        Self {
            bg: Color::Rgb(15, 17, 21),
            fg: Color::Rgb(160, 168, 183),
            fg_dim: Color::Rgb(58, 62, 71),
            fg_muted: Color::Rgb(90, 101, 119),
            separator: Color::Rgb(42, 46, 55),
            accent_warm: Color::Rgb(138, 117, 96),
            accent_green: Color::Rgb(90, 158, 111),
            accent_red: Color::Rgb(158, 90, 90),
            accent_blue: Color::Rgb(90, 122, 158),
            accent_purple: Color::Rgb(188, 140, 255),
            bar_filled: Color::Rgb(90, 101, 119),
            bar_empty: Color::Rgb(26, 29, 34),
        }
    }

    pub fn catppuccin() -> Self {
        // Catppuccin Mocha palette
        Self {
            bg: Color::Rgb(30, 30, 46),
            fg: Color::Rgb(205, 214, 244),
            fg_dim: Color::Rgb(88, 91, 112),
            fg_muted: Color::Rgb(127, 132, 156),
            separator: Color::Rgb(69, 71, 90),
            accent_warm: Color::Rgb(250, 179, 135),
            accent_green: Color::Rgb(166, 227, 161),
            accent_red: Color::Rgb(243, 139, 168),
            accent_blue: Color::Rgb(137, 180, 250),
            accent_purple: Color::Rgb(203, 166, 247),
            bar_filled: Color::Rgb(137, 180, 250),
            bar_empty: Color::Rgb(49, 50, 68),
        }
    }

    pub fn gruvbox() -> Self {
        // Gruvbox Dark palette
        Self {
            bg: Color::Rgb(40, 40, 40),
            fg: Color::Rgb(235, 219, 178),
            fg_dim: Color::Rgb(102, 92, 84),
            fg_muted: Color::Rgb(146, 131, 116),
            separator: Color::Rgb(80, 73, 69),
            accent_warm: Color::Rgb(254, 128, 25),
            accent_green: Color::Rgb(184, 187, 38),
            accent_red: Color::Rgb(251, 73, 52),
            accent_blue: Color::Rgb(131, 165, 152),
            accent_purple: Color::Rgb(211, 134, 155),
            bar_filled: Color::Rgb(184, 187, 38),
            bar_empty: Color::Rgb(60, 56, 54),
        }
    }

    pub fn light() -> Self {
        Self {
            bg: Color::Rgb(250, 250, 250),
            fg: Color::Rgb(50, 50, 50),
            fg_dim: Color::Rgb(180, 180, 180),
            fg_muted: Color::Rgb(120, 120, 120),
            separator: Color::Rgb(210, 210, 210),
            accent_warm: Color::Rgb(150, 100, 50),
            accent_green: Color::Rgb(40, 120, 60),
            accent_red: Color::Rgb(180, 50, 50),
            accent_blue: Color::Rgb(40, 80, 150),
            accent_purple: Color::Rgb(120, 70, 180),
            bar_filled: Color::Rgb(40, 80, 150),
            bar_empty: Color::Rgb(230, 230, 230),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}
