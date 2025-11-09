use crate::dashboard::types::{Dashboard, AppMode};

impl Dashboard {
    pub fn execute_wrap(&mut self) {
        self.mode = AppMode::WrapPopup;
        self.needs_clear = true;
        self.bridge_amount.clear();
        self.status_message = Some("Enter wrap amount...".to_string());
    }
}
