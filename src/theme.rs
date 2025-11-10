// Post-Quantum Terminal theme for pqcoin
// Clean, modern white background design with bold typography

use ratatui::style::Color;

pub struct Theme;

impl Theme {
    // Base colors - Clean white background
    pub const BASE: Color = Color::Rgb(255, 255, 255);      // Pure white background
    pub const MANTLE: Color = Color::Rgb(250, 250, 250);    // Light gray
    pub const CRUST: Color = Color::Rgb(245, 245, 245);     // Slightly darker gray

    // Panel backgrounds - Light modern panels
    pub const PANEL_BG: Color = Color::Rgb(248, 248, 248);  // Main panels
    pub const GLASS_1: Color = Color::Rgb(245, 245, 250);   // Panel layer 1 (slight blue tint)
    pub const GLASS_2: Color = Color::Rgb(240, 240, 245);   // Panel layer 2
    pub const GLASS_3: Color = Color::Rgb(235, 235, 240);   // Panel layer 3
    pub const GLASS_ACCENT: Color = Color::Rgb(230, 230, 235); // Elevated panels

    // Surface layers - For table headers and elevated sections
    pub const SURFACE0: Color = Color::Rgb(235, 235, 235);  // Elevated surface
    pub const SURFACE1: Color = Color::Rgb(230, 230, 230);  // More elevated
    pub const SURFACE2: Color = Color::Rgb(225, 225, 225);  // Highest elevation

    // Text colors - Dark text on white background
    pub const TEXT: Color = Color::Rgb(20, 20, 20);         // Almost black text
    pub const TEXT_BRIGHT: Color = Color::Rgb(0, 0, 0);     // Pure black
    pub const SUBTEXT1: Color = Color::Rgb(60, 60, 60);     // Dark gray
    pub const SUBTEXT0: Color = Color::Rgb(100, 100, 100);  // Medium gray
    pub const OVERLAY2: Color = Color::Rgb(140, 140, 140);  // Light gray
    pub const OVERLAY1: Color = Color::Rgb(180, 180, 180);  // Very light gray
    pub const DIM: Color = Color::Rgb(200, 200, 200);       // Dim separator

    // Post-Quantum Signature Colors - Dark versions for white background
    pub const QUANTUM_CYAN: Color = Color::Rgb(0, 150, 200);      // Dark cyan
    pub const QUANTUM_CYAN_DIM: Color = Color::Rgb(0, 120, 160);  // Darker cyan
    pub const QUANTUM_MAGENTA: Color = Color::Rgb(180, 0, 200);   // Dark magenta
    pub const QUANTUM_MAGENTA_DIM: Color = Color::Rgb(140, 0, 160); // Darker magenta

    // Primary Purple Theme - Dark purple for white background
    pub const BLOOMBERG_ORANGE: Color = Color::Rgb(120, 60, 200); // Dark purple
    pub const ORANGE_BRIGHT: Color = Color::Rgb(140, 80, 220);    // Medium purple
    pub const ORANGE_DIM: Color = Color::Rgb(100, 50, 180);       // Darker purple
    pub const ORANGE_DARK: Color = Color::Rgb(80, 40, 160);       // Darkest purple

    // Accent colors - Darker versions for white background visibility
    pub const CYAN_NEON: Color = Color::Rgb(0, 140, 180);   // Dark cyan
    pub const CYAN_BRIGHT: Color = Color::Rgb(0, 160, 200); // Medium cyan
    pub const BLUE_NEON: Color = Color::Rgb(30, 100, 200);  // Dark blue
    pub const PURPLE_NEON: Color = Color::Rgb(140, 60, 200); // Dark purple
    pub const PINK_NEON: Color = Color::Rgb(200, 40, 140);   // Dark pink
    pub const GREEN_NEON: Color = Color::Rgb(0, 160, 80);    // Dark green
    pub const RED_NEON: Color = Color::Rgb(220, 50, 50);     // Dark red
    pub const YELLOW_NEON: Color = Color::Rgb(200, 140, 0);  // Dark yellow/gold
    pub const ORANGE_NEON: Color = Color::Rgb(120, 60, 200); // Dark purple

    // Softer accent colors - For secondary data
    pub const CYAN: Color = Color::Rgb(0, 120, 160);        // Medium cyan
    pub const BLUE: Color = Color::Rgb(40, 100, 180);       // Medium blue
    pub const PURPLE: Color = Color::Rgb(120, 80, 200);     // Medium purple
    pub const GREEN: Color = Color::Rgb(40, 160, 80);       // Medium green
    pub const RED: Color = Color::Rgb(200, 70, 70);         // Medium red
    pub const YELLOW: Color = Color::Rgb(180, 120, 0);      // Medium yellow

    // Border colors - Dark borders for white background
    pub const BORDER_BRIGHT: Color = Color::Rgb(120, 60, 200);  // Dark purple border
    pub const BORDER_DIM: Color = Color::Rgb(200, 200, 200);    // Light gray border
    pub const BORDER_GLOW: Color = Color::Rgb(140, 80, 220);    // Medium purple glow
    pub const BORDER_MAGENTA: Color = Color::Rgb(180, 0, 200);  // Dark magenta border

    // Semantic color functions - Modern glassmorphic style

    /// Border color for active/focused panels - Bright glow
    pub const fn active_border() -> Color {
        Self::BORDER_GLOW
    }

    /// Border color for inactive panels - Subtle depth
    pub const fn inactive_border() -> Color {
        Self::BORDER_DIM
    }

    /// Color for table/section headers - Bright cyan
    pub const fn header() -> Color {
        Self::CYAN_BRIGHT
    }

    /// Color for success states (unlocked, completed) - Matrix green
    pub const fn success() -> Color {
        Self::GREEN_NEON
    }

    /// Color for error states - Alert red
    pub const fn error() -> Color {
        Self::RED_NEON
    }

    /// Color for warning states - Warning yellow
    pub const fn warning() -> Color {
        Self::YELLOW_NEON
    }

    /// Color for info/neutral states - Tech blue
    pub const fn info() -> Color {
        Self::BLUE_NEON
    }

    /// Color for quantum/crypto operations - Quantum purple
    pub const fn quantum() -> Color {
        Self::PURPLE_NEON
    }

    /// Color for locked state - Alert red
    pub const fn locked() -> Color {
        Self::RED_NEON
    }

    /// Color for unlocked state - Matrix green
    pub const fn unlocked() -> Color {
        Self::GREEN_NEON
    }

    /// Color for selected/focused input - Warning yellow
    pub const fn selection() -> Color {
        Self::YELLOW_NEON
    }

    /// Color for progress/in-progress operations - Soft cyan
    pub const fn progress() -> Color {
        Self::CYAN
    }

    /// Dim border for subtle sections
    pub const fn section_border() -> Color {
        Self::BORDER_DIM
    }

    /// Primary glass panel background
    pub const fn glass_panel() -> Color {
        Self::GLASS_1
    }

    /// Secondary glass layer (more elevated)
    pub const fn glass_elevated() -> Color {
        Self::GLASS_2
    }

    /// Tertiary glass layer (highest elevation)
    pub const fn glass_top() -> Color {
        Self::GLASS_3
    }
}
