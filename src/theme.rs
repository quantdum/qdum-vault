// Cyberpunk-inspired color theme for qdum-vault
// Dark backgrounds with neon accents for a futuristic crypto aesthetic

use ratatui::style::Color;

pub struct Theme;

impl Theme {
    // Base colors - Dark cyberpunk backgrounds
    pub const BASE: Color = Color::Rgb(10, 10, 20);         // Very dark blue-black
    pub const MANTLE: Color = Color::Rgb(15, 15, 25);       // Slightly lighter
    pub const CRUST: Color = Color::Rgb(5, 5, 15);          // Darkest background
    pub const PANEL_BG: Color = Color::Rgb(12, 12, 22);     // Panel background

    // Surface layers - For elevated elements
    pub const SURFACE0: Color = Color::Rgb(20, 20, 35);     // Elevated surface
    pub const SURFACE1: Color = Color::Rgb(25, 25, 40);     // More elevated
    pub const SURFACE2: Color = Color::Rgb(30, 30, 45);     // Highest elevation

    // Text colors - High contrast for readability
    pub const TEXT: Color = Color::Rgb(255, 255, 255);      // Pure white primary text
    pub const SUBTEXT1: Color = Color::Rgb(220, 220, 230);  // Light gray secondary
    pub const SUBTEXT0: Color = Color::Rgb(180, 180, 200);  // Medium gray tertiary
    pub const OVERLAY2: Color = Color::Rgb(140, 140, 160);  // Muted text
    pub const OVERLAY1: Color = Color::Rgb(100, 100, 120);  // Very muted
    pub const DIM: Color = Color::Rgb(60, 60, 80);          // Dim/disabled text

    // Neon accent colors - Cyberpunk aesthetic
    pub const CYAN_NEON: Color = Color::Rgb(0, 255, 255);   // Bright cyan - headers
    pub const BLUE_NEON: Color = Color::Rgb(0, 150, 255);   // Blue - info
    pub const PURPLE_NEON: Color = Color::Rgb(180, 0, 255); // Purple - quantum/crypto
    pub const PINK_NEON: Color = Color::Rgb(255, 0, 180);   // Pink - special
    pub const GREEN_NEON: Color = Color::Rgb(0, 255, 150);  // Green - success/unlocked
    pub const RED_NEON: Color = Color::Rgb(255, 0, 100);    // Red - error/locked
    pub const YELLOW_NEON: Color = Color::Rgb(255, 255, 0); // Yellow - warning/selection
    pub const ORANGE_NEON: Color = Color::Rgb(255, 150, 0); // Orange - emphasis

    // Muted versions for less aggressive use
    pub const CYAN: Color = Color::Rgb(0, 200, 200);        // Muted cyan
    pub const BLUE: Color = Color::Rgb(80, 150, 255);       // Muted blue
    pub const PURPLE: Color = Color::Rgb(150, 80, 255);     // Muted purple
    pub const GREEN: Color = Color::Rgb(80, 255, 150);      // Muted green
    pub const RED: Color = Color::Rgb(255, 80, 120);        // Muted red
    pub const YELLOW: Color = Color::Rgb(255, 220, 80);     // Muted yellow

    // Semantic color functions

    /// Border color for active/focused panels
    pub const fn active_border() -> Color {
        Self::CYAN_NEON
    }

    /// Border color for inactive panels
    pub const fn inactive_border() -> Color {
        Self::SURFACE1
    }

    /// Color for table/section headers
    pub const fn header() -> Color {
        Self::CYAN_NEON
    }

    /// Color for success states (unlocked, completed)
    pub const fn success() -> Color {
        Self::GREEN_NEON
    }

    /// Color for error states
    pub const fn error() -> Color {
        Self::RED_NEON
    }

    /// Color for warning states
    pub const fn warning() -> Color {
        Self::YELLOW_NEON
    }

    /// Color for info/neutral states
    pub const fn info() -> Color {
        Self::BLUE_NEON
    }

    /// Color for quantum/crypto operations
    pub const fn quantum() -> Color {
        Self::PURPLE_NEON
    }

    /// Color for locked state
    pub const fn locked() -> Color {
        Self::RED_NEON
    }

    /// Color for unlocked state
    pub const fn unlocked() -> Color {
        Self::GREEN_NEON
    }

    /// Color for selected/focused input
    pub const fn selection() -> Color {
        Self::YELLOW_NEON
    }

    /// Color for progress/in-progress operations
    pub const fn progress() -> Color {
        Self::CYAN
    }

    /// Dim/muted border for sections
    pub const fn section_border() -> Color {
        Self::SURFACE2
    }
}
