use anyhow::{Context, Result};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use solana_client::{
    rpc_client::RpcClient,
    rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig},
    rpc_filter::{RpcFilterType, Memcmp},
};
use solana_account_decoder::{UiAccountEncoding, UiDataSliceConfig};
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::fs;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use crate::crypto::sphincs::{SphincsKeyManager, SPHINCS_PUBKEY_SIZE, SPHINCS_SIGNATURE_SIZE};

/// Progress callback type for TUI integration
/// (step_number, total_steps, message)
pub type ProgressCallback = Box<dyn FnMut(usize, usize, String) + Send>;

/// PDA seeds
const PQ_ACCOUNT_SEED: &[u8] = b"pq_account";

/// SPL Token-2022 Program ID
const TOKEN_2022_PROGRAM_ID: Pubkey = solana_sdk::pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");

/// Instruction discriminators (from IDL - quantdum_token.json)
const INITIALIZE_PQ_ACCOUNT_DISCRIMINATOR: [u8; 8] = [185, 126, 40, 29, 205, 105, 111, 213];
const WRITE_PUBLIC_KEY_DISCRIMINATOR: [u8; 8] = [69, 199, 141, 25, 213, 45, 192, 226];
const LOCK_TOKENS_DISCRIMINATOR: [u8; 8] = [136, 11, 32, 232, 161, 117, 54, 211];
const CLOSE_PQ_ACCOUNT_DISCRIMINATOR: [u8; 8] = [213, 32, 12, 184, 191, 154, 92, 97];

// SPHINCS+ verification flow discriminators
const INITIALIZE_SPHINCS_STORAGE_DISCRIMINATOR: [u8; 8] = [140, 15, 169, 242, 61, 148, 238, 70];
const UPLOAD_SIGNATURE_CHUNK_DISCRIMINATOR: [u8; 8] = [194, 98, 90, 80, 66, 99, 246, 39];
const SPHINCS_VERIFY_STEP0_INIT_DISCRIMINATOR: [u8; 8] = [220, 238, 45, 110, 130, 122, 244, 163];
const SPHINCS_VERIFY_STEP1_FORS_BATCH1_DISCRIMINATOR: [u8; 8] = [172, 180, 149, 174, 231, 243, 99, 8];
const SPHINCS_VERIFY_STEP2_FORS_BATCH2_DISCRIMINATOR: [u8; 8] = [171, 180, 113, 96, 124, 173, 99, 26];
const SPHINCS_VERIFY_STEP3_FORS_ROOT_DISCRIMINATOR: [u8; 8] = [49, 50, 138, 190, 206, 224, 103, 217];
const SPHINCS_VERIFY_LAYER_WOTS_PART1_DISCRIMINATOR: [u8; 8] = [91, 71, 30, 151, 123, 241, 249, 203];
const SPHINCS_VERIFY_LAYER_WOTS_PART2_DISCRIMINATOR: [u8; 8] = [175, 251, 183, 24, 194, 124, 11, 9];
const SPHINCS_VERIFY_LAYER_WOTS_PART3_DISCRIMINATOR: [u8; 8] = [232, 111, 23, 93, 206, 103, 19, 220];
const SPHINCS_VERIFY_LAYER_MERKLE_DISCRIMINATOR: [u8; 8] = [200, 98, 174, 105, 13, 24, 123, 28];
const SPHINCS_VERIFY_STEP11_FINALIZE_DISCRIMINATOR: [u8; 8] = [34, 44, 245, 31, 130, 88, 38, 184];

/// Compute Associated Token Account address
fn get_associated_token_address(wallet: &Pubkey, mint: &Pubkey, token_program: &Pubkey) -> Pubkey {
    let seeds = &[
        wallet.as_ref(),
        token_program.as_ref(),
        mint.as_ref(),
    ];
    let (address, _) = Pubkey::find_program_address(
        seeds,
        &solana_sdk::pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"),
    );
    address
}

/// Cache for network lock query results
#[derive(Debug, Clone)]
struct NetworkLockCache {
    total_locked: f64,
    holder_count: usize,
    timestamp: SystemTime,
    mint: Pubkey,
}

impl NetworkLockCache {
    fn is_expired(&self, max_age: Duration) -> bool {
        SystemTime::now()
            .duration_since(self.timestamp)
            .map(|age| age > max_age)
            .unwrap_or(true)
    }
}

pub struct VaultClient {
    rpc_client: RpcClient,
    program_id: Pubkey,
    network_lock_cache: Arc<Mutex<Option<NetworkLockCache>>>,
}

impl VaultClient {
    pub fn new(rpc_url: &str, program_id: Pubkey) -> Result<Self> {
        // Use 60 second timeout for better reliability on slow networks
        let rpc_client = RpcClient::new_with_timeout_and_commitment(
            rpc_url.to_string(),
            Duration::from_secs(60),
            CommitmentConfig::confirmed(),
        );

        Ok(Self {
            rpc_client,
            program_id,
            network_lock_cache: Arc::new(Mutex::new(None)),
        })
    }

    /// Load keypair from JSON file
    fn load_keypair(&self, path: &str) -> Result<Keypair> {
        let data = fs::read_to_string(path)
            .with_context(|| format!("Failed to read keypair from {}", path))?;
        let bytes: Vec<u8> = serde_json::from_str(&data)
            .context("Failed to parse keypair JSON")?;
        Keypair::try_from(&bytes[..])
            .context("Invalid keypair bytes")
    }

    /// Derive PQ account PDA
    fn derive_pq_account(&self, owner: Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[PQ_ACCOUNT_SEED, owner.as_ref()],
            &self.program_id,
        )
    }


    /// Register SPHINCS+ public key on-chain
    pub async fn register_pq_account(
        &self,
        wallet: Pubkey,
        keypair_path: &str,
        sphincs_pubkey: &[u8; SPHINCS_PUBKEY_SIZE],
    ) -> Result<()> {
        println!("Wallet Address: {}", wallet.to_string().cyan());
        println!("SPHINCS+ Public Key: {}", hex::encode(sphincs_pubkey).cyan());
        println!();

        let keypair = self.load_keypair(keypair_path)?;
        let (pq_account, _) = self.derive_pq_account(wallet);

        println!("PQ Account (PDA): {}", pq_account.to_string().cyan());
        println!();

        // Check if already registered
        if let Ok(account_info) = self.rpc_client.get_account(&pq_account) {
            if account_info.data.len() > 0 {
                println!("{}", "⚠️  PQ Account already registered!".yellow());
                println!("   You can skip this step.");
                return Ok(());
            }
        }

        println!("Creating PQ account registration transaction...");

        // Build instruction data (algorithm only - public key written separately)
        let mut instruction_data = Vec::new();
        instruction_data.extend_from_slice(&INITIALIZE_PQ_ACCOUNT_DISCRIMINATOR);
        instruction_data.push(2); // Algorithm: SPHINCS+-SHA2-128s

        let instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![
                solana_sdk::instruction::AccountMeta::new(pq_account, false),
                solana_sdk::instruction::AccountMeta::new(keypair.pubkey(), true),
                solana_sdk::instruction::AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            ],
            data: instruction_data,
        };

        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&keypair.pubkey()),
            &[&keypair],
            recent_blockhash,
        );

        println!("Sending transaction...");
        let signature = self.rpc_client.send_and_confirm_transaction(&transaction)?;

        println!();
        println!("{}", "✅ PQ Account Registered!".green().bold());
        println!("   Transaction: {}", signature.to_string().cyan());
        println!("   View on Solscan: https://solscan.io/tx/{}?cluster=devnet", signature);
        println!();

        // Now write the SPHINCS+ public key to the PQ account
        println!("Writing SPHINCS+ public key to PQ account...");
        self.write_public_key(wallet, keypair_path, sphincs_pubkey).await?;

        Ok(())
    }

    /// Write SPHINCS+ public key to PQ account (called after registration)
    async fn write_public_key(
        &self,
        wallet: Pubkey,
        keypair_path: &str,
        sphincs_pubkey: &[u8; SPHINCS_PUBKEY_SIZE],
    ) -> Result<()> {
        let keypair = self.load_keypair(keypair_path)?;
        let (pq_account, _) = self.derive_pq_account(wallet);

        // Create a temporary account to hold the public key data
        let temp_keypair = Keypair::new();

        // Calculate rent for 32 bytes
        let rent = self.rpc_client.get_minimum_balance_for_rent_exemption(32)?;

        // Create the temporary account with the public key as initial data
        // We'll allocate and assign to our program so we can write the data
        let create_account_ix = solana_sdk::system_instruction::create_account(
            &keypair.pubkey(),
            &temp_keypair.pubkey(),
            rent,
            32,
            &self.program_id, // Owned by our program so write_data can write to it
        );

        // Build write_data instruction to write the public key to temp account
        let mut write_data_instruction_data = Vec::new();
        write_data_instruction_data.extend_from_slice(&[211, 152, 195, 131, 83, 179, 248, 77]); // WRITE_DATA_DISCRIMINATOR
        write_data_instruction_data.extend_from_slice(&0u32.to_le_bytes()); // offset = 0
        write_data_instruction_data.extend_from_slice(&(sphincs_pubkey.len() as u32).to_le_bytes()); // data length
        write_data_instruction_data.extend_from_slice(sphincs_pubkey); // the public key data

        let write_data_ix = Instruction {
            program_id: self.program_id,
            accounts: vec![
                solana_sdk::instruction::AccountMeta::new(temp_keypair.pubkey(), false),
                solana_sdk::instruction::AccountMeta::new(keypair.pubkey(), true),
            ],
            data: write_data_instruction_data,
        };

        // Build the write_public_key instruction
        let instruction_data = WRITE_PUBLIC_KEY_DISCRIMINATOR.to_vec();

        let write_pubkey_ix = Instruction {
            program_id: self.program_id,
            accounts: vec![
                solana_sdk::instruction::AccountMeta::new(pq_account, false),
                solana_sdk::instruction::AccountMeta::new(temp_keypair.pubkey(), false),
                solana_sdk::instruction::AccountMeta::new(keypair.pubkey(), true),
                solana_sdk::instruction::AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            ],
            data: instruction_data,
        };

        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &[create_account_ix, write_data_ix, write_pubkey_ix],
            Some(&keypair.pubkey()),
            &[&keypair, &temp_keypair],
            recent_blockhash,
        );

        println!("Sending public key write transaction...");
        let signature = self.rpc_client.send_and_confirm_transaction(&transaction)?;

        println!("{}", "✅ SPHINCS+ Public Key Written!".green().bold());
        println!("   Transaction: {}", signature.to_string().cyan());
        println!();

        Ok(())
    }

    /// Lock the vault
    pub async fn lock_vault(&self, wallet: Pubkey, keypair_path: &str) -> Result<()> {
        println!("Wallet Address: {}", wallet.to_string().cyan());
        println!();

        let keypair = self.load_keypair(keypair_path)?;
        let (pq_account, _) = self.derive_pq_account(wallet);

        println!("PQ Account (PDA): {}", pq_account.to_string().cyan());
        println!();

        // Check current status
        let account_info = self.rpc_client.get_account(&pq_account)
            .context("PQ account not found! Register first with: qdum-vault register")?;

        // Parse lock status (account layout: discriminator(8) + owner(32) + algorithm(1) + pubkey_len(4) + tokens_locked(1))
        let pubkey_len = u32::from_le_bytes(account_info.data[41..45].try_into().unwrap());
        let tokens_locked_offset = 45 + pubkey_len as usize;
        let is_locked = account_info.data[tokens_locked_offset] == 1;
        if is_locked {
            println!("{}", "⚠️  Vault is already locked!".yellow());
            return Ok(());
        }

        println!("Locking vault...");

        let instruction_data = LOCK_TOKENS_DISCRIMINATOR.to_vec();

        let instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![
                solana_sdk::instruction::AccountMeta::new(pq_account, false),
                solana_sdk::instruction::AccountMeta::new_readonly(keypair.pubkey(), true),
            ],
            data: instruction_data,
        };

        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&keypair.pubkey()),
            &[&keypair],
            recent_blockhash,
        );

        let signature = self.rpc_client.send_and_confirm_transaction(&transaction)?;

        println!();
        println!("{}", "✅ Vault Locked!".green().bold());
        println!("   Transaction: {}", signature.to_string().cyan());
        println!("   View on Solscan: https://solscan.io/tx/{}?cluster=devnet", signature);
        println!();
        println!("⚠️  Your tokens are now locked and cannot be transferred.");
        println!("   To unlock, you must sign the challenge with your SPHINCS+ private key.");
        println!();

        // Fetch and display the challenge
        let account_info = self.rpc_client.get_account(&pq_account)?;
        let pubkey_len = u32::from_le_bytes(account_info.data[41..45].try_into().unwrap());
        let challenge_offset = 46 + pubkey_len as usize;
        let challenge = &account_info.data[challenge_offset..challenge_offset + 32];
        println!("🔐 Unlock Challenge (32 bytes):");
        println!("   {}", hex::encode(challenge).cyan());
        println!();

        Ok(())
    }

    /// Close PQ account and reclaim rent
    pub async fn close_pq_account(&self, wallet: Pubkey, keypair_path: &str, receiver: Option<Pubkey>) -> Result<()> {
        println!("Wallet Address: {}", wallet.to_string().cyan());
        println!();

        let keypair = self.load_keypair(keypair_path)?;
        let (pq_account, _) = self.derive_pq_account(wallet);
        let receiver_pubkey = receiver.unwrap_or_else(|| keypair.pubkey());

        println!("PQ Account (PDA): {}", pq_account.to_string().cyan());
        println!("Rent Receiver: {}", receiver_pubkey.to_string().cyan());
        println!();

        // Check current status
        let account_info = self.rpc_client.get_account(&pq_account)
            .context("PQ account not found! Nothing to close.")?;

        // Parse lock status - must be unlocked to close
        let pubkey_len = u32::from_le_bytes(account_info.data[41..45].try_into().unwrap());
        let tokens_locked_offset = 45 + pubkey_len as usize;
        let is_locked = account_info.data[tokens_locked_offset] == 1;

        if is_locked {
            println!("{}", "❌ Cannot close PQ account while tokens are locked!".red().bold());
            println!("   Unlock your vault first with: qdum-vault unlock");
            println!();
            return Err(anyhow::anyhow!("Tokens are locked - unlock first before closing"));
        }

        println!("Closing PQ account and reclaiming rent...");

        let instruction_data = CLOSE_PQ_ACCOUNT_DISCRIMINATOR.to_vec();

        let instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![
                solana_sdk::instruction::AccountMeta::new(pq_account, false),
                solana_sdk::instruction::AccountMeta::new_readonly(keypair.pubkey(), true),
                solana_sdk::instruction::AccountMeta::new(receiver_pubkey, false),
            ],
            data: instruction_data,
        };

        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&keypair.pubkey()),
            &[&keypair],
            recent_blockhash,
        );

        let signature = self.rpc_client.send_and_confirm_transaction(&transaction)?;

        println!();
        println!("{}", "✅ PQ Account Closed!".green().bold());
        println!("   Transaction: {}", signature.to_string().cyan());
        println!("   View on Solscan: https://solscan.io/tx/{}?cluster=devnet", signature);
        println!();
        println!("💰 Rent refunded to: {}", receiver_pubkey.to_string().cyan());
        println!("   (approximately ~0.003 SOL)");
        println!();
        println!("⚠️  Your PQ account is now closed. You will need to re-register");
        println!("   if you want to use quantum-resistant features again.");
        println!();

        Ok(())
    }

    /// Unlock the vault (multi-step SPHINCS+ verification process)
    pub async fn unlock_vault(
        &self,
        wallet: Pubkey,
        keypair_path: &str,
        sphincs_privkey: &[u8; 64],
        sphincs_pubkey: &[u8; 32],
        progress_callback: Option<Box<dyn FnMut(usize, usize, String) + Send>>,
    ) -> Result<()> {
        // Wrap entire function to catch and log errors
        let result = self.unlock_vault_inner(wallet, keypair_path, sphincs_privkey, sphincs_pubkey, progress_callback).await;

        match &result {
            Ok(_) => {
                let _ = std::fs::write("/tmp/qdum-unlock-result.log", "SUCCESS");
            }
            Err(e) => {
                let error_msg = format!("UNLOCK FAILED: {:?}", e);
                let _ = std::fs::write("/tmp/qdum-unlock-result.log", &error_msg);
                eprintln!("{}", error_msg);
            }
        }

        result
    }

    async fn unlock_vault_inner(
        &self,
        wallet: Pubkey,
        keypair_path: &str,
        sphincs_privkey: &[u8; 64],
        sphincs_pubkey: &[u8; 32],
        mut progress_callback: Option<Box<dyn FnMut(usize, usize, String) + Send>>,
    ) -> Result<()> {
        println!("{}", "╔═══════════════════════════════════════════════════════════╗".on_black().bright_magenta());
        println!("{}", "║                                                           ║".on_black().bright_magenta());
        println!("{}", "║   ⚛️  QUANTUM VAULT UNLOCK SEQUENCE INITIATED  ⚛️        ║".on_black().bright_cyan().bold());
        println!("{}", "║                                                           ║".on_black().bright_magenta());
        println!("{}", "╚═══════════════════════════════════════════════════════════╝".on_black().bright_magenta());
        println!();

        println!("{} {}", "Wallet:".bright_blue().bold(), wallet.to_string().bright_white());

        let keypair = self.load_keypair(keypair_path)?;
        let (pq_account, _) = self.derive_pq_account(wallet);

        println!("{} {}", "PQ Account:".bright_blue().bold(), pq_account.to_string().bright_white());
        println!();

        // Check current status
        let account_info = self.rpc_client.get_account(&pq_account)
            .context("PQ account not found!")?;

        // Parse lock status and challenge
        let pubkey_len = u32::from_le_bytes(account_info.data[41..45].try_into().unwrap());
        let tokens_locked_offset = 45 + pubkey_len as usize;
        let is_locked = account_info.data[tokens_locked_offset] == 1;
        if !is_locked {
            println!("{}", "⚠️  Vault is already unlocked!".bright_yellow());
            return Ok(());
        }

        // Get the challenge
        let challenge_offset = tokens_locked_offset + 1;
        let challenge = &account_info.data[challenge_offset..challenge_offset + 32];
        println!("{} {}", "Challenge:".bright_blue().bold(), hex::encode(challenge).bright_cyan());
        println!();

        // Calculate total steps for progress tracking
        // 1 signature gen + 1 init storage + 10 upload chunks + 33 verify steps + 1 finalize = 46 total
        const CHUNK_SIZE: usize = 800;
        let total_chunks = (SPHINCS_SIGNATURE_SIZE + CHUNK_SIZE - 1) / CHUNK_SIZE;
        let total_steps = 1 + 1 + total_chunks + 33 + 1;
        let mut current_step = 0;

        // Step 1: Generate signature
        current_step += 1;
        if let Some(ref mut cb) = progress_callback {
            cb(current_step, total_steps, "Generating SPHINCS+ signature...".to_string());
        }

        // Spinner for signature generation
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::default_spinner()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                .template("{spinner:.magenta} {msg}")
                .unwrap()
        );
        spinner.enable_steady_tick(Duration::from_millis(80));
        spinner.set_message(format!("{}", "⚛️  Generating SPHINCS+ signature...".bright_white()));

        // Generate signature
        let key_manager = SphincsKeyManager::new(None)?;
        let signature = key_manager.sign_message(challenge, sphincs_privkey)?;

        spinner.finish_with_message(format!("{} {} bytes", "✓ Signature generated:".bright_green(), SPHINCS_SIGNATURE_SIZE.to_string().bright_yellow()));
        println!();

        // Debug logging
        println!("{}", "═══════════════════════════════════════════════════════════".bright_yellow());
        println!("{} {}", "DEBUG: SPHINCS Public Key (unlock):".bright_yellow().bold(), hex::encode(sphincs_pubkey).bright_cyan());

        // Use SPHINCS public key hash as identifier to avoid conflicts from corrupted PDAs
        // Each vault has unique SPHINCS keys, so this gives each vault its own storage
        // while still allowing reuse across multiple unlocks of the same vault
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(sphincs_pubkey);
        let pubkey_hash = hasher.finalize();
        let unique_identifier = hex::encode(&pubkey_hash[..8]);

        println!("{} {}", "DEBUG: Storage Identifier:".bright_yellow().bold(), unique_identifier.bright_cyan());
        println!("{}", "═══════════════════════════════════════════════════════════".bright_yellow());
        println!();

        // Derive signature storage PDA
        let (signature_storage, _) = Pubkey::find_program_address(
            &[b"sphincs_sig", keypair.pubkey().as_ref(), unique_identifier.as_bytes()],
            &self.program_id,
        );

        println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_cyan());
        println!("{} {}", "📦 PHASE 1:".bright_cyan().bold(), "Signature Upload".bright_white().bold());
        println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_cyan());
        println!("{} {}", "Storage PDA:".bright_blue(), signature_storage.to_string().bright_white());
        println!();

        // Step 2-9: Upload signature in chunks (800 bytes per tx - max allowed by on-chain program)
        // Using CHUNK_SIZE from earlier calculation
        let total_phase1_steps = 1 + total_chunks;

        // Progress bar for Phase 1
        let pb_phase1 = ProgressBar::new(total_phase1_steps as u64);
        pb_phase1.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.cyan} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("━━╸")
        );

        // Step 1: (Re)initialize signature storage to reset state for new unlock
        current_step += 1;
        if let Some(ref mut cb) = progress_callback {
            cb(current_step, total_steps, "Initializing signature storage...".to_string());
        }
        pb_phase1.set_message(format!("{}", "Initializing storage...".bright_white()));

        // Always reinitialize to reset state (program allows reinit of existing PDAs)
        self.initialize_sphincs_storage(&keypair, &signature_storage, &unique_identifier, sphincs_pubkey, challenge).await?;
        pb_phase1.inc(1);

        for i in 0..total_chunks {
            current_step += 1;
            if let Some(ref mut cb) = progress_callback {
                cb(current_step, total_steps, format!("Uploading signature chunk {}/{}", i + 1, total_chunks));
            }
            let start = i * CHUNK_SIZE;
            let end = ((i + 1) * CHUNK_SIZE).min(SPHINCS_SIGNATURE_SIZE);
            let chunk = &signature[start..end];
            pb_phase1.set_message(format!("{} {} ({} bytes)", "Uploading chunk".bright_white(), i + 1, chunk.len()));
            self.upload_signature_chunk(&keypair, &signature_storage, start as u32, chunk).await?;
            pb_phase1.inc(1);
        }

        pb_phase1.finish_with_message(format!("{}", "✓ Upload complete".bright_green()));
        println!();

        // Derive verification state PDA (using same unique_identifier from signature storage)
        let (verification_state, _) = Pubkey::find_program_address(
            &[b"sphincs_verify", keypair.pubkey().as_ref(), unique_identifier.as_bytes()],
            &self.program_id,
        );

        println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_magenta());
        println!("{} {}", "⚛️  PHASE 2:".bright_magenta().bold(), "Quantum Signature Verification".bright_white().bold());
        println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_magenta());
        println!("{} {}", "Verification PDA:".bright_blue(), verification_state.to_string().bright_white());
        println!();

        // Progress bar for Phase 2
        let pb_phase2 = ProgressBar::new(33);
        pb_phase2.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.magenta} [{bar:40.magenta/purple}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("━━╸")
        );

        // Step 0: (Re)initialize verification state to reset for new unlock
        current_step += 1;
        if let Some(ref mut cb) = progress_callback {
            cb(current_step, total_steps, "Initializing verification state...".to_string());
        }
        pb_phase2.set_message(format!("{}", "Initializing verification...".bright_white()));

        // Always reinitialize to reset state (program allows reinit of existing PDAs)
        self.sphincs_verify_step0_init(
            &keypair,
            &verification_state,
            &signature_storage,
            &unique_identifier,
            challenge,
            sphincs_pubkey,
            0, // unlock_duration_slots (0 = immediate unlock)
        ).await?;
        pb_phase2.inc(1);

        // Steps 1-3: FORS verification
        current_step += 1;
        if let Some(ref mut cb) = progress_callback {
            cb(current_step, total_steps, "Verifying FORS trees (batch 1/2)...".to_string());
        }
        pb_phase2.set_message(format!("{}", "Verifying FORS trees 0-6...".bright_white()));
        self.sphincs_verify_fors_batch1(&keypair, &verification_state, &signature_storage).await?;
        pb_phase2.inc(1);

        current_step += 1;
        if let Some(ref mut cb) = progress_callback {
            cb(current_step, total_steps, "Verifying FORS trees (batch 2/2)...".to_string());
        }
        pb_phase2.set_message(format!("{}", "Verifying FORS trees 7-13...".bright_white()));
        self.sphincs_verify_fors_batch2(&keypair, &verification_state, &signature_storage).await?;
        pb_phase2.inc(1);

        current_step += 1;
        if let Some(ref mut cb) = progress_callback {
            cb(current_step, total_steps, "Computing FORS root hash...".to_string());
        }
        pb_phase2.set_message(format!("{}", "Computing FORS root...".bright_white()));
        self.sphincs_verify_fors_root(&keypair, &verification_state).await?;
        pb_phase2.inc(1);

        // Steps 4-31: Layer verification (7 layers × 4 steps each)
        for layer in 0..7 {
            current_step += 1;
            if let Some(ref mut cb) = progress_callback {
                cb(current_step, total_steps, format!("Verifying layer {} - WOTS signature part 1/3", layer));
            }
            pb_phase2.set_message(format!("{} {} - WOTS Part 1", "Layer".bright_white(), layer));
            self.sphincs_verify_layer_wots_part1(&keypair, &verification_state, &signature_storage, layer as u8).await?;
            pb_phase2.inc(1);

            current_step += 1;
            if let Some(ref mut cb) = progress_callback {
                cb(current_step, total_steps, format!("Verifying layer {} - WOTS signature part 2/3", layer));
            }
            pb_phase2.set_message(format!("{} {} - WOTS Part 2", "Layer".bright_white(), layer));
            self.sphincs_verify_layer_wots_part2(&keypair, &verification_state, &signature_storage, layer as u8).await?;
            pb_phase2.inc(1);

            current_step += 1;
            if let Some(ref mut cb) = progress_callback {
                cb(current_step, total_steps, format!("Verifying layer {} - WOTS signature part 3/3", layer));
            }
            pb_phase2.set_message(format!("{} {} - WOTS Part 3", "Layer".bright_white(), layer));
            self.sphincs_verify_layer_wots_part3(&keypair, &verification_state, &signature_storage, layer as u8).await?;
            pb_phase2.inc(1);

            current_step += 1;
            if let Some(ref mut cb) = progress_callback {
                cb(current_step, total_steps, format!("Verifying layer {} - Merkle tree path", layer));
            }
            pb_phase2.set_message(format!("{} {} - Merkle tree", "Layer".bright_white(), layer));
            self.sphincs_verify_layer_merkle(&keypair, &verification_state, &signature_storage, layer as u8).await?;
            pb_phase2.inc(1);
        }

        // Step 32 (33rd step): Finalize and unlock
        current_step += 1;
        if let Some(ref mut cb) = progress_callback {
            cb(current_step, total_steps, "Finalizing and unlocking vault...".to_string());
        }
        pb_phase2.set_message(format!("{}", "Finalizing and unlocking...".bright_white()));
        self.sphincs_verify_finalize(&keypair, &verification_state, &pq_account, wallet).await?;
        pb_phase2.inc(1);

        pb_phase2.finish_with_message(format!("{}", "✓ Verification complete".bright_green()));
        println!();

        // Animated success box
        use std::io::{self, Write};
        use std::thread;

        println!();

        // Pulsing border effect
        for i in 0..3 {
            if i % 2 == 0 {
                println!("{}", "╔═══════════════════════════════════════════════════════════╗".on_black().bright_green().bold());
            } else {
                println!("{}", "╔═══════════════════════════════════════════════════════════╗".on_black().green());
            }
            if i < 2 {
                thread::sleep(Duration::from_millis(150));
                print!("\x1B[1A\r");
            }
        }

        println!("{}", "║ ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ ║".on_black().bright_green());
        thread::sleep(Duration::from_millis(50));
        println!("{}", "║        🔓 VAULT UNLOCKED [SUCCESS]                       ║".on_black().bright_green().bold());
        thread::sleep(Duration::from_millis(50));
        println!("{}", "║ ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ ║".on_black().bright_green());
        thread::sleep(Duration::from_millis(50));
        println!("{}", "╚═══════════════════════════════════════════════════════════╝".on_black().bright_green().bold());
        println!();

        // Animated checkmarks
        let checks = vec![
            "  ✓ SPHINCS+ signature verified on-chain",
            "  ✓ Vault is now unlocked",
            "  ✓ Tokens are accessible"
        ];

        for check in &checks {
            print!("{}", check.on_black().bright_green().bold());
            io::stdout().flush().unwrap();
            thread::sleep(Duration::from_millis(100));
            println!();
        }

        println!();
        println!("{} {}", "  ┃ Total transactions:".on_black().bright_magenta().bold(), "44".on_black().bright_yellow().bold());
        println!("{} {}", "  ┃ Protocol:".on_black().bright_magenta().bold(), "NIST FIPS 205".on_black().bright_cyan());
        println!();

        Ok(())
    }

    /// Initialize SPHINCS+ signature storage account
    async fn initialize_sphincs_storage(
        &self,
        keypair: &Keypair,
        signature_storage: &Pubkey,
        identifier: &str,
        public_key: &[u8; 32],
        message: &[u8],
    ) -> Result<()> {
        // Build instruction data: discriminator + identifier (string) + public_key + message (bytes)
        let mut instruction_data = Vec::new();
        instruction_data.extend_from_slice(&INITIALIZE_SPHINCS_STORAGE_DISCRIMINATOR);

        // Serialize identifier as Borsh string (length + data)
        let id_bytes = identifier.as_bytes();
        instruction_data.extend_from_slice(&(id_bytes.len() as u32).to_le_bytes());
        instruction_data.extend_from_slice(id_bytes);

        // Public key (32 bytes)
        instruction_data.extend_from_slice(public_key);

        // Serialize message as Borsh bytes (length + data)
        instruction_data.extend_from_slice(&(message.len() as u32).to_le_bytes());
        instruction_data.extend_from_slice(message);

        let instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![
                solana_sdk::instruction::AccountMeta::new(*signature_storage, false),
                solana_sdk::instruction::AccountMeta::new(keypair.pubkey(), true),
                solana_sdk::instruction::AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            ],
            data: instruction_data,
        };

        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&keypair.pubkey()),
            &[keypair],
            recent_blockhash,
        );

        // Send transaction and capture detailed error
        match self.rpc_client.send_and_confirm_transaction(&transaction) {
            Ok(sig) => {
                let _ = std::fs::write("/tmp/qdum-init-sig-success.log", format!("Signature: {}\nIdentifier: {}", sig, identifier));
                Ok(())
            }
            Err(e) => {
                let error_msg = format!("Init signature storage error:\nIdentifier: {}\nSignature Storage PDA: {}\nError: {:?}", identifier, signature_storage, e);
                let _ = std::fs::write("/tmp/qdum-init-sig-error.log", &error_msg);
                eprintln!("UNLOCK ERROR: {}", error_msg);
                Err(e.into())
            }
        }
    }

    /// Upload a chunk of SPHINCS+ signature
    async fn upload_signature_chunk(
        &self,
        keypair: &Keypair,
        signature_storage: &Pubkey,
        offset: u32,
        chunk: &[u8],
    ) -> Result<()> {
        // Build instruction data: discriminator + offset (u32) + chunk (bytes)
        let mut instruction_data = Vec::new();
        instruction_data.extend_from_slice(&UPLOAD_SIGNATURE_CHUNK_DISCRIMINATOR);
        instruction_data.extend_from_slice(&offset.to_le_bytes());

        // Serialize chunk as Borsh bytes (length + data)
        instruction_data.extend_from_slice(&(chunk.len() as u32).to_le_bytes());
        instruction_data.extend_from_slice(chunk);

        let instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![
                solana_sdk::instruction::AccountMeta::new(*signature_storage, false),
                solana_sdk::instruction::AccountMeta::new_readonly(keypair.pubkey(), true),
            ],
            data: instruction_data,
        };

        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&keypair.pubkey()),
            &[keypair],
            recent_blockhash,
        );

        self.rpc_client.send_and_confirm_transaction(&transaction)?;
        Ok(())
    }

    /// Step 0: Initialize SPHINCS+ verification state
    async fn sphincs_verify_step0_init(
        &self,
        keypair: &Keypair,
        verification_state: &Pubkey,
        signature_storage: &Pubkey,
        identifier: &str,
        message: &[u8],
        expected_public_key: &[u8; 32],
        unlock_duration_slots: u64,
    ) -> Result<()> {
        // Build instruction data: discriminator + identifier (string) + message (bytes) + expected_public_key (32 bytes) + unlock_duration_slots (u64)
        let mut instruction_data = Vec::new();
        instruction_data.extend_from_slice(&SPHINCS_VERIFY_STEP0_INIT_DISCRIMINATOR);

        // Serialize identifier as Borsh string (length + data)
        let id_bytes = identifier.as_bytes();
        instruction_data.extend_from_slice(&(id_bytes.len() as u32).to_le_bytes());
        instruction_data.extend_from_slice(id_bytes);

        // Serialize message as Borsh bytes (length + data)
        instruction_data.extend_from_slice(&(message.len() as u32).to_le_bytes());
        instruction_data.extend_from_slice(message);

        // Expected public key (32 bytes)
        instruction_data.extend_from_slice(expected_public_key);

        // Unlock duration slots (u64)
        instruction_data.extend_from_slice(&unlock_duration_slots.to_le_bytes());

        let instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![
                solana_sdk::instruction::AccountMeta::new(*verification_state, false),
                solana_sdk::instruction::AccountMeta::new(*signature_storage, false),
                solana_sdk::instruction::AccountMeta::new(keypair.pubkey(), true),
                solana_sdk::instruction::AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            ],
            data: instruction_data,
        };

        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&keypair.pubkey()),
            &[keypair],
            recent_blockhash,
        );

        self.rpc_client.send_and_confirm_transaction(&transaction)?;
        Ok(())
    }

    /// FORS verification step 1 (trees 0-6)
    async fn sphincs_verify_fors_batch1(
        &self,
        keypair: &Keypair,
        verification_state: &Pubkey,
        signature_storage: &Pubkey,
    ) -> Result<()> {
        let instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![
                solana_sdk::instruction::AccountMeta::new(*verification_state, false),
                solana_sdk::instruction::AccountMeta::new_readonly(*signature_storage, false),
                solana_sdk::instruction::AccountMeta::new_readonly(keypair.pubkey(), true),
            ],
            data: SPHINCS_VERIFY_STEP1_FORS_BATCH1_DISCRIMINATOR.to_vec(),
        };

        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&keypair.pubkey()),
            &[keypair],
            recent_blockhash,
        );

        self.rpc_client.send_and_confirm_transaction(&transaction)?;
        Ok(())
    }

    /// FORS verification step 2 (trees 7-13)
    async fn sphincs_verify_fors_batch2(
        &self,
        keypair: &Keypair,
        verification_state: &Pubkey,
        signature_storage: &Pubkey,
    ) -> Result<()> {
        let instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![
                solana_sdk::instruction::AccountMeta::new(*verification_state, false),
                solana_sdk::instruction::AccountMeta::new_readonly(*signature_storage, false),
                solana_sdk::instruction::AccountMeta::new_readonly(keypair.pubkey(), true),
            ],
            data: SPHINCS_VERIFY_STEP2_FORS_BATCH2_DISCRIMINATOR.to_vec(),
        };

        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&keypair.pubkey()),
            &[keypair],
            recent_blockhash,
        );

        self.rpc_client.send_and_confirm_transaction(&transaction)?;
        Ok(())
    }

    /// FORS root computation (step 3)
    async fn sphincs_verify_fors_root(
        &self,
        keypair: &Keypair,
        verification_state: &Pubkey,
    ) -> Result<()> {
        let instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![
                solana_sdk::instruction::AccountMeta::new(*verification_state, false),
                solana_sdk::instruction::AccountMeta::new_readonly(keypair.pubkey(), true),
            ],
            data: SPHINCS_VERIFY_STEP3_FORS_ROOT_DISCRIMINATOR.to_vec(),
        };

        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&keypair.pubkey()),
            &[keypair],
            recent_blockhash,
        );

        self.rpc_client.send_and_confirm_transaction(&transaction)?;
        Ok(())
    }

    /// Layer WOTS Part 1 verification
    async fn sphincs_verify_layer_wots_part1(
        &self,
        keypair: &Keypair,
        verification_state: &Pubkey,
        signature_storage: &Pubkey,
        layer: u8,
    ) -> Result<()> {
        // Build instruction data: discriminator + layer (u8)
        let mut instruction_data = Vec::new();
        instruction_data.extend_from_slice(&SPHINCS_VERIFY_LAYER_WOTS_PART1_DISCRIMINATOR);
        instruction_data.push(layer);

        let instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![
                solana_sdk::instruction::AccountMeta::new(*verification_state, false),
                solana_sdk::instruction::AccountMeta::new_readonly(*signature_storage, false),
                solana_sdk::instruction::AccountMeta::new_readonly(keypair.pubkey(), true),
            ],
            data: instruction_data,
        };

        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&keypair.pubkey()),
            &[keypair],
            recent_blockhash,
        );

        self.rpc_client.send_and_confirm_transaction(&transaction)?;
        Ok(())
    }

    /// Layer WOTS Part 2 verification
    async fn sphincs_verify_layer_wots_part2(
        &self,
        keypair: &Keypair,
        verification_state: &Pubkey,
        signature_storage: &Pubkey,
        layer: u8,
    ) -> Result<()> {
        // Build instruction data: discriminator + layer (u8)
        let mut instruction_data = Vec::new();
        instruction_data.extend_from_slice(&SPHINCS_VERIFY_LAYER_WOTS_PART2_DISCRIMINATOR);
        instruction_data.push(layer);

        let instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![
                solana_sdk::instruction::AccountMeta::new(*verification_state, false),
                solana_sdk::instruction::AccountMeta::new_readonly(*signature_storage, false),
                solana_sdk::instruction::AccountMeta::new_readonly(keypair.pubkey(), true),
            ],
            data: instruction_data,
        };

        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&keypair.pubkey()),
            &[keypair],
            recent_blockhash,
        );

        self.rpc_client.send_and_confirm_transaction(&transaction)?;
        Ok(())
    }

    /// Layer WOTS Part 3 verification
    async fn sphincs_verify_layer_wots_part3(
        &self,
        keypair: &Keypair,
        verification_state: &Pubkey,
        signature_storage: &Pubkey,
        layer: u8,
    ) -> Result<()> {
        // Build instruction data: discriminator + layer (u8)
        let mut instruction_data = Vec::new();
        instruction_data.extend_from_slice(&SPHINCS_VERIFY_LAYER_WOTS_PART3_DISCRIMINATOR);
        instruction_data.push(layer);

        let instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![
                solana_sdk::instruction::AccountMeta::new(*verification_state, false),
                solana_sdk::instruction::AccountMeta::new_readonly(*signature_storage, false),
                solana_sdk::instruction::AccountMeta::new_readonly(keypair.pubkey(), true),
            ],
            data: instruction_data,
        };

        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&keypair.pubkey()),
            &[keypair],
            recent_blockhash,
        );

        self.rpc_client.send_and_confirm_transaction(&transaction)?;
        Ok(())
    }

    /// Layer Merkle verification
    async fn sphincs_verify_layer_merkle(
        &self,
        keypair: &Keypair,
        verification_state: &Pubkey,
        signature_storage: &Pubkey,
        layer: u8,
    ) -> Result<()> {
        // Build instruction data: discriminator + layer (u8)
        let mut instruction_data = Vec::new();
        instruction_data.extend_from_slice(&SPHINCS_VERIFY_LAYER_MERKLE_DISCRIMINATOR);
        instruction_data.push(layer);

        let instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![
                solana_sdk::instruction::AccountMeta::new(*verification_state, false),
                solana_sdk::instruction::AccountMeta::new_readonly(*signature_storage, false),
                solana_sdk::instruction::AccountMeta::new_readonly(keypair.pubkey(), true),
            ],
            data: instruction_data,
        };

        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&keypair.pubkey()),
            &[keypair],
            recent_blockhash,
        );

        self.rpc_client.send_and_confirm_transaction(&transaction)?;
        Ok(())
    }

    /// Finalize verification and unlock vault (step 11/33)
    async fn sphincs_verify_finalize(
        &self,
        keypair: &Keypair,
        verification_state: &Pubkey,
        pq_account: &Pubkey,
        _wallet: Pubkey,
    ) -> Result<()> {
        let instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![
                solana_sdk::instruction::AccountMeta::new(*verification_state, false),
                solana_sdk::instruction::AccountMeta::new(*pq_account, false),
                solana_sdk::instruction::AccountMeta::new(keypair.pubkey(), true),
            ],
            data: SPHINCS_VERIFY_STEP11_FINALIZE_DISCRIMINATOR.to_vec(),
        };

        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&keypair.pubkey()),
            &[keypair],
            recent_blockhash,
        );

        self.rpc_client.send_and_confirm_transaction(&transaction)?;
        Ok(())
    }

    /// Check vault status
    pub async fn check_status(&self, wallet: Pubkey) -> Result<()> {
        println!("Wallet Address: {}", wallet.to_string().cyan());
        println!();

        let (pq_account, _) = self.derive_pq_account(wallet);
        println!("PQ Account (PDA): {}", pq_account.to_string().cyan());
        println!();

        let account_info = self.rpc_client.get_account(&pq_account)
            .context("PQ account not found! Register first with: qdum-vault register")?;

        // Parse account data (assuming public_key Vec<u8> with length = 0)
        // Layout: discriminator(8) + owner(32) + algorithm(1) + pubkey_len(4) + tokens_locked(1) + unlock_challenge(32) + ...
        let _owner_pubkey = &account_info.data[8..40];
        let algorithm = account_info.data[40];
        let pubkey_len = u32::from_le_bytes(account_info.data[41..45].try_into().unwrap());
        let tokens_locked_offset = 45 + pubkey_len as usize;
        let is_locked = account_info.data[tokens_locked_offset];
        let unlock_challenge_offset = tokens_locked_offset + 1;
        let unlock_challenge = &account_info.data[unlock_challenge_offset..unlock_challenge_offset + 32];

        // Read the actual public key if it exists
        let sphincs_pubkey = if pubkey_len > 0 {
            &account_info.data[45..45 + pubkey_len as usize]
        } else {
            &[] // No public key set yet
        };

        // Create status table
        use comfy_table::{Table, presets::UTF8_FULL};

        let mut status_table = Table::new();
        status_table.load_preset(UTF8_FULL);
        status_table.set_header(vec![
            "Property".bright_white().bold().to_string(),
            "Value".bright_white().bold().to_string()
        ]);

        // Add rows with proper formatting
        status_table.add_row(vec![
            "Wallet".dimmed().to_string(),
            wallet.to_string().bright_cyan().to_string()
        ]);

        status_table.add_row(vec![
            "PQ Account (PDA)".dimmed().to_string(),
            pq_account.to_string().bright_cyan().to_string()
        ]);

        let pubkey_display = if sphincs_pubkey.len() > 0 {
            hex::encode(sphincs_pubkey)[..16].to_string() + "..." + &hex::encode(sphincs_pubkey)[sphincs_pubkey.len()*2-16..]
        } else {
            "Not set".yellow().to_string()
        };
        status_table.add_row(vec![
            "SPHINCS+ Public Key".dimmed().to_string(),
            pubkey_display
        ]);

        status_table.add_row(vec![
            "Algorithm".dimmed().to_string(),
            format!("SPHINCS+-SHA2-128s ({})", algorithm).bright_green().to_string()
        ]);

        let status_display = if is_locked == 1 {
            "🔒 LOCKED".red().bold().to_string()
        } else {
            "🔓 UNLOCKED".green().bold().to_string()
        };
        status_table.add_row(vec![
            "Vault Status".dimmed().to_string(),
            status_display
        ]);

        println!("{}", status_table);
        println!();

        if is_locked == 1 {
            println!("{}", "⚠️  Vault is Locked".yellow().bold());
            println!();
            println!("  {} Your tokens cannot be transferred while locked.", "•".bright_yellow());
            println!("  {} Run {} to unlock", "•".bright_yellow(), "qdum-vault unlock".bright_green());
            println!();
            println!("{}", "Unlock Challenge:".dimmed());
            println!("  {}", hex::encode(unlock_challenge).bright_cyan());
        } else {
            println!("{}", "✓ Vault is Unlocked".green().bold());
            println!();
            println!("  {} Your tokens can be transferred freely", "•".bright_green());
            println!("  {} Run {} to lock the vault", "•".bright_green(), "qdum-vault lock".bright_yellow());
        }

        println!();

        Ok(())
    }

    /// Get vault status without printing (for dashboard)
    pub async fn get_vault_status(&self, wallet: Pubkey) -> Result<(bool, Pubkey)> {
        let (pq_account, _) = self.derive_pq_account(wallet);

        let account_info = self.rpc_client.get_account(&pq_account)
            .context("PQ account not found! Register first with: qdum-vault register")?;

        // Parse account data to get is_locked status
        let pubkey_len = u32::from_le_bytes(account_info.data[41..45].try_into().unwrap());
        let tokens_locked_offset = 45 + pubkey_len as usize;
        let is_locked = account_info.data[tokens_locked_offset];

        Ok((is_locked == 1, pq_account))
    }

    /// Get token balance without printing (for dashboard)
    /// Returns balance in base units (raw u64)
    pub async fn get_balance(&self, wallet: Pubkey, mint: Pubkey) -> Result<u64> {
        // Derive ATA (Associated Token Account)
        let ata = get_associated_token_address(&wallet, &mint, &TOKEN_2022_PROGRAM_ID);

        match self.rpc_client.get_account(&ata) {
            Ok(account_info) => {
                // Parse token account data (amount is at offset 64, 8 bytes little-endian)
                let amount = u64::from_le_bytes(account_info.data[64..72].try_into().unwrap());
                Ok(amount)
            }
            Err(_) => {
                // Token account not found, return 0
                Ok(0)
            }
        }
    }

    /// Get SOL balance (in lamports)
    pub async fn get_sol_balance(&self, wallet: Pubkey) -> Result<u64> {
        self.rpc_client.get_balance(&wallet)
            .map_err(|e| anyhow::anyhow!("Failed to get SOL balance: {}", e))
    }

    /// Check token balance
    pub async fn check_balance(&self, wallet: Pubkey, mint: Pubkey) -> Result<()> {
        println!("Wallet Address: {}", wallet.to_string().cyan());
        println!("Mint Address: {}", mint.to_string().cyan());
        println!();

        // Derive ATA (Associated Token Account)
        let ata = get_associated_token_address(&wallet, &mint, &TOKEN_2022_PROGRAM_ID);

        println!("Token Account (ATA): {}", ata.to_string().cyan());
        println!();

        match self.rpc_client.get_account(&ata) {
            Ok(account_info) => {
                // Parse token account data (amount is at offset 64, 8 bytes little-endian)
                let amount = u64::from_le_bytes(account_info.data[64..72].try_into().unwrap());
                let balance = amount as f64 / 1_000_000.0; // 6 decimals

                println!("{}", "💰 Balance".bold().cyan());
                println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
                println!();
                println!("   {} QDUM", balance.to_string().green().bold());
                println!();
                println!("   ({} base units)", amount.to_string().dimmed());
            }
            Err(_) => {
                println!("{}", "⚠️  Token account not found".yellow());
                println!("   This wallet has no QDUM tokens yet.");
            }
        }

        println!();

        Ok(())
    }

    /// Calculate scarcity multiplier (matches on-chain calculation)
    /// Scenario A: Progressive multipliers for $2.5M protocol revenue
    /// Target: $100 per user (0.667 SOL @ $150/SOL) for 123,695 QDUM
    ///
    /// Uses 10x scaling to handle decimal multipliers:
    /// - 0.75× = 7 (will divide by 10 in fee calculation)
    /// - 1.5×  = 15
    /// - 2.5×  = 25
    /// - 4×    = 40
    pub async fn transfer_tokens(
        &self,
        keypair: &Keypair,
        recipient: Pubkey,
        mint: Pubkey,
        amount: u64,
    ) -> Result<()> {
        self.transfer_tokens_with_confirm(keypair, recipient, mint, amount, true).await
    }

    pub async fn transfer_tokens_with_confirm(
        &self,
        keypair: &Keypair,
        recipient: Pubkey,
        mint: Pubkey,
        amount: u64,
        skip_confirm: bool,
    ) -> Result<()> {
        use solana_sdk::instruction::Instruction;
        use std::io::{self, Write};

        println!("To:           {}", recipient.to_string().cyan());
        println!("Amount:       {} base units ({} QDUM)", amount.to_string().yellow(), (amount as f64 / 1_000_000.0).to_string().green());
        println!("Mint:         {}", mint.to_string().cyan());
        println!();

        // Get sender and recipient token accounts (ATAs)
        let sender_token_account = get_associated_token_address(
            &keypair.pubkey(),
            &mint,
            &TOKEN_2022_PROGRAM_ID,
        );

        let recipient_token_account = get_associated_token_address(
            &recipient,
            &mint,
            &TOKEN_2022_PROGRAM_ID,
        );

        // Derive PQ account PDA for sender (for transfer hook validation)
        let (pq_account, _) = self.derive_pq_account(keypair.pubkey());

        // Check if sender account has sufficient balance
        let sender_account_info = self.rpc_client.get_account(&sender_token_account)
            .context("Sender token account not found! You don't have any tokens to transfer.")?;

        let current_balance = u64::from_le_bytes(sender_account_info.data[64..72].try_into().unwrap());
        let balance_qdum = current_balance as f64 / 1_000_000.0;

        println!("{}", "╔═══════════════════════════════════════════════════════════╗".bright_cyan());
        println!("{}", "║                  TRANSFER SUMMARY                         ║".bright_cyan().bold());
        println!("{}", "╚═══════════════════════════════════════════════════════════╝".bright_cyan());
        println!();
        println!("{} {}", "Your Balance:".bold(), format!("{} QDUM", balance_qdum).green());
        println!("{} {}", "Transfer Amount:".bold(), format!("{} QDUM", amount as f64 / 1_000_000.0).yellow());
        println!("{} {}", "Remaining:".bold(), format!("{} QDUM", (current_balance - amount) as f64 / 1_000_000.0).cyan());
        println!();

        if current_balance < amount {
            println!("{}", "❌ Insufficient balance!".red().bold());
            return Ok(());
        }

        // Check if PQ account exists and is locked
        if let Ok(pq_account_info) = self.rpc_client.get_account(&pq_account) {
            let pubkey_len = u32::from_le_bytes(pq_account_info.data[41..45].try_into().unwrap());
            let tokens_locked_offset = 45 + pubkey_len as usize;
            let is_locked = pq_account_info.data[tokens_locked_offset] == 1;

            if is_locked {
                println!("{}", "⚠️  Your vault is LOCKED!".red().bold());
                println!();
                println!("Transfers are disabled while your vault is locked.");
                println!("To unlock your vault, run: qdum-vault unlock");
                println!();
                return Ok(());
            } else {
                println!("{}", "✓ Vault is unlocked - transfer allowed".green());
                println!();
            }
        } else {
            println!("{}", "ℹ  No PQ account found - proceeding with normal transfer".dimmed());
            println!();
        }

        // Confirmation prompt (only if not skipped)
        if !skip_confirm {
            print!("{}", "Proceed with transfer? (y/n): ".bright_green().bold());
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let answer = input.trim().to_lowercase();

            if answer != "y" && answer != "yes" {
                println!();
                println!("{}", "❌ Transfer cancelled".red());
                return Ok(());
            }

            println!();
        }

        // Build transaction with ComputeBudget instructions (like Phantom does)
        let mut instructions = Vec::new();

        // Add ComputeBudget instructions to request more compute units
        // Phantom uses: setComputeUnitLimit (200,000) and setComputeUnitPrice
        use solana_sdk::compute_budget::ComputeBudgetInstruction;

        instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(200_000));
        instructions.push(ComputeBudgetInstruction::set_compute_unit_price(200_000));

        // Check if recipient ATA exists, create if not
        match self.rpc_client.get_account(&recipient_token_account) {
            Ok(_) => {
                println!("Recipient token account exists: {}", recipient_token_account.to_string().cyan());
            }
            Err(_) => {
                println!("Creating recipient token account...");

                // Associated Token Program ID
                const ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey = solana_sdk::pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

                let create_ata_ix = Instruction {
                    program_id: ASSOCIATED_TOKEN_PROGRAM_ID,
                    accounts: vec![
                        solana_sdk::instruction::AccountMeta::new(keypair.pubkey(), true),
                        solana_sdk::instruction::AccountMeta::new(recipient_token_account, false),
                        solana_sdk::instruction::AccountMeta::new_readonly(recipient, false),
                        solana_sdk::instruction::AccountMeta::new_readonly(mint, false),
                        solana_sdk::instruction::AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
                        solana_sdk::instruction::AccountMeta::new_readonly(TOKEN_2022_PROGRAM_ID, false),
                    ],
                    data: vec![],
                };

                instructions.push(create_ata_ix);
            }
        }

        // Build standard Token-2022 TransferChecked instruction
        // According to SPL Transfer Hook docs, we need to:
        // 1. Create base TransferChecked instruction (4 accounts)
        // 2. Read and resolve extra accounts from ExtraAccountMetaList PDA
        // 3. Append those accounts to the instruction

        let mut instruction_data = Vec::new();
        instruction_data.push(12); // TransferChecked discriminator
        instruction_data.extend_from_slice(&amount.to_le_bytes());
        instruction_data.push(6); // decimals

        // Build TransferChecked accounts following Token-2022 spec
        // Analyzed from Phantom's successful transaction (2uhKM7acwx3Z...):
        // Account order for TransferChecked with transfer hook:
        // 0: source_account (writable)
        // 1: mint (read-only)
        // 2: destination_account (writable)
        // 3: owner/authority (signer)
        // 4: transfer_hook_program_id (read-only) ← CRITICAL! Token-2022 needs this to invoke the hook
        // 5: extra_account_metas_pda (read-only)
        // 6: pq_account (read-only)

        let (extra_account_meta_list, _) = Pubkey::find_program_address(
            &[b"extra-account-metas", mint.as_ref()],
            &self.program_id,
        );

        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new(sender_token_account, false),           // 0: source
            solana_sdk::instruction::AccountMeta::new_readonly(mint, false),                  // 1: mint
            solana_sdk::instruction::AccountMeta::new(recipient_token_account, false),        // 2: destination
            solana_sdk::instruction::AccountMeta::new_readonly(keypair.pubkey(), true),       // 3: owner (signer)
            solana_sdk::instruction::AccountMeta::new_readonly(self.program_id, false),       // 4: transfer hook program
            solana_sdk::instruction::AccountMeta::new_readonly(extra_account_meta_list, false), // 5: extra metas PDA
            solana_sdk::instruction::AccountMeta::new_readonly(pq_account, false),            // 6: PQ account
        ];

        let transfer_ix = Instruction {
            program_id: TOKEN_2022_PROGRAM_ID,
            accounts,
            data: instruction_data,
        };

        instructions.push(transfer_ix);

        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &instructions,
            Some(&keypair.pubkey()),
            &[keypair],
            recent_blockhash,
        );

        // Progress bar
        let pb = ProgressBar::new(3);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.cyan} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("━━╸")
        );

        pb.set_message(format!("{}", "Building transaction...".bright_white()));
        pb.inc(1);

        pb.set_message(format!("{}", "Sending to network...".bright_white()));
        let signature = self.rpc_client.send_and_confirm_transaction(&transaction)?;
        pb.inc(1);

        pb.set_message(format!("{}", "Confirming...".bright_white()));
        pb.inc(1);

        pb.finish_with_message(format!("{}", "✓ Transaction confirmed".bright_green()));
        println!();

        println!();
        println!("{}", "╔═══════════════════════════════════════════════════════════╗".on_black().bright_green());
        println!("{}", "║          ✅ TRANSFER COMPLETE [SUCCESS]                  ║".on_black().bright_green().bold());
        println!("{}", "╚═══════════════════════════════════════════════════════════╝".on_black().bright_green());
        println!();
        println!("{} {}", "  ┃ Amount:     ".on_black().bright_magenta().bold(), format!("{} QDUM", amount as f64 / 1_000_000.0).on_black().bright_green());
        println!("{} {}", "  ┃ Recipient:  ".on_black().bright_magenta().bold(), recipient.to_string().on_black().bright_cyan());
        println!("{} {}", "  ┃ Transaction:".on_black().bright_magenta().bold(), signature.to_string().on_black().cyan());
        println!();
        println!("{}", format!("   View on Solscan: https://solscan.io/tx/{}?cluster=devnet", signature).dimmed());
        println!();

        Ok(())
    }

    /// Get total locked QDUM across ALL network holders (with caching and batching)
    /// Returns (total_locked, holder_count)
    pub async fn get_network_locked_total(&self, mint: Pubkey, force_refresh: bool) -> Result<(f64, usize)> {

        // Check cache first (5 minute expiry) unless force_refresh is true
        if !force_refresh {
            let cache_max_age = Duration::from_secs(5 * 60);
            {
                let cache = self.network_lock_cache.lock().unwrap();
                if let Some(cached) = cache.as_ref() {
                    if cached.mint == mint && !cached.is_expired(cache_max_age) {
                        return Ok((cached.total_locked, cached.holder_count));
                    }
                }
            }
        }

        // Cache miss or expired - query the network with optimized filters

        // OPTIMIZATION: Use RPC filters to only fetch LOCKED accounts
        // Account layout: discriminator(8) + owner(32) + algorithm(1) + pubkey_len(4) + pubkey_data(32) + is_locked(1)
        // For SPHINCS+ (32 byte keys), is_locked is at offset 77 (8+32+1+4+32=77)
        const LOCKED_OFFSET: usize = 77;

        let config = RpcProgramAccountsConfig {
            filters: Some(vec![
                // Filter 1: Only accounts where is_locked == 1 at offset 77
                RpcFilterType::Memcmp(Memcmp::new_raw_bytes(LOCKED_OFFSET, vec![1])),
            ]),
            account_config: RpcAccountInfoConfig {
                encoding: Some(UiAccountEncoding::Base64),
                data_slice: Some(UiDataSliceConfig {
                    offset: 0,
                    length: 100, // Only fetch first 100 bytes (enough for owner + lock status)
                }),
                ..RpcAccountInfoConfig::default()
            },
            ..RpcProgramAccountsConfig::default()
        };

        // Get only LOCKED PQ accounts (1 RPC call, highly filtered)
        let accounts = self.rpc_client.get_program_accounts_with_config(&self.program_id, config)?;

        let mut debug_log = format!("=== Network Lock Query (OPTIMIZED with RPC Filters) ===\n");
        debug_log.push_str(&format!("Program ID: {}\n", self.program_id));
        debug_log.push_str(&format!("Mint: {}\n", mint));
        debug_log.push_str(&format!("Locked PQ accounts found (filtered): {}\n", accounts.len()));
        debug_log.push_str(&format!("\n"));

        // Step 1: Parse locked accounts to extract owner addresses
        let mut locked_owners = Vec::new();

        for (_pubkey, account) in &accounts {
            let account_data = &account.data;

            // Check if account has enough data (8 discriminator + 32 owner)
            if account_data.len() >= 40 {
                // Extract owner pubkey from account data
                let mut owner_bytes = [0u8; 32];
                owner_bytes.copy_from_slice(&account_data[8..40]);
                let owner = Pubkey::new_from_array(owner_bytes);
                locked_owners.push(owner);
            }
        }

        debug_log.push_str(&format!("Valid locked accounts parsed: {}\n", locked_owners.len()));
        debug_log.push_str(&format!("\n"));

        // Step 2: Derive token account addresses for locked owners only
        let token_program = &TOKEN_2022_PROGRAM_ID;
        let token_accounts: Vec<Pubkey> = locked_owners
            .iter()
            .map(|owner| get_associated_token_address(owner, &mint, token_program))
            .collect();

        debug_log.push_str(&format!("Fetching balances for {} token accounts in batches...\n", token_accounts.len()));

        // Step 3: Batch fetch all token accounts (100 at a time)
        const BATCH_SIZE: usize = 100;
        let mut all_balances: Vec<Option<u64>> = vec![None; token_accounts.len()];

        for (i, chunk) in token_accounts.chunks(BATCH_SIZE).enumerate() {
            match self.rpc_client.get_multiple_accounts(chunk) {
                Ok(accounts_batch) => {
                    for (j, account_opt) in accounts_batch.iter().enumerate() {
                        let idx = i * BATCH_SIZE + j;
                        if let Some(account) = account_opt {
                            // Parse SPL token account data (amount is at offset 64)
                            if account.data.len() >= 72 {
                                let amount = u64::from_le_bytes(account.data[64..72].try_into().unwrap_or([0u8; 8]));
                                all_balances[idx] = Some(amount);
                            }
                        }
                    }
                    debug_log.push_str(&format!("  Batch {}: fetched {} accounts\n", i + 1, chunk.len()));
                }
                Err(e) => {
                    debug_log.push_str(&format!("  Batch {}: error - {}\n", i + 1, e));
                }
            }
        }

        debug_log.push_str(&format!("\n"));

        // Step 4: Process results (only locked accounts)
        let mut total_locked: u64 = 0;
        let mut locked_count = 0;
        let mut all_accounts_with_balance = Vec::new();

        // Process locked accounts
        for (i, owner) in locked_owners.iter().enumerate() {
            if let Some(balance) = all_balances[i] {
                if balance > 0 {
                    total_locked += balance;
                    locked_count += 1;
                    all_accounts_with_balance.push((*owner, balance));
                    debug_log.push_str(&format!("  LOCKED: {} - {} QDUM ✓\n", owner, balance as f64 / 1_000_000.0));
                } else {
                    locked_count += 1;
                    debug_log.push_str(&format!("  LOCKED: {} - 0 QDUM (empty)\n", owner));
                }
            } else {
                locked_count += 1;
                debug_log.push_str(&format!("  LOCKED: {} - No token account\n", owner));
            }
        }

        debug_log.push_str(&format!("\n=== SUMMARY ===\n"));
        debug_log.push_str(&format!("Locked accounts: {}\n", locked_count));
        debug_log.push_str(&format!("Total locked QDUM: {}\n", total_locked as f64 / 1_000_000.0));

        debug_log.push_str(&format!("\n=== LOCKED ACCOUNTS WITH BALANCES ===\n"));
        for (owner, balance) in &all_accounts_with_balance {
            debug_log.push_str(&format!("  {} - {} QDUM 🔒\n",
                owner,
                *balance as f64 / 1_000_000.0
            ));
        }

        // Write debug log
        debug_log.push_str(&format!("\n=== PERFORMANCE ===\n"));
        debug_log.push_str(&format!("OPTIMIZED with RPC filters:\n"));
        debug_log.push_str(&format!("  - Filter: Only fetched LOCKED accounts at RPC level\n"));
        debug_log.push_str(&format!("  - DataSlice: Only fetched first 100 bytes per account\n"));
        debug_log.push_str(&format!("Total RPC calls: {} (1 filtered getProgramAccounts + {} batches of getMultipleAccounts)\n",
            1 + (token_accounts.len() + BATCH_SIZE - 1) / BATCH_SIZE,
            (token_accounts.len() + BATCH_SIZE - 1) / BATCH_SIZE));
        debug_log.push_str(&format!("Without filter optimization: Would fetch ALL accounts (locked + unlocked) then filter locally\n"));
        let _ = std::fs::write("/tmp/qdum-network-query.log", debug_log);

        // Convert to QDUM (divide by 1_000_000)
        let total_qdum = total_locked as f64 / 1_000_000.0;

        // Update cache
        {
            let mut cache = self.network_lock_cache.lock().unwrap();
            *cache = Some(NetworkLockCache {
                total_locked: total_qdum,
                holder_count: locked_count,
                timestamp: SystemTime::now(),
                mint,
            });
        }

        Ok((total_qdum, locked_count))
    }

    /// Get the cache timestamp for display purposes
    pub fn get_network_lock_cache_age(&self) -> Option<Duration> {
        let cache = self.network_lock_cache.lock().unwrap();
        cache.as_ref().and_then(|c| {
            SystemTime::now().duration_since(c.timestamp).ok()
        })
    }

}
