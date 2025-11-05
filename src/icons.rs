// ASCII art icons that work in all terminals
// These are multi-character icons using box-drawing and ASCII

pub struct Icons;

impl Icons {
    // Action icons using box-drawing
    pub const WALLET: &'static str = "[W]";
    pub const LOCK: &'static str = "[X]";
    pub const UNLOCK: &'static str = "[O]";
    pub const TRANSFER: &'static str = "[→]";
    pub const REGISTER: &'static str = "[+]";

    // Status icons
    pub const LOCKED_STATUS: &'static str = "[X]";
    pub const UNLOCKED_STATUS: &'static str = "[O]";
    pub const LOADING: &'static str = "[~]";

    // Info icons
    pub const BALANCE: &'static str = "[$]";
    pub const ALGORITHM: &'static str = "[#]";
    pub const SECURITY: &'static str = "[!]";
    pub const NETWORK: &'static str = "[N]";

    // UI icons
    pub const MENU: &'static str = "[≡]";
    pub const KEYBOARD: &'static str = "[K]";
    pub const ARROW_RIGHT: &'static str = ">";
    pub const INFO: &'static str = "[i]";
    pub const QUANTUM: &'static str = "[Q]";
}
