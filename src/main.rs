use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use comfy_table::{Table, presets::UTF8_FULL};
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use solana_sdk::{pubkey::Pubkey, signature::Keypair};
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

mod crypto;
mod solana;

use crypto::sphincs::SphincsKeyManager;
use solana::client::VaultClient;

#[derive(Serialize, Deserialize, Default)]
struct VaultConfig {
    keypair_path: Option<String>,
}

#[derive(Parser)]
#[command(name = "qdum-vault")]
#[command(author, version)]
#[command(about = "⚛️  Quantum-Resistant Vault CLI")]
#[command(long_about = None)]
#[command(after_help = format!("\n{}\n\n{}\n  {} {}\n  {} {}\n  {} {}\n  {} {}\n\n{}\n  {}\n\n{}\n  {} {}\n  {} {}\n  {} {}\n\n{}\n  {}\n  {}\n",
    "╔═══════════════════════════════════════════════════════════╗".bright_cyan(),
    "GETTING STARTED:".bright_magenta().bold(),
    "1.".bright_cyan(), "qdum-vault init".bright_white(),
    "2.".bright_cyan(), "qdum-vault register".bright_white(),
    "3.".bright_cyan(), "qdum-vault lock".bright_white(),
    "4.".bright_cyan(), "qdum-vault unlock".bright_white(),
    "SECURITY:".bright_magenta().bold(),
    "SPHINCS+-SHA2-128s (NIST FIPS 205)".bright_cyan(),
    "NETWORK:".bright_magenta().bold(),
    "RPC:".bright_blue(), "https://api.devnet.solana.com".dimmed(),
    "Program:".bright_blue(), "HyC27AVHW4VwkEiWwWxevaUpvkiAqPUueaa94og9HmLQ".dimmed(),
    "Keys:".bright_blue(), "~/.qdum/".dimmed(),
    "EXAMPLES:".bright_magenta().bold(),
    "qdum-vault init                    # Initialize quantum keypair".dimmed(),
    "qdum-vault unlock                  # 44-tx quantum verification".dimmed(),
))]
#[command(styles = get_styles())]
struct Cli {
    /// RPC endpoint URL (defaults to devnet)
    #[arg(long, default_value = "https://api.devnet.solana.com")]
    rpc_url: String,

    /// Program ID
    #[arg(long, default_value = "HyC27AVHW4VwkEiWwWxevaUpvkiAqPUueaa94og9HmLQ")]
    program_id: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate SPHINCS+ keys and Solana keypair (all-in-one setup)
    Init {
        /// Output directory for keys (defaults to ~/.qdum/)
        #[arg(long)]
        output_dir: Option<String>,
    },

    /// Configure default settings (keypair path, etc.)
    Config {
        /// Set default Solana keypair path
        #[arg(long)]
        keypair: Option<String>,

        /// Show current configuration
        #[arg(long)]
        show: bool,
    },

    /// Register your SPHINCS+ public key on-chain
    Register {
        /// Path to your Solana wallet keypair JSON file (optional, uses configured path or ~/.config/solana/id.json)
        #[arg(long)]
        keypair: Option<String>,

        /// Path to SPHINCS+ public key file (optional, defaults to ~/.qdum/sphincs_public.key)
        #[arg(long)]
        sphincs_pubkey: Option<String>,
    },

    /// Lock your vault (generate challenge)
    Lock {
        /// Path to your Solana wallet keypair JSON file (optional, uses configured path or ~/.config/solana/id.json)
        #[arg(long)]
        keypair: Option<String>,
    },

    /// Unlock your vault (11-step verification process)
    Unlock {
        /// Path to your Solana wallet keypair JSON file (optional, uses configured path or ~/.config/solana/id.json)
        #[arg(long)]
        keypair: Option<String>,

        /// Path to SPHINCS+ private key file (optional, defaults to ~/.qdum/sphincs_private.key)
        #[arg(long)]
        sphincs_privkey: Option<String>,
    },

    /// Check vault status
    Status {
        /// Path to your Solana wallet keypair JSON file (optional, uses configured path or ~/.config/solana/id.json)
        #[arg(long)]
        keypair: Option<String>,
    },

    /// Check token balance
    Balance {
        /// Path to your Solana wallet keypair JSON file (optional, uses configured path or ~/.config/solana/id.json)
        #[arg(long)]
        keypair: Option<String>,

        /// Mint address (defaults to QDUM devnet mint)
        #[arg(long, default_value = "3V6ogu16de86nChsmC5wHMKJmCx5YdGXA6fbp3y3497n")]
        mint: String,
    },

    /// Transfer QDUM tokens to another wallet
    Transfer {
        /// Path to your Solana wallet keypair JSON file (optional, uses configured path or ~/.config/solana/id.json)
        #[arg(long)]
        keypair: Option<String>,

        /// Recipient wallet address
        #[arg(long)]
        to: String,

        /// Amount of QDUM tokens to transfer (in base units with 6 decimals)
        #[arg(long)]
        amount: u64,

        /// Mint address (defaults to QDUM devnet mint)
        #[arg(long, default_value = "3V6ogu16de86nChsmC5wHMKJmCx5YdGXA6fbp3y3497n")]
        mint: String,
    },
}

fn get_styles() -> clap::builder::Styles {
    use clap::builder::styling::*;
    clap::builder::Styles::styled()
        .header(AnsiColor::BrightMagenta.on_default().bold())
        .usage(AnsiColor::BrightCyan.on_default().bold())
        .literal(AnsiColor::BrightGreen.on_default())
        .placeholder(AnsiColor::Magenta.on_default())
        .error(AnsiColor::BrightRed.on_default().bold())
        .valid(AnsiColor::BrightCyan.on_default())
        .invalid(AnsiColor::BrightYellow.on_default())
}

fn print_banner() {
    println!();
    println!("{}", "╔═══════════════════════════════════════════════════════════╗".on_black().bright_magenta());
    println!("{}", "║                                                           ║".on_black().bright_magenta());
    println!("{}", "║   ██████╗ ██████╗ ██╗   ██╗███╗   ███╗                   ║".on_black().bright_cyan().bold());
    println!("{}", "║  ██╔═══██╗██╔══██╗██║   ██║████╗ ████║                   ║".on_black().bright_cyan().bold());
    println!("{}", "║  ██║   ██║██║  ██║██║   ██║██╔████╔██║                   ║".on_black().bright_cyan().bold());
    println!("{}", "║  ██║▄▄ ██║██║  ██║██║   ██║██║╚██╔╝██║                   ║".on_black().bright_cyan().bold());
    println!("{}", "║  ╚██████╔╝██████╔╝╚██████╔╝██║ ╚═╝ ██║                   ║".on_black().bright_cyan().bold());
    println!("{}", "║   ╚══▀▀═╝ ╚═════╝  ╚═════╝ ╚═╝     ╚═╝                   ║".on_black().bright_cyan().bold());
    println!("{}", "║                                                           ║".on_black().bright_magenta());
    println!("{}", "║          ⚛️  Q U A N T U M   V A U L T  ⚛️              ║".on_black().bright_magenta().bold());
    println!("{}", "║              [ SPHINCS+ POST-QUANTUM ]                    ║".on_black().bright_green());
    println!("{}", "║                                                           ║".on_black().bright_magenta());
    println!("{}", "╚═══════════════════════════════════════════════════════════╝".on_black().bright_magenta());
    println!();
    println!("{}", "  ▓▒░ CYBERSECURITY STATUS ░▒▓".on_black().bright_cyan().bold());
    println!("{} {}", "  ┃ SECURITY:".on_black().bright_magenta().bold(), "NIST FIPS 205 [ACTIVE]".on_black().bright_green());
    println!("{} {}", "  ┃ NETWORK: ".on_black().bright_magenta().bold(), "SOLANA DEVNET".on_black().bright_cyan());
    println!("{} {}", "  ┃ QUANTUM:".on_black().bright_magenta().bold(), "RESISTANT ✓".on_black().bright_green().bold());
    println!();
}

fn get_config_path() -> PathBuf {
    let home = dirs::home_dir().expect("Could not determine home directory");
    home.join(".qdum").join("config.json")
}

fn load_config() -> VaultConfig {
    let config_path = get_config_path();

    if config_path.exists() {
        if let Ok(data) = fs::read_to_string(&config_path) {
            if let Ok(config) = serde_json::from_str(&data) {
                return config;
            }
        }
    }

    VaultConfig::default()
}

fn save_config(config: &VaultConfig) -> Result<()> {
    let config_path = get_config_path();

    // Ensure .qdum directory exists
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(config)?;
    fs::write(&config_path, json)?;

    Ok(())
}

fn get_default_keypair_path() -> String {
    let config = load_config();

    if let Some(path) = config.keypair_path {
        return path;
    }

    // Fallback to default Solana path
    let home = dirs::home_dir().expect("Could not determine home directory");
    home.join(".config/solana/id.json")
        .to_str()
        .expect("Invalid path")
        .to_string()
}

fn load_keypair_and_extract_wallet(keypair_path: &str) -> Result<(String, Pubkey)> {
    use solana_sdk::signature::Signer;

    let data = fs::read_to_string(keypair_path)
        .context(format!("Failed to read keypair file: {}", keypair_path))?;

    let bytes: Vec<u8> = serde_json::from_str(&data)
        .context("Invalid keypair JSON format")?;

    let keypair = Keypair::try_from(&bytes[..])
        .context("Invalid keypair bytes")?;

    let wallet_pubkey = keypair.pubkey();

    Ok((keypair_path.to_string(), wallet_pubkey))
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Print banner for all commands except help
    print_banner();

    match cli.command {
        Commands::Init { output_dir } => {
            println!("{}", "▓▒░ KEY GENERATION ░▒▓".on_black().bright_cyan().bold());
            println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".on_black().bright_magenta());
            println!();

            cmd_init(output_dir).await?;
        }

        Commands::Config { keypair, show } => {
            println!("{}", "▓▒░ CONFIGURATION ░▒▓".on_black().bright_cyan().bold());
            println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".on_black().bright_magenta());
            println!();

            let mut config = load_config();

            if let Some(kp_path) = keypair {
                // Validate the keypair file exists
                if !std::path::Path::new(&kp_path).exists() {
                    eprintln!("{} Keypair file not found: {}", "Error:".red().bold(), kp_path);
                    std::process::exit(1);
                }

                config.keypair_path = Some(kp_path.clone());
                save_config(&config)?;

                println!("{} Default keypair path set to:", "✓".green().bold());
                println!("  {}", kp_path.yellow());
                println!();
                println!("Saved to: {}", get_config_path().display().to_string().dimmed());
            } else if show {
                println!("{}", "Current Configuration:".bold());
                println!();
                println!("{} {}", "Config file:".bold(), get_config_path().display().to_string().dimmed());
                println!("{} {}", "Keypair path:".bold(),
                    config.keypair_path.as_deref().unwrap_or("~/.config/solana/id.json (default)").yellow());
            } else {
                println!("{}", "Usage:".bold());
                println!("  qdum-vault config --keypair <PATH>  # Set default keypair");
                println!("  qdum-vault config --show            # Show current config");
            }
        }

        Commands::Register {
            keypair,
            sphincs_pubkey,
        } => {
            println!("{}", "▓▒░ REGISTER PQ ACCOUNT ░▒▓".on_black().bright_cyan().bold());
            println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".on_black().bright_magenta());
            println!();

            let program_id = Pubkey::from_str(&cli.program_id)?;

            // Auto-detect keypair and wallet
            let keypair_path = keypair.unwrap_or_else(|| get_default_keypair_path());
            let (kp_path, wallet_pubkey) = load_keypair_and_extract_wallet(&keypair_path)?;

            println!("{} {}", "Using keypair:".bold(), kp_path.dimmed());
            println!("{} {}", "Wallet:       ".bold(), wallet_pubkey.to_string().yellow());
            println!();

            cmd_register(
                &cli.rpc_url,
                program_id,
                wallet_pubkey,
                &kp_path,
                sphincs_pubkey,
            )
            .await?;
        }

        Commands::Lock { keypair } => {
            println!("{}", "▓▒░ VAULT LOCKDOWN ░▒▓".on_black().bright_red().bold());
            println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".on_black().bright_magenta());
            println!();

            let program_id = Pubkey::from_str(&cli.program_id)?;

            // Auto-detect keypair and wallet
            let keypair_path = keypair.unwrap_or_else(|| get_default_keypair_path());
            let (kp_path, wallet_pubkey) = load_keypair_and_extract_wallet(&keypair_path)?;

            println!("{} {}", "Using keypair:".bold(), kp_path.dimmed());
            println!("{} {}", "Wallet:       ".bold(), wallet_pubkey.to_string().yellow());
            println!();

            cmd_lock(&cli.rpc_url, program_id, wallet_pubkey, &kp_path).await?;
        }

        Commands::Unlock {
            keypair,
            sphincs_privkey,
        } => {
            println!("{}", "▓▒░ VAULT UNLOCK ░▒▓".on_black().bright_green().bold());
            println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".on_black().bright_magenta());
            println!();

            let program_id = Pubkey::from_str(&cli.program_id)?;

            // Auto-detect keypair and wallet
            let keypair_path = keypair.unwrap_or_else(|| get_default_keypair_path());
            let (kp_path, wallet_pubkey) = load_keypair_and_extract_wallet(&keypair_path)?;

            println!("{} {}", "Using keypair:".bold(), kp_path.dimmed());
            println!("{} {}", "Wallet:       ".bold(), wallet_pubkey.to_string().yellow());
            println!();

            cmd_unlock(
                &cli.rpc_url,
                program_id,
                wallet_pubkey,
                &kp_path,
                sphincs_privkey,
            )
            .await?;
        }

        Commands::Status { keypair } => {
            println!("{}", "▓▒░ STATUS CHECK ░▒▓".on_black().bright_cyan().bold());
            println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".on_black().bright_magenta());
            println!();

            let program_id = Pubkey::from_str(&cli.program_id)?;

            // Auto-detect keypair and wallet
            let keypair_path = keypair.unwrap_or_else(|| get_default_keypair_path());
            let (kp_path, wallet_pubkey) = load_keypair_and_extract_wallet(&keypair_path)?;

            println!("{} {}", "Using keypair:".bold(), kp_path.dimmed());
            println!("{} {}", "Wallet:       ".bold(), wallet_pubkey.to_string().yellow());
            println!();

            cmd_status(&cli.rpc_url, program_id, wallet_pubkey).await?;
        }

        Commands::Balance { keypair, mint } => {
            println!("{}", "▓▒░ BALANCE CHECK ░▒▓".on_black().bright_cyan().bold());
            println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".on_black().bright_magenta());
            println!();

            // Auto-detect keypair and wallet
            let keypair_path = keypair.unwrap_or_else(|| get_default_keypair_path());
            let (kp_path, wallet_pubkey) = load_keypair_and_extract_wallet(&keypair_path)?;

            println!("{} {}", "Using keypair:".bold(), kp_path.dimmed());
            println!("{} {}", "Wallet:       ".bold(), wallet_pubkey.to_string().yellow());
            println!();

            let mint_pubkey = Pubkey::from_str(&mint)?;

            cmd_balance(&cli.rpc_url, wallet_pubkey, mint_pubkey).await?;
        }

        Commands::Transfer { keypair, to, amount, mint } => {
            println!("{}", "▓▒░ TOKEN TRANSFER ░▒▓".on_black().bright_yellow().bold());
            println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".on_black().bright_magenta());
            println!();

            let program_id = Pubkey::from_str(&cli.program_id)?;

            // Auto-detect keypair and wallet
            let keypair_path = keypair.unwrap_or_else(|| get_default_keypair_path());
            let (kp_path, wallet_pubkey) = load_keypair_and_extract_wallet(&keypair_path)?;

            println!("{} {}", "Using keypair:".bold(), kp_path.dimmed());
            println!("{} {}", "From:         ".bold(), wallet_pubkey.to_string().yellow());
            println!();

            let recipient = Pubkey::from_str(&to)?;
            let mint_pubkey = Pubkey::from_str(&mint)?;

            cmd_transfer(&cli.rpc_url, program_id, wallet_pubkey, &kp_path, recipient, mint_pubkey, amount).await?;
        }

    }

    Ok(())
}

async fn cmd_init(output_dir: Option<String>) -> Result<()> {
    use std::io::{self, Write};
    use solana_sdk::signature::{Keypair, Signer};

    println!("{}", "╔═══════════════════════════════════════════════════════════╗".on_black().bright_magenta());
    println!("{}", "║                                                           ║".on_black().bright_magenta());
    println!("{}", "║    ⚛️  INITIALIZING QUANTUM KEYPAIR GENERATION ⚛️        ║".on_black().bright_cyan().bold());
    println!("{}", "║                                                           ║".on_black().bright_magenta());
    println!("{}", "╚═══════════════════════════════════════════════════════════╝".on_black().bright_magenta());
    println!();

    // Spinner for SPHINCS+ key generation
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner:.magenta} {msg}")
            .unwrap()
    );
    spinner.enable_steady_tick(Duration::from_millis(80));
    spinner.set_message("⚛️  Generating SPHINCS+ quantum-resistant keypair...".to_string());

    // Generate SPHINCS+ keys
    let key_manager = SphincsKeyManager::new(output_dir.clone())?;
    key_manager.generate_and_save_keypair()?;

    spinner.finish_with_message(format!("{}", "✓ SPHINCS+ keypair generated".bright_green()));

    // Spinner for Solana keypair
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner:.cyan} {msg}")
            .unwrap()
    );
    spinner.enable_steady_tick(Duration::from_millis(80));
    spinner.set_message("🔑 Generating Solana wallet keypair...".to_string());

    // Generate Solana keypair
    let solana_keypair = Keypair::new();
    let wallet_address = solana_keypair.pubkey();

    let qdum_dir = if let Some(ref dir) = output_dir {
        PathBuf::from(dir)
    } else {
        dirs::home_dir()
            .expect("Could not determine home directory")
            .join(".qdum")
    };

    let keypair_path = qdum_dir.join("solana-keypair.json");
    let keypair_bytes = solana_keypair.to_bytes();
    let keypair_json = serde_json::to_string(&keypair_bytes.to_vec())?;
    fs::write(&keypair_path, keypair_json)?;

    spinner.finish_with_message(format!("{}", "✓ Solana keypair created".bright_green()));

    // Summary table
    println!();
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table
        .set_header(vec![
            "Component".bright_white().to_string(),
            "Location".bright_white().to_string(),
            "Size".bright_white().to_string()
        ])
        .add_row(vec![
            "Private Key".bright_cyan().to_string(),
            "~/.qdum/sphincs_private.key".bright_white().to_string(),
            "64 bytes".bright_yellow().to_string()
        ])
        .add_row(vec![
            "Public Key".bright_cyan().to_string(),
            "~/.qdum/sphincs_public.key".bright_white().to_string(),
            "32 bytes".bright_yellow().to_string()
        ])
        .add_row(vec![
            "Solana Keypair".bright_cyan().to_string(),
            keypair_path.display().to_string().bright_white().to_string(),
            "JSON".bright_yellow().to_string()
        ]);

    println!("{}", table);
    println!();
    println!("{} {}", "Wallet Address:".bright_blue().bold(), wallet_address.to_string().bright_magenta());
    println!();

    // Ask if they want to set it as default
    println!("{}", "╔═══════════════════════════════════════════════════════════╗".on_black().bright_green());
    println!("{}", "║          ✓ INITIALIZATION COMPLETE [SUCCESS]             ║".on_black().bright_green().bold());
    println!("{}", "╚═══════════════════════════════════════════════════════════╝".on_black().bright_green());
    println!();

    print!("{} ", "Set this as your default keypair? (y/n):".bright_yellow().bold());
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let answer = input.trim().to_lowercase();

    if answer == "y" || answer == "yes" {
        let mut config = load_config();
        config.keypair_path = Some(keypair_path.to_str().unwrap().to_string());
        save_config(&config)?;

        println!();
        println!("{} {}", "✓".bright_green().bold(), "Default keypair configured!".bright_white().bold());
        println!("  All commands will now use: {}", keypair_path.display().to_string().bright_magenta());
    } else {
        println!();
        println!("  You can set it later with:");
        println!("  {}", format!("qdum-vault config --keypair {}", keypair_path.display()).dimmed());
    }

    println!();
    println!("{} {}", "Next step:".bright_yellow().bold(), "qdum-vault register".bright_white().bold());
    println!();

    Ok(())
}

async fn cmd_register(
    rpc_url: &str,
    program_id: Pubkey,
    wallet: Pubkey,
    keypair_path: &str,
    sphincs_pubkey_path: Option<String>,
) -> Result<()> {
    let key_manager = SphincsKeyManager::new(None)?;
    let sphincs_pubkey = key_manager.load_public_key(sphincs_pubkey_path)?;

    let mut client = VaultClient::new(rpc_url, program_id)?;
    client.register_pq_account(wallet, keypair_path, &sphincs_pubkey).await?;

    Ok(())
}

async fn cmd_lock(
    rpc_url: &str,
    program_id: Pubkey,
    wallet: Pubkey,
    keypair_path: &str,
) -> Result<()> {
    let mut client = VaultClient::new(rpc_url, program_id)?;
    client.lock_vault(wallet, keypair_path).await?;

    Ok(())
}

async fn cmd_unlock(
    rpc_url: &str,
    program_id: Pubkey,
    wallet: Pubkey,
    keypair_path: &str,
    sphincs_privkey_path: Option<String>,
) -> Result<()> {
    let key_manager = SphincsKeyManager::new(None)?;
    let sphincs_privkey = key_manager.load_private_key(sphincs_privkey_path)?;

    let mut client = VaultClient::new(rpc_url, program_id)?;
    client.unlock_vault(wallet, keypair_path, &sphincs_privkey).await?;

    Ok(())
}

async fn cmd_status(rpc_url: &str, program_id: Pubkey, wallet: Pubkey) -> Result<()> {
    let client = VaultClient::new(rpc_url, program_id)?;
    client.check_status(wallet).await?;

    Ok(())
}

async fn cmd_balance(rpc_url: &str, wallet: Pubkey, mint: Pubkey) -> Result<()> {
    let client = VaultClient::new(rpc_url, Pubkey::default())?;
    client.check_balance(wallet, mint).await?;

    Ok(())
}


async fn cmd_transfer(
    rpc_url: &str,
    program_id: Pubkey,
    _from_wallet: Pubkey,
    keypair_path: &str,
    to_wallet: Pubkey,
    mint: Pubkey,
    amount: u64,
) -> Result<()> {
    let client = VaultClient::new(rpc_url, program_id)?;

    let data = fs::read_to_string(keypair_path)
        .context(format!("Failed to read keypair file: {}", keypair_path))?;
    let bytes: Vec<u8> = serde_json::from_str(&data)
        .context("Invalid keypair JSON format")?;
    let keypair = Keypair::try_from(&bytes[..])
        .context("Invalid keypair bytes")?;

    client.transfer_tokens(&keypair, to_wallet, mint, amount).await?;

    Ok(())
}

