use anyhow::{Context, Result};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::fs;
use std::time::Duration;

use crate::crypto::sphincs::{SphincsKeyManager, SPHINCS_PUBKEY_SIZE, SPHINCS_SIGNATURE_SIZE};

/// PDA seeds
const PQ_ACCOUNT_SEED: &[u8] = b"pq_account";

/// SPL Token-2022 Program ID
const TOKEN_2022_PROGRAM_ID: Pubkey = solana_sdk::pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");

/// Instruction discriminators (from IDL - quantdum_token.json)
const INITIALIZE_PQ_ACCOUNT_DISCRIMINATOR: [u8; 8] = [185, 126, 40, 29, 205, 105, 111, 213];
const LOCK_TOKENS_DISCRIMINATOR: [u8; 8] = [136, 11, 32, 232, 161, 117, 54, 211];

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

pub struct VaultClient {
    rpc_client: RpcClient,
    program_id: Pubkey,
}

impl VaultClient {
    pub fn new(rpc_url: &str, program_id: Pubkey) -> Result<Self> {
        let rpc_client = RpcClient::new_with_commitment(
            rpc_url.to_string(),
            CommitmentConfig::confirmed(),
        );

        Ok(Self {
            rpc_client,
            program_id,
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
        &mut self,
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

        // Build instruction data (algorithm only - public key set separately)
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

        Ok(())
    }

    /// Lock the vault
    pub async fn lock_vault(&mut self, wallet: Pubkey, keypair_path: &str) -> Result<()> {
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

    /// Unlock the vault (multi-step SPHINCS+ verification process)
    pub async fn unlock_vault(
        &mut self,
        wallet: Pubkey,
        keypair_path: &str,
        sphincs_privkey: &[u8; 64],
    ) -> Result<()> {
        println!("{}", "╔═══════════════════════════════════════════════════════════╗".bright_cyan());
        println!("{}", "║                                                           ║".bright_cyan());
        println!("{}", "║    ⚛️  QUANTUM VAULT UNLOCK SEQUENCE INITIATED  ⚛️       ║".bright_magenta().bold());
        println!("{}", "║                                                           ║".bright_cyan());
        println!("{}", "╚═══════════════════════════════════════════════════════════╝".bright_cyan());
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

        // Load SPHINCS+ public key
        let key_manager = SphincsKeyManager::new(None)?;
        let sphincs_pubkey = key_manager.load_public_key(None)?;

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
        let signature = key_manager.sign_message(challenge, sphincs_privkey)?;

        spinner.finish_with_message(format!("{} {} bytes", "✓ Signature generated:".bright_green(), SPHINCS_SIGNATURE_SIZE.to_string().bright_yellow()));
        println!();

        // Derive signature storage PDA
        let identifier = "unlock";
        let (signature_storage, _) = Pubkey::find_program_address(
            &[b"sphincs_sig", keypair.pubkey().as_ref(), identifier.as_bytes()],
            &self.program_id,
        );

        println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_cyan());
        println!("{} {}", "📦 PHASE 1:".bright_cyan().bold(), "Signature Upload".bright_white().bold());
        println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_cyan());
        println!("{} {}", "Storage PDA:".bright_blue(), signature_storage.to_string().bright_white());
        println!();

        // Step 2-9: Upload signature in chunks (800 bytes per tx - max allowed by on-chain program)
        const CHUNK_SIZE: usize = 800;
        let total_chunks = (SPHINCS_SIGNATURE_SIZE + CHUNK_SIZE - 1) / CHUNK_SIZE;
        let total_phase1_steps = 1 + total_chunks;

        // Progress bar for Phase 1
        let pb_phase1 = ProgressBar::new(total_phase1_steps as u64);
        pb_phase1.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.cyan} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("━━╸")
        );

        // Step 1: Initialize signature storage (or reuse existing)
        pb_phase1.set_message(format!("{}", "Initializing storage...".bright_white()));

        // Check if signature storage already exists
        let storage_exists = self.rpc_client.get_account(&signature_storage).is_ok();
        if !storage_exists {
            self.initialize_sphincs_storage(&keypair, &signature_storage, identifier, &sphincs_pubkey, challenge).await?;
        }
        pb_phase1.inc(1);

        for i in 0..total_chunks {
            let start = i * CHUNK_SIZE;
            let end = ((i + 1) * CHUNK_SIZE).min(SPHINCS_SIGNATURE_SIZE);
            let chunk = &signature[start..end];
            pb_phase1.set_message(format!("{} {} ({} bytes)", "Uploading chunk".bright_white(), i + 1, chunk.len()));
            self.upload_signature_chunk(&keypair, &signature_storage, start as u32, chunk).await?;
            pb_phase1.inc(1);
        }

        pb_phase1.finish_with_message(format!("{}", "✓ Upload complete".bright_green()));
        println!();

        // Derive verification state PDA
        let (verification_state, _) = Pubkey::find_program_address(
            &[b"sphincs_verify", keypair.pubkey().as_ref(), identifier.as_bytes()],
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

        // Step 0: Initialize verification state
        pb_phase2.set_message(format!("{}", "Initializing verification...".bright_white()));
        self.sphincs_verify_step0_init(
            &keypair,
            &verification_state,
            &signature_storage,
            identifier,
            challenge,
            &sphincs_pubkey,
            0, // unlock_duration_slots (0 = immediate unlock)
        ).await?;
        pb_phase2.inc(1);

        // Steps 1-3: FORS verification
        pb_phase2.set_message(format!("{}", "Verifying FORS trees 0-6...".bright_white()));
        self.sphincs_verify_fors_batch1(&keypair, &verification_state, &signature_storage).await?;
        pb_phase2.inc(1);

        pb_phase2.set_message(format!("{}", "Verifying FORS trees 7-13...".bright_white()));
        self.sphincs_verify_fors_batch2(&keypair, &verification_state, &signature_storage).await?;
        pb_phase2.inc(1);

        pb_phase2.set_message(format!("{}", "Computing FORS root...".bright_white()));
        self.sphincs_verify_fors_root(&keypair, &verification_state).await?;
        pb_phase2.inc(1);

        // Steps 4-31: Layer verification (7 layers × 4 steps each)
        for layer in 0..7 {
            pb_phase2.set_message(format!("{} {} - WOTS Part 1", "Layer".bright_white(), layer));
            self.sphincs_verify_layer_wots_part1(&keypair, &verification_state, &signature_storage, layer as u8).await?;
            pb_phase2.inc(1);

            pb_phase2.set_message(format!("{} {} - WOTS Part 2", "Layer".bright_white(), layer));
            self.sphincs_verify_layer_wots_part2(&keypair, &verification_state, &signature_storage, layer as u8).await?;
            pb_phase2.inc(1);

            pb_phase2.set_message(format!("{} {} - WOTS Part 3", "Layer".bright_white(), layer));
            self.sphincs_verify_layer_wots_part3(&keypair, &verification_state, &signature_storage, layer as u8).await?;
            pb_phase2.inc(1);

            pb_phase2.set_message(format!("{} {} - Merkle tree", "Layer".bright_white(), layer));
            self.sphincs_verify_layer_merkle(&keypair, &verification_state, &signature_storage, layer as u8).await?;
            pb_phase2.inc(1);
        }

        // Step 32 (33rd step): Finalize and unlock
        pb_phase2.set_message(format!("{}", "Finalizing and unlocking...".bright_white()));
        self.sphincs_verify_finalize(&keypair, &verification_state, &pq_account, wallet).await?;
        pb_phase2.inc(1);

        pb_phase2.finish_with_message(format!("{}", "✓ Verification complete".bright_green()));
        println!();

        println!("{}", "═".repeat(61).bright_green());
        println!();
        println!("  {}", "🔓 VAULT UNLOCKED SUCCESSFULLY".bright_green().bold());
        println!();
        println!("{}", "═".repeat(61).bright_green());
        println!();
        println!("  {} SPHINCS+ signature verified on-chain", "✓".bright_green());
        println!("  {} Vault is now unlocked", "✓".bright_green());
        println!("  {} Tokens are accessible", "✓".bright_green());
        println!();
        println!("{} {}", "Total transactions:".bright_blue().bold(), "44".bright_yellow().bold());
        println!("{} {}", "Protocol:".bright_blue().bold(), "NIST FIPS 205".bright_cyan());
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

        self.rpc_client.send_and_confirm_transaction(&transaction)?;
        Ok(())
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

        println!("{}", "📊 Vault Status".bold().cyan());
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!();
        if sphincs_pubkey.len() > 0 {
            println!("SPHINCS+ Public Key: {}", hex::encode(sphincs_pubkey).cyan());
        } else {
            println!("SPHINCS+ Public Key: {} (not set - use write_public_key instruction)", "None".yellow());
        }
        println!("Algorithm: SPHINCS+-SHA2-128s ({})", algorithm);
        println!();

        if is_locked == 1 {
            println!("Status: {}", "🔒 LOCKED".red().bold());
            println!();
            println!("Your tokens cannot be transferred while locked.");
            println!("To unlock, run: qdum-vault unlock --wallet {} --keypair <path>", wallet);
            println!();
            println!("Current Unlock Challenge:");
            println!("   {}", hex::encode(unlock_challenge).cyan());
        } else {
            println!("Status: {}", "🔓 UNLOCKED".green().bold());
            println!();
            println!("Your tokens can be transferred freely.");
        }

        println!();

        Ok(())
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

    /// Mint QDUM tokens
    pub async fn mint_tokens(
        &self,
        keypair: &Keypair,
        mint: Pubkey,
        amount: u64,
    ) -> Result<()> {
        use solana_sdk::instruction::Instruction;

        // Free mint instruction discriminator (from Anchor IDL)
        const FREE_MINT_DISCRIMINATOR: [u8; 8] = [164, 250, 205, 24, 232, 61, 200, 237];

        // Derive PDAs
        let (mint_state, _) = Pubkey::find_program_address(
            &[b"state"],
            &self.program_id,
        );

        let (mint_tracker, _) = Pubkey::find_program_address(
            &[b"mint_tracker", keypair.pubkey().as_ref()],
            &self.program_id,
        );

        let (mint_authority, _) = Pubkey::find_program_address(
            &[b"mint_authority"],
            &self.program_id,
        );

        // Get user's token account (ATA)
        let user_token_account = get_associated_token_address(
            &keypair.pubkey(),
            &mint,
            &TOKEN_2022_PROGRAM_ID,
        );

        // Get mint state to find dev wallet
        let mint_state_account = self.rpc_client.get_account(&mint_state)
            .context("Mint state not found - program may not be initialized")?;

        // Parse dev_wallet from mint_state (offset 41 for 32-byte pubkey)
        let dev_wallet = Pubkey::try_from(&mint_state_account.data[41..73])
            .context("Failed to parse dev wallet from mint state")?;

        // Build instruction data: discriminator + amount (u64)
        let mut instruction_data = Vec::new();
        instruction_data.extend_from_slice(&FREE_MINT_DISCRIMINATOR);
        instruction_data.extend_from_slice(&amount.to_le_bytes());

        // Build accounts list
        let accounts = vec![
            solana_sdk::instruction::AccountMeta::new(keypair.pubkey(), true),
            solana_sdk::instruction::AccountMeta::new(mint_state, false),
            solana_sdk::instruction::AccountMeta::new(mint_tracker, false),
            solana_sdk::instruction::AccountMeta::new_readonly(mint_authority, false),
            solana_sdk::instruction::AccountMeta::new(mint, false),
            solana_sdk::instruction::AccountMeta::new(user_token_account, false),
            solana_sdk::instruction::AccountMeta::new(dev_wallet, false),
            solana_sdk::instruction::AccountMeta::new_readonly(TOKEN_2022_PROGRAM_ID, false),
            solana_sdk::instruction::AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ];

        let instruction = Instruction {
            program_id: self.program_id,
            accounts,
            data: instruction_data,
        };

        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&keypair.pubkey()),
            &[keypair],
            recent_blockhash,
        );

        println!("Sending mint transaction...");
        let signature = self.rpc_client.send_and_confirm_transaction(&transaction)?;

        println!();
        println!("{}", "✅ Tokens Minted Successfully!".green().bold());
        println!("   Transaction: {}", signature.to_string().cyan());
        println!("   View on Solscan: https://solscan.io/tx/{}?cluster=devnet", signature);
        println!();

        Ok(())
    }
}
