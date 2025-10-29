use anyhow::{Context, Result};
use colored::Colorize;
use fips205::slh_dsa_sha2_128s;
use fips205::traits::{SerDes, Signer, Verifier};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

/// SPHINCS+ key sizes
pub const SPHINCS_PUBKEY_SIZE: usize = 32;
pub const SPHINCS_PRIVKEY_SIZE: usize = 64;
pub const SPHINCS_SIGNATURE_SIZE: usize = 7856;

pub struct SphincsKeyManager {
    key_dir: PathBuf,
}

impl SphincsKeyManager {
    /// Create a new key manager with the specified directory
    /// Defaults to ~/.qdum/ if no directory is specified
    pub fn new(output_dir: Option<String>) -> Result<Self> {
        let key_dir = if let Some(dir) = output_dir {
            PathBuf::from(dir)
        } else {
            let home = dirs::home_dir().context("Failed to get home directory")?;
            home.join(".qdum")
        };

        Ok(Self { key_dir })
    }

    /// Generate a new SPHINCS+ keypair and save it to disk
    pub fn generate_and_save_keypair(&self) -> Result<()> {
        println!("Generating SPHINCS+-SHA2-128s keypair...");
        println!();

        // Create key directory if it doesn't exist
        fs::create_dir_all(&self.key_dir)
            .context("Failed to create key directory")?;

        // Generate keypair
        let (pk, sk) = slh_dsa_sha2_128s::try_keygen()
            .map_err(|e| anyhow::anyhow!("Key generation failed: {:?}", e))?;

        // Convert to bytes
        let public_key = pk.into_bytes();
        let secret_key = sk.into_bytes();

        // Save private key
        let privkey_path = self.key_dir.join("sphincs_private.key");
        fs::write(&privkey_path, &secret_key)
            .context("Failed to write private key")?;

        println!("{}", "✅ Private Key Generated".green().bold());
        println!("   Location: {}", privkey_path.display());
        println!("   Size: {} bytes", SPHINCS_PRIVKEY_SIZE);
        println!();

        // Save public key
        let pubkey_path = self.key_dir.join("sphincs_public.key");
        fs::write(&pubkey_path, &public_key)
            .context("Failed to write public key")?;

        println!("{}", "✅ Public Key Generated".green().bold());
        println!("   Location: {}", pubkey_path.display());
        println!("   Size: {} bytes", SPHINCS_PUBKEY_SIZE);
        println!();

        // Display public key in hex
        println!("{}", "📋 Public Key (hex):".cyan().bold());
        println!("   {}", hex::encode(&public_key));
        println!();

        // Security warning
        println!("{}", "⚠️  SECURITY WARNING".yellow().bold());
        println!("   Keep your private key EXTREMELY safe!");
        println!("   Anyone with access to your private key can unlock your vault.");
        println!("   Consider storing it offline or in a hardware security module.");
        println!();

        println!("{}", "✅ Keypair generation complete!".green().bold());

        Ok(())
    }

    /// Load public key from file
    pub fn load_public_key(&self, path: Option<String>) -> Result<[u8; SPHINCS_PUBKEY_SIZE]> {
        let pubkey_path = if let Some(p) = path {
            PathBuf::from(p)
        } else {
            self.key_dir.join("sphincs_public.key")
        };

        let data = fs::read(&pubkey_path)
            .with_context(|| format!("Failed to read public key from {}", pubkey_path.display()))?;

        if data.len() != SPHINCS_PUBKEY_SIZE {
            anyhow::bail!(
                "Invalid public key size: expected {} bytes, got {}",
                SPHINCS_PUBKEY_SIZE,
                data.len()
            );
        }

        let mut pubkey = [0u8; SPHINCS_PUBKEY_SIZE];
        pubkey.copy_from_slice(&data);
        Ok(pubkey)
    }

    /// Load private key from file
    pub fn load_private_key(&self, path: Option<String>) -> Result<[u8; SPHINCS_PRIVKEY_SIZE]> {
        let privkey_path = if let Some(p) = path {
            PathBuf::from(p)
        } else {
            self.key_dir.join("sphincs_private.key")
        };

        let data = fs::read(&privkey_path)
            .with_context(|| format!("Failed to read private key from {}", privkey_path.display()))?;

        if data.len() != SPHINCS_PRIVKEY_SIZE {
            anyhow::bail!(
                "Invalid private key size: expected {} bytes, got {}",
                SPHINCS_PRIVKEY_SIZE,
                data.len()
            );
        }

        let mut privkey = [0u8; SPHINCS_PRIVKEY_SIZE];
        privkey.copy_from_slice(&data);
        Ok(privkey)
    }

    /// Sign a message with SPHINCS+ private key
    pub fn sign_message(
        &self,
        message: &[u8],
        private_key: &[u8; SPHINCS_PRIVKEY_SIZE],
    ) -> Result<[u8; SPHINCS_SIGNATURE_SIZE]> {
        // Deserialize private key
        let sk = slh_dsa_sha2_128s::PrivateKey::try_from_bytes(private_key)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize private key: {:?}", e))?;

        // Sign message
        let signature = sk.try_sign(message, &[], true)
            .map_err(|e| anyhow::anyhow!("Signature generation failed: {:?}", e))?;

        Ok(signature)
    }

    /// Verify a SPHINCS+ signature
    #[allow(dead_code)]
    pub fn verify_signature(
        message: &[u8],
        signature: &[u8; SPHINCS_SIGNATURE_SIZE],
        public_key: &[u8; SPHINCS_PUBKEY_SIZE],
    ) -> Result<bool> {
        // Deserialize public key
        let pk = slh_dsa_sha2_128s::PublicKey::try_from_bytes(public_key)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize public key: {:?}", e))?;

        // Verify using Verifier trait (takes 3 params: message, sig bytes, context)
        // Empty context slice for standard SPHINCS+ verification
        let is_valid = pk.verify(message, signature, &[]);
        Ok(is_valid)
    }

    /// Hash a signature to 32 bytes (for on-chain storage)
    #[allow(dead_code)]
    pub fn hash_signature(signature: &[u8; SPHINCS_SIGNATURE_SIZE]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(signature);
        hasher.finalize().into()
    }
}

// We need hex crate for displaying keys
// Add this to Cargo.toml if not already present
