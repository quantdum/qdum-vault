use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VaultProfile {
    /// Unique name for this vault
    pub name: String,

    /// Optional description
    pub description: Option<String>,

    /// Path to Solana keypair JSON
    pub solana_keypair_path: String,

    /// Path to SPHINCS+ public key
    pub sphincs_public_key_path: String,

    /// Path to SPHINCS+ private key
    pub sphincs_private_key_path: String,

    /// Wallet address (cached for quick display)
    pub wallet_address: String,

    /// When this vault was created
    pub created_at: String,

    /// Last used timestamp
    pub last_used: Option<String>,
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct VaultConfig {
    /// Active vault name
    pub active_vault: Option<String>,

    /// All vault profiles
    pub vaults: HashMap<String, VaultProfile>,

    /// Config version (for future migrations)
    pub version: u32,
}

impl VaultConfig {
    /// Load vault config from disk
    pub fn load() -> Result<Self> {
        let config_path = Self::get_config_path();

        if !config_path.exists() {
            // Try to migrate from old config
            return Self::migrate_from_old_config();
        }

        let data = fs::read_to_string(&config_path)
            .context("Failed to read vault config")?;

        let config: VaultConfig = serde_json::from_str(&data)
            .context("Failed to parse vault config")?;

        Ok(config)
    }

    /// Save vault config to disk
    pub fn save(&self) -> Result<()> {
        let config_path = Self::get_config_path();

        // Ensure .qdum directory exists
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .context("Failed to create .qdum directory")?;
        }

        let json = serde_json::to_string_pretty(self)
            .context("Failed to serialize vault config")?;

        fs::write(&config_path, json)
            .context("Failed to write vault config")?;

        Ok(())
    }

    /// Get path to vault config file
    fn get_config_path() -> PathBuf {
        let home = dirs::home_dir().expect("Could not determine home directory");
        home.join(".qdum").join("vaults.json")
    }

    /// Get path to old config file
    fn get_old_config_path() -> PathBuf {
        let home = dirs::home_dir().expect("Could not determine home directory");
        home.join(".qdum").join("config.json")
    }

    /// Migrate from old config format
    fn migrate_from_old_config() -> Result<Self> {
        let old_config_path = Self::get_old_config_path();

        if !old_config_path.exists() {
            // No old config, return empty config
            return Ok(VaultConfig {
                version: 1,
                ..Default::default()
            });
        }

        // Read old config
        #[derive(Deserialize)]
        struct OldConfig {
            keypair_path: Option<String>,
        }

        let data = fs::read_to_string(&old_config_path)
            .context("Failed to read old config")?;

        let old_config: OldConfig = serde_json::from_str(&data)
            .context("Failed to parse old config")?;

        // Create default vault from old config
        let mut config = VaultConfig {
            version: 1,
            active_vault: Some("default".to_string()),
            vaults: HashMap::new(),
        };

        if let Some(keypair_path) = old_config.keypair_path {
            let home = dirs::home_dir().expect("Could not determine home directory");
            let qdum_dir = home.join(".qdum");

            let profile = VaultProfile {
                name: "default".to_string(),
                description: Some("Auto-migrated from old config".to_string()),
                solana_keypair_path: keypair_path,
                sphincs_public_key_path: qdum_dir.join("sphincs_public.key")
                    .to_str().unwrap().to_string(),
                sphincs_private_key_path: qdum_dir.join("sphincs_private.key")
                    .to_str().unwrap().to_string(),
                wallet_address: String::new(), // Will be populated on first use
                created_at: Utc::now().to_rfc3339(),
                last_used: Some(Utc::now().to_rfc3339()),
            };

            config.vaults.insert("default".to_string(), profile);
        }

        // Save new config
        config.save()?;

        // Rename old config to .bak
        let backup_path = old_config_path.with_extension("json.bak");
        let _ = fs::rename(&old_config_path, &backup_path);

        Ok(config)
    }

    /// Create a new vault profile
    pub fn create_vault(&mut self, name: String, profile: VaultProfile) -> Result<()> {
        if self.vaults.contains_key(&name) {
            return Err(anyhow!("Vault '{}' already exists", name));
        }

        self.vaults.insert(name.clone(), profile);

        // If this is the first vault, make it active
        if self.active_vault.is_none() {
            self.active_vault = Some(name);
        }

        self.save()?;

        Ok(())
    }

    /// Switch active vault
    pub fn switch_vault(&mut self, name: &str) -> Result<()> {
        if !self.vaults.contains_key(name) {
            return Err(anyhow!("Vault '{}' does not exist", name));
        }

        self.active_vault = Some(name.to_string());

        // Update last_used timestamp
        if let Some(vault) = self.vaults.get_mut(name) {
            vault.last_used = Some(Utc::now().to_rfc3339());
        }

        self.save()?;

        Ok(())
    }

    /// Get active vault profile
    pub fn get_active_vault(&self) -> Option<&VaultProfile> {
        if let Some(active_name) = &self.active_vault {
            return self.vaults.get(active_name);
        }
        None
    }

    /// Get mutable active vault profile
    pub fn get_active_vault_mut(&mut self) -> Option<&mut VaultProfile> {
        if let Some(active_name) = &self.active_vault {
            let name = active_name.clone();
            return self.vaults.get_mut(&name);
        }
        None
    }

    /// Get vault by name
    pub fn get_vault(&self, name: &str) -> Option<&VaultProfile> {
        self.vaults.get(name)
    }

    /// Delete a vault profile
    pub fn delete_vault(&mut self, name: &str) -> Result<()> {
        if !self.vaults.contains_key(name) {
            return Err(anyhow!("Vault '{}' does not exist", name));
        }

        // Don't allow deleting active vault without switching first
        if self.active_vault.as_deref() == Some(name) {
            // If there are other vaults, switch to one of them
            if self.vaults.len() > 1 {
                let other_vault = self.vaults.keys()
                    .find(|k| k.as_str() != name)
                    .cloned()
                    .unwrap();
                self.active_vault = Some(other_vault);
            } else {
                self.active_vault = None;
            }
        }

        self.vaults.remove(name);
        self.save()?;

        Ok(())
    }

    /// List all vaults sorted by last used
    pub fn list_vaults(&self) -> Vec<&VaultProfile> {
        let mut vaults: Vec<&VaultProfile> = self.vaults.values().collect();

        // Sort by last_used (most recent first), then by name
        vaults.sort_by(|a, b| {
            match (&b.last_used, &a.last_used) {
                (Some(b_time), Some(a_time)) => b_time.cmp(a_time),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.name.cmp(&b.name),
            }
        });

        vaults
    }

    /// Rename a vault
    pub fn rename_vault(&mut self, old_name: &str, new_name: String) -> Result<()> {
        if !self.vaults.contains_key(old_name) {
            return Err(anyhow!("Vault '{}' does not exist", old_name));
        }

        if self.vaults.contains_key(&new_name) {
            return Err(anyhow!("Vault '{}' already exists", new_name));
        }

        if let Some(mut profile) = self.vaults.remove(old_name) {
            profile.name = new_name.clone();
            self.vaults.insert(new_name.clone(), profile);

            // Update active vault if it was the renamed one
            if self.active_vault.as_deref() == Some(old_name) {
                self.active_vault = Some(new_name);
            }

            self.save()?;
        }

        Ok(())
    }

    /// Update vault description
    pub fn update_description(&mut self, name: &str, description: Option<String>) -> Result<()> {
        if let Some(vault) = self.vaults.get_mut(name) {
            vault.description = description;
            self.save()?;
            Ok(())
        } else {
            Err(anyhow!("Vault '{}' does not exist", name))
        }
    }

    /// Update wallet address cache for a vault
    pub fn update_wallet_address(&mut self, name: &str, address: String) -> Result<()> {
        if let Some(vault) = self.vaults.get_mut(name) {
            vault.wallet_address = address;
            self.save()?;
            Ok(())
        } else {
            Err(anyhow!("Vault '{}' does not exist", name))
        }
    }
}

impl VaultProfile {
    /// Create a new vault profile
    pub fn new(
        name: String,
        solana_keypair_path: String,
        sphincs_public_key_path: String,
        sphincs_private_key_path: String,
        wallet_address: String,
    ) -> Self {
        Self {
            name,
            description: None,
            solana_keypair_path,
            sphincs_public_key_path,
            sphincs_private_key_path,
            wallet_address,
            created_at: Utc::now().to_rfc3339(),
            last_used: Some(Utc::now().to_rfc3339()),
        }
    }

    /// Get display name (with description if available)
    pub fn display_name(&self) -> String {
        if let Some(desc) = &self.description {
            format!("{} - {}", self.name, desc)
        } else {
            self.name.clone()
        }
    }

    /// Get short wallet address (first 4 and last 4 characters)
    pub fn short_wallet(&self) -> String {
        if self.wallet_address.len() >= 8 {
            format!("{}...{}",
                &self.wallet_address[..4],
                &self.wallet_address[self.wallet_address.len()-4..])
        } else {
            self.wallet_address.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_vault() {
        let mut config = VaultConfig {
            version: 1,
            ..Default::default()
        };

        let profile = VaultProfile::new(
            "test".to_string(),
            "/path/to/keypair.json".to_string(),
            "/path/to/public.key".to_string(),
            "/path/to/private.key".to_string(),
            "ABC123XYZ".to_string(),
        );

        assert!(config.create_vault("test".to_string(), profile).is_ok());
        assert!(config.vaults.contains_key("test"));
        assert_eq!(config.active_vault, Some("test".to_string()));
    }

    #[test]
    fn test_switch_vault() {
        let mut config = VaultConfig {
            version: 1,
            active_vault: Some("vault1".to_string()),
            vaults: HashMap::new(),
        };

        let profile1 = VaultProfile::new(
            "vault1".to_string(),
            "/path/1".to_string(),
            "/pub/1".to_string(),
            "/priv/1".to_string(),
            "ADDR1".to_string(),
        );

        let profile2 = VaultProfile::new(
            "vault2".to_string(),
            "/path/2".to_string(),
            "/pub/2".to_string(),
            "/priv/2".to_string(),
            "ADDR2".to_string(),
        );

        config.vaults.insert("vault1".to_string(), profile1);
        config.vaults.insert("vault2".to_string(), profile2);

        assert!(config.switch_vault("vault2").is_ok());
        assert_eq!(config.active_vault, Some("vault2".to_string()));
    }

    #[test]
    fn test_delete_vault() {
        let mut config = VaultConfig {
            version: 1,
            active_vault: Some("vault1".to_string()),
            vaults: HashMap::new(),
        };

        let profile1 = VaultProfile::new(
            "vault1".to_string(),
            "/path/1".to_string(),
            "/pub/1".to_string(),
            "/priv/1".to_string(),
            "ADDR1".to_string(),
        );

        let profile2 = VaultProfile::new(
            "vault2".to_string(),
            "/path/2".to_string(),
            "/pub/2".to_string(),
            "/priv/2".to_string(),
            "ADDR2".to_string(),
        );

        config.vaults.insert("vault1".to_string(), profile1);
        config.vaults.insert("vault2".to_string(), profile2);

        assert!(config.delete_vault("vault1").is_ok());
        assert!(!config.vaults.contains_key("vault1"));
        // Active should switch to vault2
        assert_eq!(config.active_vault, Some("vault2".to_string()));
    }

    #[test]
    fn test_short_wallet() {
        let profile = VaultProfile::new(
            "test".to_string(),
            "/path".to_string(),
            "/pub".to_string(),
            "/priv".to_string(),
            "7vZ8mpR3HqLqkFX2nM5Xq2M".to_string(),
        );

        assert_eq!(profile.short_wallet(), "7vZ8...Xq2M");
    }
}
