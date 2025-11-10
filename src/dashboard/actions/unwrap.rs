use crate::dashboard::types::{Dashboard, AppMode};

impl Dashboard {
    pub fn execute_unwrap(&mut self) {
        self.mode = AppMode::UnwrapPopup;  // Keep popup for now as it needs input
        self.needs_clear = true;
        self.bridge_amount.clear();
        self.status_message = Some("Enter unwrap amount...".to_string());
    }
}
