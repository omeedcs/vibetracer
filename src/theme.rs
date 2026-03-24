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
    pub agent_colors: [Color; 6],
}

impl Theme {
    pub fn from_preset(name: &str) -> Self {
        match name {
            "catppuccin-mocha" => Self::catppuccin_mocha(),
            "catppuccin-macchiato" => Self::catppuccin_macchiato(),
            "gruvbox-dark" => Self::gruvbox_dark(),
            "tokyo-night" => Self::tokyo_night(),
            "tokyo-night-storm" => Self::tokyo_night_storm(),
            "dracula" => Self::dracula(),
            "nord" => Self::nord(),
            "kanagawa" => Self::kanagawa(),
            "rose-pine" => Self::rose_pine(),
            "one-dark" => Self::one_dark(),
            "solarized-dark" => Self::solarized_dark(),
            "everforest-dark" => Self::everforest_dark(),
            "light" => Self::light(),
            "catppuccin-latte" => Self::catppuccin_latte(),
            "gruvbox-light" => Self::gruvbox_light(),
            "solarized-light" => Self::solarized_light(),
            "rose-pine-dawn" => Self::rose_pine_dawn(),
            "everforest-light" => Self::everforest_light(),
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
            agent_colors: [
                Color::Rgb(138, 117, 96),
                Color::Rgb(90, 158, 111),
                Color::Rgb(90, 122, 158),
                Color::Rgb(158, 90, 90),
                Color::Rgb(188, 140, 255),
                Color::Rgb(158, 150, 90),
            ],
        }
    }

    pub fn catppuccin_mocha() -> Self {
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
            agent_colors: [
                Color::Rgb(250, 179, 135),
                Color::Rgb(166, 227, 161),
                Color::Rgb(137, 180, 250),
                Color::Rgb(243, 139, 168),
                Color::Rgb(203, 166, 247),
                Color::Rgb(148, 226, 213),
            ],
        }
    }

    pub fn catppuccin_macchiato() -> Self {
        Self {
            bg: Color::Rgb(36, 39, 58),
            fg: Color::Rgb(202, 211, 245),
            fg_dim: Color::Rgb(91, 96, 120),
            fg_muted: Color::Rgb(128, 135, 162),
            separator: Color::Rgb(73, 77, 100),
            accent_warm: Color::Rgb(245, 169, 127),
            accent_green: Color::Rgb(166, 218, 149),
            accent_red: Color::Rgb(237, 135, 150),
            accent_blue: Color::Rgb(138, 173, 244),
            accent_purple: Color::Rgb(198, 160, 246),
            bar_filled: Color::Rgb(138, 173, 244),
            bar_empty: Color::Rgb(54, 58, 79),
            agent_colors: [
                Color::Rgb(245, 169, 127),
                Color::Rgb(166, 218, 149),
                Color::Rgb(138, 173, 244),
                Color::Rgb(237, 135, 150),
                Color::Rgb(198, 160, 246),
                Color::Rgb(145, 215, 227),
            ],
        }
    }

    pub fn gruvbox_dark() -> Self {
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
            agent_colors: [
                Color::Rgb(254, 128, 25),
                Color::Rgb(184, 187, 38),
                Color::Rgb(131, 165, 152),
                Color::Rgb(251, 73, 52),
                Color::Rgb(211, 134, 155),
                Color::Rgb(250, 189, 47),
            ],
        }
    }

    pub fn tokyo_night() -> Self {
        Self {
            bg: Color::Rgb(26, 27, 38),
            fg: Color::Rgb(169, 177, 214),
            fg_dim: Color::Rgb(65, 72, 104),
            fg_muted: Color::Rgb(86, 95, 137),
            separator: Color::Rgb(52, 59, 88),
            accent_warm: Color::Rgb(255, 158, 100),
            accent_green: Color::Rgb(158, 206, 106),
            accent_red: Color::Rgb(247, 118, 142),
            accent_blue: Color::Rgb(122, 162, 247),
            accent_purple: Color::Rgb(187, 154, 247),
            bar_filled: Color::Rgb(122, 162, 247),
            bar_empty: Color::Rgb(41, 46, 66),
            agent_colors: [
                Color::Rgb(255, 158, 100),
                Color::Rgb(158, 206, 106),
                Color::Rgb(122, 162, 247),
                Color::Rgb(247, 118, 142),
                Color::Rgb(187, 154, 247),
                Color::Rgb(115, 218, 202),
            ],
        }
    }

    pub fn tokyo_night_storm() -> Self {
        Self {
            bg: Color::Rgb(36, 40, 59),
            fg: Color::Rgb(169, 177, 214),
            fg_dim: Color::Rgb(65, 72, 104),
            fg_muted: Color::Rgb(86, 95, 137),
            separator: Color::Rgb(52, 59, 88),
            accent_warm: Color::Rgb(255, 158, 100),
            accent_green: Color::Rgb(158, 206, 106),
            accent_red: Color::Rgb(247, 118, 142),
            accent_blue: Color::Rgb(122, 162, 247),
            accent_purple: Color::Rgb(187, 154, 247),
            bar_filled: Color::Rgb(122, 162, 247),
            bar_empty: Color::Rgb(48, 52, 70),
            agent_colors: [
                Color::Rgb(255, 158, 100),
                Color::Rgb(158, 206, 106),
                Color::Rgb(122, 162, 247),
                Color::Rgb(247, 118, 142),
                Color::Rgb(187, 154, 247),
                Color::Rgb(115, 218, 202),
            ],
        }
    }

    pub fn dracula() -> Self {
        Self {
            bg: Color::Rgb(40, 42, 54),
            fg: Color::Rgb(248, 248, 242),
            fg_dim: Color::Rgb(98, 114, 164),
            fg_muted: Color::Rgb(127, 132, 156),
            separator: Color::Rgb(68, 71, 90),
            accent_warm: Color::Rgb(255, 184, 108),
            accent_green: Color::Rgb(80, 250, 123),
            accent_red: Color::Rgb(255, 85, 85),
            accent_blue: Color::Rgb(139, 233, 253),
            accent_purple: Color::Rgb(189, 147, 249),
            bar_filled: Color::Rgb(189, 147, 249),
            bar_empty: Color::Rgb(55, 57, 69),
            agent_colors: [
                Color::Rgb(255, 184, 108),
                Color::Rgb(80, 250, 123),
                Color::Rgb(139, 233, 253),
                Color::Rgb(255, 85, 85),
                Color::Rgb(189, 147, 249),
                Color::Rgb(255, 121, 198),
            ],
        }
    }

    pub fn nord() -> Self {
        Self {
            bg: Color::Rgb(46, 52, 64),
            fg: Color::Rgb(216, 222, 233),
            fg_dim: Color::Rgb(76, 86, 106),
            fg_muted: Color::Rgb(96, 107, 128),
            separator: Color::Rgb(59, 66, 82),
            accent_warm: Color::Rgb(208, 135, 112),
            accent_green: Color::Rgb(163, 190, 140),
            accent_red: Color::Rgb(191, 97, 106),
            accent_blue: Color::Rgb(136, 192, 208),
            accent_purple: Color::Rgb(180, 142, 173),
            bar_filled: Color::Rgb(136, 192, 208),
            bar_empty: Color::Rgb(59, 66, 82),
            agent_colors: [
                Color::Rgb(208, 135, 112),
                Color::Rgb(163, 190, 140),
                Color::Rgb(136, 192, 208),
                Color::Rgb(191, 97, 106),
                Color::Rgb(180, 142, 173),
                Color::Rgb(143, 188, 187),
            ],
        }
    }

    pub fn kanagawa() -> Self {
        Self {
            bg: Color::Rgb(31, 31, 40),
            fg: Color::Rgb(220, 215, 186),
            fg_dim: Color::Rgb(84, 88, 98),
            fg_muted: Color::Rgb(114, 117, 126),
            separator: Color::Rgb(54, 54, 70),
            accent_warm: Color::Rgb(255, 160, 102),
            accent_green: Color::Rgb(152, 195, 121),
            accent_red: Color::Rgb(195, 64, 67),
            accent_blue: Color::Rgb(126, 156, 216),
            accent_purple: Color::Rgb(149, 127, 184),
            bar_filled: Color::Rgb(126, 156, 216),
            bar_empty: Color::Rgb(43, 43, 58),
            agent_colors: [
                Color::Rgb(255, 160, 102),
                Color::Rgb(152, 195, 121),
                Color::Rgb(126, 156, 216),
                Color::Rgb(195, 64, 67),
                Color::Rgb(149, 127, 184),
                Color::Rgb(122, 200, 195),
            ],
        }
    }

    pub fn rose_pine() -> Self {
        Self {
            bg: Color::Rgb(25, 23, 36),
            fg: Color::Rgb(224, 222, 244),
            fg_dim: Color::Rgb(110, 106, 134),
            fg_muted: Color::Rgb(144, 140, 170),
            separator: Color::Rgb(57, 53, 82),
            accent_warm: Color::Rgb(246, 193, 119),
            accent_green: Color::Rgb(156, 207, 216),
            accent_red: Color::Rgb(235, 111, 146),
            accent_blue: Color::Rgb(49, 116, 143),
            accent_purple: Color::Rgb(196, 167, 231),
            bar_filled: Color::Rgb(196, 167, 231),
            bar_empty: Color::Rgb(38, 35, 58),
            agent_colors: [
                Color::Rgb(246, 193, 119),
                Color::Rgb(156, 207, 216),
                Color::Rgb(196, 167, 231),
                Color::Rgb(235, 111, 146),
                Color::Rgb(49, 116, 143),
                Color::Rgb(235, 188, 186),
            ],
        }
    }

    pub fn one_dark() -> Self {
        Self {
            bg: Color::Rgb(40, 44, 52),
            fg: Color::Rgb(171, 178, 191),
            fg_dim: Color::Rgb(76, 82, 99),
            fg_muted: Color::Rgb(92, 99, 112),
            separator: Color::Rgb(60, 65, 77),
            accent_warm: Color::Rgb(209, 154, 102),
            accent_green: Color::Rgb(152, 195, 121),
            accent_red: Color::Rgb(224, 108, 117),
            accent_blue: Color::Rgb(97, 175, 239),
            accent_purple: Color::Rgb(198, 120, 221),
            bar_filled: Color::Rgb(97, 175, 239),
            bar_empty: Color::Rgb(50, 55, 66),
            agent_colors: [
                Color::Rgb(209, 154, 102),
                Color::Rgb(152, 195, 121),
                Color::Rgb(97, 175, 239),
                Color::Rgb(224, 108, 117),
                Color::Rgb(198, 120, 221),
                Color::Rgb(86, 182, 194),
            ],
        }
    }

    pub fn solarized_dark() -> Self {
        Self {
            bg: Color::Rgb(0, 43, 54),
            fg: Color::Rgb(131, 148, 150),
            fg_dim: Color::Rgb(88, 110, 117),
            fg_muted: Color::Rgb(101, 123, 131),
            separator: Color::Rgb(7, 54, 66),
            accent_warm: Color::Rgb(203, 75, 22),
            accent_green: Color::Rgb(133, 153, 0),
            accent_red: Color::Rgb(220, 50, 47),
            accent_blue: Color::Rgb(38, 139, 210),
            accent_purple: Color::Rgb(108, 113, 196),
            bar_filled: Color::Rgb(38, 139, 210),
            bar_empty: Color::Rgb(7, 54, 66),
            agent_colors: [
                Color::Rgb(203, 75, 22),
                Color::Rgb(133, 153, 0),
                Color::Rgb(38, 139, 210),
                Color::Rgb(220, 50, 47),
                Color::Rgb(108, 113, 196),
                Color::Rgb(42, 161, 152),
            ],
        }
    }

    pub fn everforest_dark() -> Self {
        Self {
            bg: Color::Rgb(47, 53, 55),
            fg: Color::Rgb(211, 198, 170),
            fg_dim: Color::Rgb(113, 119, 109),
            fg_muted: Color::Rgb(135, 142, 129),
            separator: Color::Rgb(70, 77, 78),
            accent_warm: Color::Rgb(219, 188, 127),
            accent_green: Color::Rgb(167, 192, 128),
            accent_red: Color::Rgb(230, 126, 128),
            accent_blue: Color::Rgb(127, 187, 179),
            accent_purple: Color::Rgb(214, 153, 182),
            bar_filled: Color::Rgb(167, 192, 128),
            bar_empty: Color::Rgb(59, 66, 67),
            agent_colors: [
                Color::Rgb(219, 188, 127),
                Color::Rgb(167, 192, 128),
                Color::Rgb(127, 187, 179),
                Color::Rgb(230, 126, 128),
                Color::Rgb(214, 153, 182),
                Color::Rgb(131, 192, 146),
            ],
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
            agent_colors: [
                Color::Rgb(150, 100, 50),
                Color::Rgb(40, 120, 60),
                Color::Rgb(40, 80, 150),
                Color::Rgb(180, 50, 50),
                Color::Rgb(120, 70, 180),
                Color::Rgb(30, 130, 130),
            ],
        }
    }

    pub fn catppuccin_latte() -> Self {
        Self {
            bg: Color::Rgb(239, 241, 245),
            fg: Color::Rgb(76, 79, 105),
            fg_dim: Color::Rgb(172, 176, 190),
            fg_muted: Color::Rgb(140, 143, 161),
            separator: Color::Rgb(204, 208, 218),
            accent_warm: Color::Rgb(254, 100, 11),
            accent_green: Color::Rgb(64, 160, 43),
            accent_red: Color::Rgb(210, 15, 57),
            accent_blue: Color::Rgb(30, 102, 245),
            accent_purple: Color::Rgb(136, 57, 239),
            bar_filled: Color::Rgb(30, 102, 245),
            bar_empty: Color::Rgb(220, 224, 232),
            agent_colors: [
                Color::Rgb(254, 100, 11),
                Color::Rgb(64, 160, 43),
                Color::Rgb(30, 102, 245),
                Color::Rgb(210, 15, 57),
                Color::Rgb(136, 57, 239),
                Color::Rgb(23, 146, 153),
            ],
        }
    }

    pub fn gruvbox_light() -> Self {
        Self {
            bg: Color::Rgb(251, 241, 199),
            fg: Color::Rgb(60, 56, 54),
            fg_dim: Color::Rgb(189, 174, 147),
            fg_muted: Color::Rgb(146, 131, 116),
            separator: Color::Rgb(213, 196, 161),
            accent_warm: Color::Rgb(215, 95, 0),
            accent_green: Color::Rgb(121, 116, 14),
            accent_red: Color::Rgb(204, 36, 29),
            accent_blue: Color::Rgb(69, 133, 136),
            accent_purple: Color::Rgb(177, 98, 134),
            bar_filled: Color::Rgb(69, 133, 136),
            bar_empty: Color::Rgb(235, 219, 178),
            agent_colors: [
                Color::Rgb(215, 95, 0),
                Color::Rgb(121, 116, 14),
                Color::Rgb(69, 133, 136),
                Color::Rgb(204, 36, 29),
                Color::Rgb(177, 98, 134),
                Color::Rgb(152, 151, 26),
            ],
        }
    }

    pub fn solarized_light() -> Self {
        Self {
            bg: Color::Rgb(253, 246, 227),
            fg: Color::Rgb(101, 123, 131),
            fg_dim: Color::Rgb(147, 161, 161),
            fg_muted: Color::Rgb(131, 148, 150),
            separator: Color::Rgb(238, 232, 213),
            accent_warm: Color::Rgb(203, 75, 22),
            accent_green: Color::Rgb(133, 153, 0),
            accent_red: Color::Rgb(220, 50, 47),
            accent_blue: Color::Rgb(38, 139, 210),
            accent_purple: Color::Rgb(108, 113, 196),
            bar_filled: Color::Rgb(38, 139, 210),
            bar_empty: Color::Rgb(238, 232, 213),
            agent_colors: [
                Color::Rgb(203, 75, 22),
                Color::Rgb(133, 153, 0),
                Color::Rgb(38, 139, 210),
                Color::Rgb(220, 50, 47),
                Color::Rgb(108, 113, 196),
                Color::Rgb(42, 161, 152),
            ],
        }
    }

    pub fn rose_pine_dawn() -> Self {
        Self {
            bg: Color::Rgb(250, 244, 237),
            fg: Color::Rgb(87, 82, 121),
            fg_dim: Color::Rgb(152, 147, 165),
            fg_muted: Color::Rgb(121, 117, 147),
            separator: Color::Rgb(242, 233, 222),
            accent_warm: Color::Rgb(234, 157, 52),
            accent_green: Color::Rgb(86, 148, 159),
            accent_red: Color::Rgb(180, 99, 122),
            accent_blue: Color::Rgb(40, 105, 131),
            accent_purple: Color::Rgb(144, 122, 169),
            bar_filled: Color::Rgb(144, 122, 169),
            bar_empty: Color::Rgb(242, 233, 222),
            agent_colors: [
                Color::Rgb(234, 157, 52),
                Color::Rgb(86, 148, 159),
                Color::Rgb(144, 122, 169),
                Color::Rgb(180, 99, 122),
                Color::Rgb(40, 105, 131),
                Color::Rgb(215, 130, 126),
            ],
        }
    }

    pub fn everforest_light() -> Self {
        Self {
            bg: Color::Rgb(253, 246, 227),
            fg: Color::Rgb(92, 100, 83),
            fg_dim: Color::Rgb(167, 172, 156),
            fg_muted: Color::Rgb(135, 142, 129),
            separator: Color::Rgb(228, 218, 192),
            accent_warm: Color::Rgb(223, 159, 40),
            accent_green: Color::Rgb(141, 153, 36),
            accent_red: Color::Rgb(241, 109, 107),
            accent_blue: Color::Rgb(53, 132, 124),
            accent_purple: Color::Rgb(214, 153, 182),
            bar_filled: Color::Rgb(141, 153, 36),
            bar_empty: Color::Rgb(239, 229, 206),
            agent_colors: [
                Color::Rgb(223, 159, 40),
                Color::Rgb(141, 153, 36),
                Color::Rgb(53, 132, 124),
                Color::Rgb(241, 109, 107),
                Color::Rgb(214, 153, 182),
                Color::Rgb(104, 157, 106),
            ],
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_PRESETS: &[&str] = &[
        "dark",
        "catppuccin-mocha",
        "catppuccin-macchiato",
        "gruvbox-dark",
        "tokyo-night",
        "tokyo-night-storm",
        "dracula",
        "nord",
        "kanagawa",
        "rose-pine",
        "one-dark",
        "solarized-dark",
        "everforest-dark",
        "light",
        "catppuccin-latte",
        "gruvbox-light",
        "solarized-light",
        "rose-pine-dawn",
        "everforest-light",
    ];

    #[test]
    fn test_all_presets_load() {
        for name in ALL_PRESETS {
            let theme = Theme::from_preset(name);
            assert_eq!(
                theme.agent_colors.len(),
                6,
                "theme {} missing agent_colors",
                name
            );
        }
    }

    #[test]
    fn test_unknown_preset_falls_back_to_dark() {
        let theme = Theme::from_preset("nonexistent");
        let dark = Theme::dark();
        assert_eq!(theme.bg, dark.bg);
    }
}
