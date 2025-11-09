use arboard::Clipboard;
use crate::dashboard::types::Dashboard;

impl Dashboard {
    pub fn get_animated_dots(&self) -> &'static str {
        match self.animation_frame % 4 {
            0 => "   ",
            1 => ".  ",
            2 => ".. ",
            3 => "...",
            _ => "   ",
        }
    }
    pub fn get_pulse_intensity(&self) -> u8 {
        let phase = (self.animation_frame % 20) as f32 / 20.0;
        let pulse = ((phase * std::f32::consts::PI * 2.0).sin() + 1.0) / 2.0;
        (pulse * 155.0 + 100.0) as u8  // Range: 100-255 (much wider range)
    }
    pub fn get_pulse_color_bright(&self) -> bool {
        (self.animation_frame / 10) % 2 == 0
    }
    pub fn copy_wallet_to_clipboard(&mut self) {
        match Clipboard::new() {
            Ok(mut clipboard) => {
                let wallet_str = self.wallet.to_string();
                match clipboard.set_text(wallet_str) {
                    Ok(_) => {
                        self.status_message = Some("✓ Wallet address copied to clipboard!".to_string());
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Failed to copy to clipboard: {}", e));
                    }
                }
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to access clipboard: {}", e));
            }
        }
    }
}
