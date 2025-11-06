// Catppuccin Mocha inspired color theme for qdum-vault
// Provides a cohesive, professional dark theme with semantic color meanings

use ratatui::style::Color;

pub struct Theme;

impl Theme {
    // Base colors - Background layers
    pub const BASE: Color = Color::Rgb(30, 30, 46);         // Main background
    pub const MANTLE: Color = Color::Rgb(24, 24, 37);       // Slightly darker
    pub const CRUST: Color = Color::Rgb(17, 17, 27);        // Darkest background

    // Surface layers - For panels and cards
    pub const SURFACE0: Color = Color::Rgb(49, 50, 68);     // Elevated surface
    pub const SURFACE1: Color = Color::Rgb(69, 71, 90);     // More elevated
    pub const SURFACE2: Color = Color::Rgb(88, 91, 112);    // Highest elevation

    // Text colors - Semantic hierarchy
    pub const TEXT: Color = Color::Rgb(205, 214, 244);      // Primary text
    pub const SUBTEXT1: Color = Color::Rgb(186, 194, 222);  // Secondary text
    pub const SUBTEXT0: Color = Color::Rgb(166, 173, 200);  // Tertiary text
    pub const OVERLAY2: Color = Color::Rgb(147, 153, 178);  // Muted text
    pub const OVERLAY1: Color = Color::Rgb(127, 132, 156);  // More muted

    // Accent colors - Semantic meanings
    pub const BLUE: Color = Color::Rgb(137, 180, 250);      // Info, downloads, data flow
    pub const SAPPHIRE: Color = Color::Rgb(116, 199, 236);  // Links, interactive
    pub const SKY: Color = Color::Rgb(137, 220, 235);       // Highlights
    pub const TEAL: Color = Color::Rgb(148, 226, 213);      // Success accent
    pub const GREEN: Color = Color::Rgb(166, 227, 161);     // Success, uploads, positive
    pub const YELLOW: Color = Color::Rgb(249, 226, 175);    // Warning, selection, focus
    pub const PEACH: Color = Color::Rgb(250, 179, 135);     // Emphasis, important
    pub const MAROON: Color = Color::Rgb(238, 153, 160);    // Error accent
    pub const RED: Color = Color::Rgb(243, 139, 168);       // Error, critical, locked
    pub const MAUVE: Color = Color::Rgb(203, 166, 247);     // Active, quantum theme
    pub const PINK: Color = Color::Rgb(245, 194, 231);      // Special, decorative
    pub const FLAMINGO: Color = Color::Rgb(242, 205, 205);  // Soft accent
    pub const ROSEWATER: Color = Color::Rgb(245, 224, 220); // Subtle accent

    // Semantic color functions

    /// Border color for active/focused panels
    pub const fn active_border() -> Color {
        Self::MAUVE
    }

    /// Border color for inactive panels
    pub const fn inactive_border() -> Color {
        Self::SURFACE1
    }

    /// Color for success states (unlocked, completed)
    pub const fn success() -> Color {
        Self::GREEN
    }

    /// Color for error states
    pub const fn error() -> Color {
        Self::RED
    }

    /// Color for warning states
    pub const fn warning() -> Color {
        Self::YELLOW
    }

    /// Color for info/neutral states
    pub const fn info() -> Color {
        Self::BLUE
    }

    /// Color for quantum/crypto operations
    pub const fn quantum() -> Color {
        Self::MAUVE
    }

    /// Color for locked state
    pub const fn locked() -> Color {
        Self::RED
    }

    /// Color for unlocked state
    pub const fn unlocked() -> Color {
        Self::GREEN
    }

    /// Color for selected/focused input
    pub const fn selection() -> Color {
        Self::YELLOW
    }

    /// Color for progress/in-progress operations
    pub const fn progress() -> Color {
        Self::SAPPHIRE
    }
}
