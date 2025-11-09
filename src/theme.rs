// Glassmorphic tech-inspired theme for qdum-vault
// Modern design with layered depth, vibrant accents, and clean aesthetics

use ratatui::style::Color;

pub struct Theme;

impl Theme {
    // Base colors - Deep tech backgrounds with subtle gradients
    pub const BASE: Color = Color::Rgb(8, 8, 18);           // Deep space black-blue
    pub const MANTLE: Color = Color::Rgb(12, 12, 24);       // Slightly elevated
    pub const CRUST: Color = Color::Rgb(4, 4, 12);          // Absolute black

    // Glassmorphic panel backgrounds - Layered depth effect
    pub const PANEL_BG: Color = Color::Rgb(14, 14, 28);     // Main glass panels
    pub const GLASS_1: Color = Color::Rgb(18, 18, 32);      // First glass layer
    pub const GLASS_2: Color = Color::Rgb(22, 22, 38);      // Second glass layer
    pub const GLASS_3: Color = Color::Rgb(26, 26, 44);      // Third glass layer
    pub const GLASS_ACCENT: Color = Color::Rgb(28, 32, 48); // Glass with blue tint

    // Surface layers - For elevated elements with depth
    pub const SURFACE0: Color = Color::Rgb(20, 24, 36);     // Elevated surface
    pub const SURFACE1: Color = Color::Rgb(24, 28, 42);     // More elevated
    pub const SURFACE2: Color = Color::Rgb(28, 32, 48);     // Highest elevation

    // Text colors - Crystal clear with hierarchy
    pub const TEXT: Color = Color::Rgb(255, 255, 255);      // Pure white primary
    pub const TEXT_BRIGHT: Color = Color::Rgb(240, 245, 255); // Bright white with blue tint
    pub const SUBTEXT1: Color = Color::Rgb(200, 210, 230);  // Primary gray
    pub const SUBTEXT0: Color = Color::Rgb(160, 170, 200);  // Secondary gray
    pub const OVERLAY2: Color = Color::Rgb(120, 130, 160);  // Tertiary gray
    pub const OVERLAY1: Color = Color::Rgb(80, 90, 120);    // Muted gray
    pub const DIM: Color = Color::Rgb(50, 55, 75);          // Very dim

    // Vibrant neon accents - Modern tech palette
    pub const CYAN_NEON: Color = Color::Rgb(0, 255, 255);   // Electric cyan
    pub const CYAN_BRIGHT: Color = Color::Rgb(80, 255, 255); // Bright cyan glow
    pub const BLUE_NEON: Color = Color::Rgb(50, 150, 255);  // Tech blue
    pub const PURPLE_NEON: Color = Color::Rgb(180, 80, 255); // Quantum purple
    pub const PINK_NEON: Color = Color::Rgb(255, 80, 200);  // Electric pink
    pub const GREEN_NEON: Color = Color::Rgb(80, 255, 150); // Matrix green
    pub const RED_NEON: Color = Color::Rgb(255, 80, 120);   // Alert red
    pub const YELLOW_NEON: Color = Color::Rgb(255, 220, 0); // Warning yellow
    pub const ORANGE_NEON: Color = Color::Rgb(255, 160, 50); // Accent orange

    // Softer accent colors - For subtle highlights
    pub const CYAN: Color = Color::Rgb(60, 200, 220);       // Soft cyan
    pub const BLUE: Color = Color::Rgb(100, 160, 255);      // Soft blue
    pub const PURPLE: Color = Color::Rgb(160, 100, 255);    // Soft purple
    pub const GREEN: Color = Color::Rgb(100, 220, 160);     // Soft green
    pub const RED: Color = Color::Rgb(255, 100, 140);       // Soft red
    pub const YELLOW: Color = Color::Rgb(255, 200, 100);    // Soft yellow

    // Border colors - Layered glass effect
    pub const BORDER_BRIGHT: Color = Color::Rgb(80, 120, 180); // Active bright border
    pub const BORDER_DIM: Color = Color::Rgb(40, 50, 70);      // Inactive dim border
    pub const BORDER_GLOW: Color = Color::Rgb(0, 200, 255);    // Glowing border accent

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
