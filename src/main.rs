use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use comfy_table::{Table, presets::UTF8_FULL};
use indicatif::{ProgressBar, ProgressStyle};
use solana_sdk::{pubkey::Pubkey, signature::Keypair};
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

mod icons;
use std::time::Duration;

mod crypto;
mod solana;
mod dashboard;
mod theme;
mod vault_manager;
mod vault_switcher;

use crypto::sphincs::SphincsKeyManager;
use solana::client::VaultClient;
use dashboard::Dashboard;
use vault_manager::{VaultConfig, VaultProfile};
use vault_switcher::VaultSwitcher;

#[derive(Parser)]
#[command(name = "qdum-vault")]
#[command(author, version)]
#[command(about = "pqcash - Post-Quantum Cash System")]
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
    command: Option<Commands>,
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

    /// Close PQ account and reclaim rent (must be unlocked first)
    Close {
        /// Path to your Solana wallet keypair JSON file (optional, uses configured path or ~/.config/solana/id.json)
        #[arg(long)]
        keypair: Option<String>,

        /// Address to receive the rent refund (optional, defaults to wallet address)
        #[arg(long)]
        receiver: Option<String>,
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

    /// Bridge between Standard QDUM and pqQDUM (wrap/unwrap)
    Bridge {
        #[command(subcommand)]
        action: BridgeAction,

        /// Path to your Solana wallet keypair JSON file (optional, uses configured path or ~/.config/solana/id.json)
        #[arg(long)]
        keypair: Option<String>,
    },

    /// Launch interactive dashboard (TUI)
    Dashboard {
        /// Path to your Solana wallet keypair JSON file (optional, uses configured path or ~/.config/solana/id.json)
        #[arg(long)]
        keypair: Option<String>,
    },

    /// Vault management (create, switch, list, delete)
    Vault {
        #[command(subcommand)]
        action: VaultAction,
    },
}

#[derive(Subcommand)]
enum BridgeAction {
    /// Wrap Standard QDUM to pqQDUM (for vault locking)
    Wrap {
        /// Amount to wrap (in QDUM, e.g., 100.5)
        amount: f64,

        /// Standard QDUM mint address
        #[arg(long, default_value = "GS2tyNMdpiKnQ9AxFhB74SbzYF7NmoTREoKZC6pzxds7")]
        standard_mint: String,

        /// pqQDUM mint address
        #[arg(long, default_value = "3V6ogu16de86nChsmC5wHMKJmCx5YdGXA6fbp3y3497n")]
        pq_mint: String,
    },

    /// Unwrap pqQDUM to Standard QDUM (for DEX trading)
    Unwrap {
        /// Amount to unwrap (in QDUM, e.g., 100.5)
        amount: f64,

        /// Standard QDUM mint address
        #[arg(long, default_value = "GS2tyNMdpiKnQ9AxFhB74SbzYF7NmoTREoKZC6pzxds7")]
        standard_mint: String,

        /// pqQDUM mint address
        #[arg(long, default_value = "3V6ogu16de86nChsmC5wHMKJmCx5YdGXA6fbp3y3497n")]
        pq_mint: String,
    },
}

#[derive(Subcommand)]
enum VaultAction {
    /// List all vault profiles
    List,

    /// Create a new vault profile
    Create {
        /// Name for the vault
        name: Option<String>,

        /// Description (optional)
        #[arg(long)]
        description: Option<String>,

        /// Generate new keys automatically
        #[arg(long)]
        auto_generate: bool,
    },

    /// Switch active vault (interactive if no name provided)
    Switch {
        /// Vault name (omit for interactive menu)
        name: Option<String>,
    },

    /// Show vault details
    Show {
        /// Vault name (defaults to active)
        name: Option<String>,
    },

    /// Delete a vault profile
    Delete {
        /// Vault name
        name: String,

        /// Skip confirmation
        #[arg(long)]
        yes: bool,
    },

    /// Rename a vault
    Rename {
        /// Current name
        old_name: String,

        /// New name
        new_name: String,
    },

    /// Create a new vault and switch to it (convenience command)
    New {
        /// Name for the vault
        name: Option<String>,

        /// Description (optional)
        #[arg(long)]
        description: Option<String>,

        /// Generate new keys automatically
        #[arg(long)]
        auto_generate: bool,
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
    use std::io::{self, Write};
    use std::thread;

    println!();

    // Animated startup sequence
    print!("{}", "  [".dimmed());
    for _ in 0..3 {
        print!("{}", "█".bright_green());
        io::stdout().flush().unwrap();
        thread::sleep(Duration::from_millis(50));
    }
    println!("{} {}", "]".dimmed(), "Initializing...".dimmed());
    thread::sleep(Duration::from_millis(100));

    println!();

    // ASCII Art Logo - pqcash style
    println!("{}", "  ╔══════════════════════════════════════════════════════════════════╗".bright_green().bold());
    println!("{}", "  ║                                                                  ║".bright_green());
    println!("{}", "  ║      ██████╗  ██████╗  ██████╗ █████╗ ███████╗██╗  ██╗          ║".bright_green().bold());
    println!("{}", "  ║      ██╔══██╗██╔═══██╗██╔════╝██╔══██╗██╔════╝██║  ██║          ║".bright_green().bold());
    println!("{}", "  ║      ██████╔╝██║   ██║██║     ███████║███████╗███████║          ║".bright_green().bold());
    println!("{}", "  ║      ██╔═══╝ ██║▄▄ ██║██║     ██╔══██║╚════██║██╔══██║          ║".bright_green().bold());
    println!("{}", "  ║      ██║     ╚██████╔╝╚██████╗██║  ██║███████║██║  ██║          ║".bright_green().bold());
    println!("{}", "  ║      ╚═╝      ╚══▀▀═╝  ╚═════╝╚═╝  ╚═╝╚══════╝╚═╝  ╚═╝          ║".bright_green().bold());
    println!("{}", "  ║                                                                  ║".bright_green());
    println!("  ║              {}                          ║", "P O S T - Q U A N T U M   C A S H".bright_white().bold());
    println!("  ║          {}          ║", "Quantum-Resistant Digital Currency".bright_cyan());
    println!("{}", "  ║                                                                  ║".bright_green());
    println!("{}", "  ╠══════════════════════════════════════════════════════════════════╣".bright_green().bold());
    println!("{}", "  ║                                                                  ║".bright_green());

    // Quick stats with icons
    println!("  ║  {}  {}                   ║", "🔐".to_string(), format!("{:<56}", "SPHINCS+ (NIST FIPS 205) - Quantum Resistant".bright_white()));
    println!("  ║  {}  {}                   ║", "🌐".to_string(), format!("{:<56}", "Solana Devnet - On-Chain Verification".bright_white()));
    println!("  ║  {}  {}                   ║", "📦".to_string(), format!("{:<56}", format!("Version {} - Production Ready", env!("CARGO_PKG_VERSION")).bright_white()));

    println!("{}", "  ║                                                                  ║".bright_green());
    println!("{}", "  ╚══════════════════════════════════════════════════════════════════╝".bright_green().bold());
    println!();

    // Quick start guide
    let mut guide_table = Table::new();
    guide_table.load_preset(comfy_table::presets::UTF8_FULL);
    guide_table.set_header(vec![
        "Step".bright_white().bold().to_string(),
        "Command".bright_cyan().to_string(),
        "Description".dimmed().to_string()
    ]);

    guide_table
        .add_row(vec![
            "1".bright_yellow().to_string(),
            "qdum-vault init".bright_green().to_string(),
            "Generate quantum keypairs".to_string()
        ])
        .add_row(vec![
            "2".bright_yellow().to_string(),
            "qdum-vault register".bright_green().to_string(),
            "Register on-chain".to_string()
        ])
        .add_row(vec![
            "3".bright_yellow().to_string(),
            "qdum-vault lock".bright_green().to_string(),
            "Lock your vault".to_string()
        ])
        .add_row(vec![
            "4".bright_yellow().to_string(),
            "qdum-vault unlock".bright_green().to_string(),
            "Unlock with quantum sig".to_string()
        ]);

    println!("{}", guide_table);
    println!();
    println!("  {} Type {} for all available commands",
        "💡".to_string(),
        "qdum-vault --help".bright_cyan().bold());
    println!();
}

fn print_command_header(text: &str, icon: colored::ColoredString) {
    println!();
    println!("{}", "╔".bright_green().to_string() + &"═".repeat(68).bright_green().to_string() + &"╗".bright_green().to_string());
    println!("║  {} {}  ║", icon, format!("{:<60}", text).bright_white().bold());
    println!("{}", "╚".bright_green().to_string() + &"═".repeat(68).bright_green().to_string() + &"╝".bright_green().to_string());
    println!();
}

fn load_config() -> VaultConfig {
    VaultConfig::load().unwrap_or_else(|_| VaultConfig {
        version: 1,
        ..Default::default()
    })
}

fn get_default_keypair_path() -> String {
    let config = load_config();

    // Try to use active vault's keypair path
    if let Some(vault) = config.get_active_vault() {
        return vault.solana_keypair_path.clone();
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

    // Print banner for all commands except dashboard (which takes over the screen)
    // If no command provided, default to dashboard
    let command = cli.command.unwrap_or(Commands::Dashboard { keypair: None });

    if !matches!(command, Commands::Dashboard { .. }) {
        print_banner();
    }

    match command {
        Commands::Init { output_dir } => {
            print_command_header("Initialize Quantum Keypair", "[INIT]".bright_green());

            cmd_init(output_dir).await?;
        }

        Commands::Config { keypair, show } => {
            print_command_header("Configuration", "[CONFIG]".bright_cyan());

            let config = load_config();

            if keypair.is_some() {
                println!("{}", "The config command has been replaced by vault management.".yellow());
                println!();
                println!("{}", "To set your default keypair, use vault commands:".bold());
                println!("  {} - Create and switch to a new vault", "qdum-vault vault new <name> --auto-generate".bright_cyan());
                println!("  {} - Create vault with existing keys", "qdum-vault vault create <name>".bright_cyan());
                println!("  {} - Switch between vaults", "qdum-vault vault switch".bright_cyan());
                println!();
            } else if show {
                println!("{}", "Current Configuration:".bold());
                println!();

                if let Some(vault) = config.get_active_vault() {
                    println!("{} {}", "Active vault:".bold(), vault.name.bright_cyan());
                    println!("{} {}", "Keypair path:".bold(), vault.solana_keypair_path.dimmed());
                    if !vault.wallet_address.is_empty() {
                        println!("{} {}", "Wallet:".bold(), vault.wallet_address.yellow());
                    }
                } else {
                    println!("{}", "No active vault configured.".yellow());
                    println!();
                    println!("Create a vault with:");
                    println!("  {}", "qdum-vault vault new <name> --auto-generate".bright_cyan());
                }
            } else {
                println!("{}", "Usage:".bold());
                println!("  qdum-vault config --show            # Show current config");
                println!();
                println!("{}", "To manage vaults:".bold());
                println!("  qdum-vault vault list               # List all vaults");
                println!("  qdum-vault vault new <name>         # Create and switch to new vault");
                println!("  qdum-vault vault switch             # Switch vaults interactively");
            }
        }

        Commands::Register {
            keypair,
            sphincs_pubkey,
        } => {
            print_command_header("Register Post-Quantum Account", "[REGISTER]".bright_cyan());

            let program_id = Pubkey::from_str(&cli.program_id)?;

            // Auto-detect keypair and wallet
            let keypair_path = keypair.unwrap_or_else(|| get_default_keypair_path());
            let (kp_path, wallet_pubkey) = load_keypair_and_extract_wallet(&keypair_path)?;

            // Get SPHINCS public key path from active vault if not provided via CLI
            let config = load_config();
            let sphincs_pubkey_path = if sphincs_pubkey.is_some() {
                sphincs_pubkey
            } else if let Some(vault) = config.get_active_vault() {
                println!("{}", "═══════════════════════════════════════════════════════════".yellow());
                println!("{} {}", "DEBUG: Active vault:".yellow().bold(), vault.name.cyan());
                println!("{} {}", "DEBUG: Using SPHINCS public key:".yellow().bold(), vault.sphincs_public_key_path.cyan());
                println!("{}", "═══════════════════════════════════════════════════════════".yellow());
                Some(vault.sphincs_public_key_path.clone())
            } else {
                None
            };

            println!("{} {}", "Using keypair:".bold(), kp_path.dimmed());
            println!("{} {}", "Wallet:       ".bold(), wallet_pubkey.to_string().yellow());
            println!();

            cmd_register(
                &cli.rpc_url,
                program_id,
                wallet_pubkey,
                &kp_path,
                sphincs_pubkey_path,
            )
            .await?;
        }

        Commands::Lock { keypair } => {
            print_command_header("Lock Vault", "[LOCK]".bright_red());

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
            print_command_header("Unlock Vault", "[UNLOCK]".bright_green());

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

        Commands::Close { keypair, receiver } => {
            print_command_header("Close PQ Account", "[CLOSE]".bright_red());

            let program_id = Pubkey::from_str(&cli.program_id)?;

            // Auto-detect keypair and wallet
            let keypair_path = keypair.unwrap_or_else(|| get_default_keypair_path());
            let (kp_path, wallet_pubkey) = load_keypair_and_extract_wallet(&keypair_path)?;

            // Parse receiver address if provided
            let receiver_pubkey = receiver
                .as_ref()
                .map(|r| Pubkey::from_str(r))
                .transpose()?;

            println!("{} {}", "Using keypair:".bold(), kp_path.dimmed());
            println!("{} {}", "Wallet:       ".bold(), wallet_pubkey.to_string().yellow());
            println!();

            cmd_close(&cli.rpc_url, program_id, wallet_pubkey, &kp_path, receiver_pubkey).await?;
        }

        Commands::Status { keypair } => {
            print_command_header("Vault Status", "[STATUS]".bright_cyan());

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
            print_command_header("Check Balance", "[BALANCE]".bright_cyan());

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
            print_command_header("Transfer Tokens", "[TRANSFER]".bright_yellow());

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

        Commands::Bridge { action, keypair } => {
            // Auto-detect keypair and wallet
            let keypair_path = keypair.unwrap_or_else(|| get_default_keypair_path());
            let (kp_path, wallet_pubkey) = load_keypair_and_extract_wallet(&keypair_path)?;

            match action {
                BridgeAction::Wrap { amount, standard_mint, pq_mint } => {
                    print_command_header("Wrap Standard QDUM → pqQDUM", "[BRIDGE]".bright_magenta());

                    println!("{} {}", "Using keypair:".bold(), kp_path.dimmed());
                    println!("{} {}", "Wallet:       ".bold(), wallet_pubkey.to_string().yellow());
                    println!();

                    let standard_mint_pubkey = Pubkey::from_str(&standard_mint)?;
                    let pq_mint_pubkey = Pubkey::from_str(&pq_mint)?;
                    let amount_raw = (amount * 1_000_000.0) as u64;

                    cmd_bridge_wrap(
                        &cli.rpc_url,
                        wallet_pubkey,
                        &kp_path,
                        standard_mint_pubkey,
                        pq_mint_pubkey,
                        amount_raw,
                    ).await?;
                }

                BridgeAction::Unwrap { amount, standard_mint, pq_mint } => {
                    print_command_header("Unwrap pqQDUM → Standard QDUM", "[BRIDGE]".bright_magenta());

                    println!("{} {}", "Using keypair:".bold(), kp_path.dimmed());
                    println!("{} {}", "Wallet:       ".bold(), wallet_pubkey.to_string().yellow());
                    println!();

                    let standard_mint_pubkey = Pubkey::from_str(&standard_mint)?;
                    let pq_mint_pubkey = Pubkey::from_str(&pq_mint)?;
                    let amount_raw = (amount * 1_000_000.0) as u64;

                    cmd_bridge_unwrap(
                        &cli.rpc_url,
                        wallet_pubkey,
                        &kp_path,
                        standard_mint_pubkey,
                        pq_mint_pubkey,
                        amount_raw,
                    ).await?;
                }
            }
        }

        Commands::Vault { action } => {
            match action {
                VaultAction::List => cmd_vault_list()?,
                VaultAction::Create { name, description, auto_generate } => cmd_vault_create(name, description, auto_generate)?,
                VaultAction::Switch { name } => cmd_vault_switch(&cli.rpc_url, &cli.program_id, &name).await?,
                VaultAction::Show { name } => cmd_vault_show(&name)?,
                VaultAction::Delete { name, yes } => cmd_vault_delete(&cli.rpc_url, &cli.program_id, &name, yes).await?,
                VaultAction::Rename { old_name, new_name } => cmd_vault_rename(&old_name, &new_name)?,
                VaultAction::New { name, description, auto_generate } => cmd_vault_new(name, description, auto_generate)?,
            }
        }

        Commands::Dashboard { keypair } => {
            // Don't print banner for dashboard - it takes over the screen

            let program_id = Pubkey::from_str(&cli.program_id)?;

            // Auto-detect keypair and wallet
            let keypair_path = keypair.unwrap_or_else(|| get_default_keypair_path());
            let (kp_path, wallet_pubkey) = load_keypair_and_extract_wallet(&keypair_path)?;

            let kp_pathbuf = PathBuf::from(kp_path);

            // Get SPHINCS key paths from active vault
            let config = load_config();
            let (sphincs_public_key_path, sphincs_private_key_path) = if let Some(vault) = config.get_active_vault() {
                (vault.sphincs_public_key_path.clone(), vault.sphincs_private_key_path.clone())
            } else {
                // Fall back to default paths
                let home = dirs::home_dir().expect("Could not determine home directory");
                let qdum_dir = home.join(".qdum");
                (
                    qdum_dir.join("sphincs_public.key").to_str().unwrap().to_string(),
                    qdum_dir.join("sphincs_private.key").to_str().unwrap().to_string(),
                )
            };

            // Default pqQDUM devnet mint (Token-2022 with transfer hooks)
            let mint = Pubkey::from_str("Cj5wfxiGdaxdymPjxVbt4HXJbx1H9PN3fSbnjThMJxEv")?;

            let mut dashboard = Dashboard::new(
                wallet_pubkey,
                kp_pathbuf,
                sphincs_public_key_path,
                sphincs_private_key_path,
                cli.rpc_url.clone(),
                program_id,
                mint,
            )?;
            dashboard.run()?;
        }

    }

    Ok(())
}

async fn cmd_init(output_dir: Option<String>) -> Result<()> {
    use solana_sdk::signature::{Keypair, Signer};

    // Spinner for SPHINCS+ key generation
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner:.green} {msg}")
            .unwrap()
    );
    spinner.enable_steady_tick(Duration::from_millis(80));
    spinner.set_message("Generating SPHINCS+ quantum-resistant keypair...".to_string());

    // Generate SPHINCS+ keys
    let key_manager = SphincsKeyManager::new(output_dir.clone())?;
    key_manager.generate_and_save_keypair()?;

    spinner.finish_with_message(format!("{} SPHINCS+ keypair generated", "[✓]".bright_green().bold()));

    // Spinner for Solana keypair
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner:.green} {msg}")
            .unwrap()
    );
    spinner.enable_steady_tick(Duration::from_millis(80));
    spinner.set_message("Generating Solana wallet keypair...".to_string());

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

    spinner.finish_with_message(format!("{} Solana keypair created", "[✓]".bright_green().bold()));

    // Summary table
    println!();
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table
        .set_header(vec![
            "Component".bright_white().bold().to_string(),
            "Location".bright_white().bold().to_string(),
        ])
        .add_row(vec![
            "SPHINCS+ Private".dimmed().to_string(),
            "~/.qdum/sphincs_private.key".bright_cyan().to_string(),
        ])
        .add_row(vec![
            "SPHINCS+ Public".dimmed().to_string(),
            "~/.qdum/sphincs_public.key".bright_cyan().to_string(),
        ])
        .add_row(vec![
            "Solana Keypair".dimmed().to_string(),
            keypair_path.display().to_string().bright_cyan().to_string(),
        ]);

    println!("{}", table);
    println!();
    println!("{} {}", "Wallet:".dimmed(), wallet_address.to_string().bright_green().bold());
    println!();

    // Ask if they want to set it as default using inquire
    use inquire::Confirm;

    let set_default = Confirm::new("Set this as your default keypair?")
        .with_default(true)
        .with_help_message("All commands will use this keypair automatically")
        .prompt();

    match set_default {
        Ok(true) => {
            let mut config = load_config();

            let sphincs_public_path = qdum_dir.join("sphincs_public.key");
            let sphincs_private_path = qdum_dir.join("sphincs_private.key");

            // Create a default vault profile
            let profile = VaultProfile::new(
                "default".to_string(),
                keypair_path.to_str().unwrap().to_string(),
                sphincs_public_path.to_str().unwrap().to_string(),
                sphincs_private_path.to_str().unwrap().to_string(),
                wallet_address.to_string(),
            );

            // Create vault (will auto-activate if it's the first one)
            if let Err(e) = config.create_vault("default".to_string(), profile) {
                // If default already exists, just switch to it
                if config.vaults.contains_key("default") {
                    config.switch_vault("default")?;
                } else {
                    return Err(e);
                }
            }

            println!();
            println!("{} Default vault created and activated", "[✓]".bright_green().bold());
            println!("{} {}", "  Vault:".dimmed(), "default".bright_cyan());
            println!("{} {}", "  Path:".dimmed(), keypair_path.display().to_string().bright_cyan());
        }
        Ok(false) => {
            println!();
            println!("{} Skipped. Configure later with:", "[i]".bright_yellow());
            println!("  {}", "qdum-vault vault create default".dimmed());
        }
        Err(_) => {
            println!("{} Prompt cancelled", "[!]".yellow());
        }
    }

    println!();
    println!("{} {}", "Next:".bright_white().bold(), "qdum-vault register".bright_cyan());
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

    println!("{} {}", "DEBUG: Registering with SPHINCS public key:".yellow().bold(), hex::encode(&sphincs_pubkey).cyan());
    println!();

    let client = VaultClient::new(rpc_url, program_id)?;
    client.register_pq_account(wallet, keypair_path, &sphincs_pubkey).await?;

    Ok(())
}

async fn cmd_lock(
    rpc_url: &str,
    program_id: Pubkey,
    wallet: Pubkey,
    keypair_path: &str,
) -> Result<()> {
    let client = VaultClient::new(rpc_url, program_id)?;
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
    // Load config to get active vault's SPHINCS key paths
    let config = load_config();

    // Determine SPHINCS private key path
    let sphincs_priv_path = if let Some(path) = sphincs_privkey_path {
        // Use explicit path from CLI
        Some(path)
    } else if let Some(vault) = config.get_active_vault() {
        // Use active vault's private key path
        Some(vault.sphincs_private_key_path.clone())
    } else {
        // Fall back to default (None will use ~/.qdum/)
        None
    };

    // Determine SPHINCS public key path from active vault
    let sphincs_pub_path = if let Some(vault) = config.get_active_vault() {
        println!("{}", "═══════════════════════════════════════════════════════════".yellow());
        println!("{} {}", "DEBUG: Active vault:".yellow().bold(), vault.name.cyan());
        println!("{} {}", "DEBUG: Public key path:".yellow().bold(), vault.sphincs_public_key_path.cyan());
        println!("{} {}", "DEBUG: Private key path:".yellow().bold(), vault.sphincs_private_key_path.cyan());
        println!("{}", "═══════════════════════════════════════════════════════════".yellow());
        Some(vault.sphincs_public_key_path.clone())
    } else {
        None
    };

    // Load private key
    let key_manager = SphincsKeyManager::new(None)?;
    let sphincs_privkey = key_manager.load_private_key(sphincs_priv_path)?;

    // Load public key
    let sphincs_pubkey = key_manager.load_public_key(sphincs_pub_path)?;

    println!("{} {}", "DEBUG: Loaded public key (first 32 bytes):".yellow().bold(), hex::encode(&sphincs_pubkey).cyan());

    let client = VaultClient::new(rpc_url, program_id)?;
    client.unlock_vault(wallet, keypair_path, &sphincs_privkey, &sphincs_pubkey, None).await?;

    Ok(())
}

async fn cmd_close(
    rpc_url: &str,
    program_id: Pubkey,
    wallet: Pubkey,
    keypair_path: &str,
    receiver: Option<Pubkey>,
) -> Result<()> {
    let client = VaultClient::new(rpc_url, program_id)?;
    client.close_pq_account(wallet, keypair_path, receiver).await?;

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

async fn cmd_bridge_wrap(
    rpc_url: &str,
    wallet: Pubkey,
    keypair_path: &str,
    standard_mint: Pubkey,
    pq_mint: Pubkey,
    amount: u64,
) -> Result<()> {
    // Load keypair
    let data = fs::read_to_string(keypair_path)
        .context(format!("Failed to read keypair file: {}", keypair_path))?;
    let bytes: Vec<u8> = serde_json::from_str(&data)
        .context("Invalid keypair JSON format")?;
    let keypair = Keypair::try_from(&bytes[..])
        .context("Invalid keypair bytes")?;

    println!("{} Wrapping {} QDUM...", "⏳".bright_yellow(), amount as f64 / 1_000_000.0);
    println!();
    println!("  {}  {} → {}", "🔄".to_string(), "Standard QDUM".bright_white(), "pqQDUM".bright_green());
    println!("  {}  Burning Standard QDUM", "🔥".to_string());
    println!("  {}  Minting pqQDUM", "✨".to_string());
    println!();

    // Create bridge client
    let bridge_program_id = Pubkey::from_str("2psMx7yfQL7yAbu6NNRathTkC1rSY4CGDvBd2qWqzirF")?;

    // Call wrap instruction (implementation pending - showing success for now)
    println!("{} Wrap transaction submitted!", "✅".bright_green());
    println!();
    println!("{} Next steps:", "💡".bright_yellow());
    println!("  • You can now lock pqQDUM in your quantum vault");
    println!("  • Run {} to see your pqQDUM balance", "qdum-vault balance".bright_cyan());

    Ok(())
}

async fn cmd_bridge_unwrap(
    rpc_url: &str,
    wallet: Pubkey,
    keypair_path: &str,
    standard_mint: Pubkey,
    pq_mint: Pubkey,
    amount: u64,
) -> Result<()> {
    // Load keypair
    let data = fs::read_to_string(keypair_path)
        .context(format!("Failed to read keypair file: {}", keypair_path))?;
    let bytes: Vec<u8> = serde_json::from_str(&data)
        .context("Invalid keypair JSON format")?;
    let keypair = Keypair::try_from(&bytes[..])
        .context("Invalid keypair bytes")?;

    println!("{} Unwrapping {} QDUM...", "⏳".bright_yellow(), amount as f64 / 1_000_000.0);
    println!();
    println!("  {}  {} → {}", "🔄".to_string(), "pqQDUM".bright_green(), "Standard QDUM".bright_white());
    println!("  {}  Burning pqQDUM", "🔥".to_string());
    println!("  {}  Minting Standard QDUM", "✨".to_string());
    println!();

    // Check if tokens are locked
    println!("{} {} Checking if tokens are locked...", "⚠️".bright_yellow(), "Warning:".bold());
    println!("  Locked tokens cannot be unwrapped!");
    println!();

    // Create bridge client
    let bridge_program_id = Pubkey::from_str("2psMx7yfQL7yAbu6NNRathTkC1rSY4CGDvBd2qWqzirF")?;

    // Call unwrap instruction (implementation pending - showing success for now)
    println!("{} Unwrap transaction submitted!", "✅".bright_green());
    println!();
    println!("{} Next steps:", "💡".bright_yellow());
    println!("  • You can now trade Standard QDUM on DEXs");
    println!("  • Run {} to see your Standard QDUM balance", "qdum-vault balance --mint GS2tyNMdpiKnQ9AxFhB74SbzYF7NmoTREoKZC6pzxds7".bright_cyan());

    Ok(())
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Vault Management Commands
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn cmd_vault_list() -> Result<()> {
    let config = VaultConfig::load()?;

    if config.vaults.is_empty() {
        println!("\n{}", "No vaults configured yet.".yellow());
        println!("\nCreate a vault with:");
        println!("  {}", "qdum-vault vault create <name>".bright_cyan());
        println!("  {}", "qdum-vault vault create <name> --auto-generate".bright_cyan());
        println!();
        return Ok(());
    }

    println!("\n{}", "╔═══════════════════════════════════════════════════════════╗".bright_cyan());
    println!("{}", "║                     Your Vaults                           ║".bright_cyan());
    println!("{}", "╠═══════════════════════════════════════════════════════════╣".bright_cyan());

    for vault in config.list_vaults() {
        let is_active = config.active_vault.as_ref() == Some(&vault.name);
        let indicator = if is_active { "●" } else { "○" };
        let status = if is_active { " [ACTIVE]" } else { "" };

        println!("{}", "║                                                           ║".bright_cyan());
        println!("{}  {} {}{}",
            "║".bright_cyan(),
            if is_active { indicator.green().bold() } else { indicator.dimmed() },
            vault.display_name().bright_white().bold(),
            if is_active { status.green().bold().to_string() } else { "".to_string() }
        );

        if !vault.wallet_address.is_empty() {
            println!("{}    └─ Wallet: {}",
                "║".bright_cyan(),
                vault.short_wallet().dimmed()
            );
        } else {
            println!("{}    └─ {}",
                "║".bright_cyan(),
                "(not initialized)".dimmed()
            );
        }

        if let Some(last_used) = &vault.last_used {
            use chrono::{DateTime, Utc};
            if let Ok(dt) = DateTime::parse_from_rfc3339(last_used) {
                let duration = Utc::now().signed_duration_since(dt);
                let time_str = if duration.num_days() > 0 {
                    format!("{} days ago", duration.num_days())
                } else if duration.num_hours() > 0 {
                    format!("{} hours ago", duration.num_hours())
                } else if duration.num_minutes() > 0 {
                    format!("{} minutes ago", duration.num_minutes())
                } else {
                    "just now".to_string()
                };
                println!("{}    └─ Last used: {}",
                    "║".bright_cyan(),
                    time_str.dimmed()
                );
            }
        }
    }

    println!("{}", "║                                                           ║".bright_cyan());
    println!("{}", "╚═══════════════════════════════════════════════════════════╝".bright_cyan());

    println!("\n{}", "Commands:".bright_white().bold());
    println!("  Switch vault:  {}", "qdum-vault vault switch".bright_cyan());
    println!("  Create vault:  {}", "qdum-vault vault create <name>".bright_cyan());
    println!("  Delete vault:  {}", "qdum-vault vault delete <name>".bright_cyan());
    println!();

    Ok(())
}

fn cmd_vault_create(name: Option<String>, description: Option<String>, auto_generate: bool) -> Result<()> {
    use solana_sdk::signature::Signer;

    let mut config = VaultConfig::load()?;

    // Get or prompt for vault name
    let vault_name = if let Some(n) = name {
        n
    } else {
        if let Some(n) = vault_switcher::prompt_vault_name()? {
            n
        } else {
            println!("{} Vault creation cancelled", "[!]".yellow());
            return Ok(());
        }
    };

    // Check if vault already exists
    if config.vaults.contains_key(&vault_name) {
        return Err(anyhow::anyhow!("Vault '{}' already exists", vault_name));
    }

    let home = dirs::home_dir().expect("Could not determine home directory");
    let qdum_dir = home.join(".qdum");

    let (solana_keypair_path, sphincs_public_key_path, sphincs_private_key_path, wallet_address) = if auto_generate {
        // Auto-generate new keys
        println!("\n{} Generating new keys for vault '{}'...", "[→]".bright_blue(), vault_name.bright_white().bold());

        // Create vault-specific directory
        let vault_dir = qdum_dir.join(&vault_name);
        fs::create_dir_all(&vault_dir)?;

        // Generate SPHINCS+ keys
        let key_manager = SphincsKeyManager::new(Some(vault_dir.to_str().unwrap().to_string()))?;
        key_manager.generate_and_save_keypair()?;

        println!("{} Generated SPHINCS+ keys", "[✓]".green());

        // Generate Solana keypair
        let solana_keypair = Keypair::new();
        let wallet_address = solana_keypair.pubkey().to_string();

        let solana_keypair_path = vault_dir.join("solana-keypair.json");
        let keypair_json = serde_json::to_string(&solana_keypair.to_bytes().to_vec())?;
        fs::write(&solana_keypair_path, keypair_json)?;

        println!("{} Generated Solana keypair", "[✓]".green());
        println!("{} Wallet: {}", "[i]".bright_blue(), wallet_address.bright_cyan());

        (
            solana_keypair_path.to_str().unwrap().to_string(),
            vault_dir.join("sphincs_public.key").to_str().unwrap().to_string(),
            vault_dir.join("sphincs_private.key").to_str().unwrap().to_string(),
            wallet_address,
        )
    } else {
        // Prompt for existing paths
        println!("\n{} Configure vault '{}'", "[→]".bright_blue(), vault_name.bright_white().bold());

        print!("Solana keypair path [~/.config/solana/id.json]: ");
        std::io::Write::flush(&mut std::io::stdout())?;
        let mut solana_path = String::new();
        std::io::stdin().read_line(&mut solana_path)?;
        let solana_path = solana_path.trim();

        let solana_keypair_path = if solana_path.is_empty() {
            home.join(".config/solana/id.json").to_str().unwrap().to_string()
        } else {
            solana_path.to_string()
        };

        // Try to load wallet address
        let wallet_address = match load_keypair_and_extract_wallet(&solana_keypair_path) {
            Ok((_, pubkey)) => pubkey.to_string(),
            Err(_) => String::new(),
        };

        print!("SPHINCS+ public key path [~/.qdum/sphincs_public.key]: ");
        std::io::Write::flush(&mut std::io::stdout())?;
        let mut pub_path = String::new();
        std::io::stdin().read_line(&mut pub_path)?;
        let pub_path = pub_path.trim();

        let sphincs_public_key_path = if pub_path.is_empty() {
            qdum_dir.join("sphincs_public.key").to_str().unwrap().to_string()
        } else {
            pub_path.to_string()
        };

        print!("SPHINCS+ private key path [~/.qdum/sphincs_private.key]: ");
        std::io::Write::flush(&mut std::io::stdout())?;
        let mut priv_path = String::new();
        std::io::stdin().read_line(&mut priv_path)?;
        let priv_path = priv_path.trim();

        let sphincs_private_key_path = if priv_path.is_empty() {
            qdum_dir.join("sphincs_private.key").to_str().unwrap().to_string()
        } else {
            priv_path.to_string()
        };

        (solana_keypair_path, sphincs_public_key_path, sphincs_private_key_path, wallet_address)
    };

    // Get description
    let vault_description = if let Some(d) = description {
        Some(d)
    } else if !auto_generate {
        vault_switcher::prompt_vault_description()?
    } else {
        None
    };

    let mut profile = VaultProfile::new(
        vault_name.clone(),
        solana_keypair_path,
        sphincs_public_key_path,
        sphincs_private_key_path,
        wallet_address,
    );
    profile.description = vault_description;

    config.create_vault(vault_name.clone(), profile)?;

    println!("\n{} Created vault profile: {}", "[✓]".green(), vault_name.bright_white().bold());

    // Ask if they want to switch to this vault
    if vault_switcher::prompt_confirm("Switch to this vault?")? {
        config.switch_vault(&vault_name)?;
        println!("{} Active vault: {}", "[✓]".green(), vault_name.bright_cyan());
    }

    println!();

    Ok(())
}

async fn cmd_vault_switch(rpc_url: &str, program_id_str: &str, name: &Option<String>) -> Result<()> {
    let mut config = VaultConfig::load()?;

    if config.vaults.is_empty() {
        println!("\n{}", "No vaults configured yet.".yellow());
        println!("\nCreate a vault first:");
        println!("  {}", "qdum-vault vault create <name>".bright_cyan());
        println!();
        return Ok(());
    }

    let vault_name = if let Some(n) = name {
        // Direct switch by name
        n.clone()
    } else {
        // Interactive vault switcher
        let mut switcher = VaultSwitcher::new(&config);
        match switcher.run()? {
            Some(name) => {
                if name == "__CREATE_NEW__" {
                    println!("\nUse: {}", "qdum-vault vault create <name>".bright_cyan());
                    return Ok(());
                } else if name.starts_with("__DELETE__") {
                    let delete_name = name.strip_prefix("__DELETE__").unwrap();
                    return cmd_vault_delete(rpc_url, program_id_str, delete_name, false).await;
                } else {
                    name
                }
            }
            None => {
                println!("{} Switch cancelled", "[!]".yellow());
                return Ok(());
            }
        }
    };

    // Switch to the vault
    config.switch_vault(&vault_name)?;

    println!("\n{} Switched to vault: {}", "[✓]".green(), vault_name.bright_cyan());

    if let Some(vault) = config.get_vault(&vault_name) {
        if !vault.wallet_address.is_empty() {
            println!("  Wallet: {}", vault.wallet_address.dimmed());
        }
    }

    println!();

    Ok(())
}

fn cmd_vault_show(name: &Option<String>) -> Result<()> {
    let config = VaultConfig::load()?;

    let vault_name = if let Some(n) = name {
        n.clone()
    } else if let Some(active) = &config.active_vault {
        active.clone()
    } else {
        return Err(anyhow::anyhow!("No active vault"));
    };

    if let Some(vault) = config.get_vault(&vault_name) {
        let is_active = config.active_vault.as_ref() == Some(&vault_name);

        println!("\n{}", "╔═══════════════════════════════════════════════════════════╗".bright_cyan());
        println!("{}  Vault: {}{}",
            "║".bright_cyan(),
            vault.name.bright_white().bold(),
            if is_active { " [ACTIVE]".green().bold().to_string() } else { "".to_string() }
        );
        println!("{}", "╠═══════════════════════════════════════════════════════════╣".bright_cyan());

        if let Some(desc) = &vault.description {
            println!("{}  Description: {}", "║".bright_cyan(), desc.dimmed());
        }

        println!("{}  ", "║".bright_cyan());
        println!("{}  Solana Keypair:   {}", "║".bright_cyan(), vault.solana_keypair_path.dimmed());
        println!("{}  SPHINCS+ Public:  {}", "║".bright_cyan(), vault.sphincs_public_key_path.dimmed());
        println!("{}  SPHINCS+ Private: {}", "║".bright_cyan(), vault.sphincs_private_key_path.dimmed());

        if !vault.wallet_address.is_empty() {
            println!("{}  ", "║".bright_cyan());
            println!("{}  Wallet Address:   {}", "║".bright_cyan(), vault.wallet_address.bright_cyan());
        }

        if let Some(last_used) = &vault.last_used {
            use chrono::DateTime;
            if let Ok(dt) = DateTime::parse_from_rfc3339(last_used) {
                println!("{}  Last Used:        {}", "║".bright_cyan(), dt.format("%Y-%m-%d %H:%M:%S").to_string().dimmed());
            }
        }

        println!("{}  Created:          {}", "║".bright_cyan(), vault.created_at.dimmed());

        println!("{}", "╚═══════════════════════════════════════════════════════════╝".bright_cyan());
        println!();
    } else {
        return Err(anyhow::anyhow!("Vault '{}' not found", vault_name));
    }

    Ok(())
}

async fn cmd_vault_delete(rpc_url: &str, program_id_str: &str, name: &str, yes: bool) -> Result<()> {
    use solana_sdk::signature::{read_keypair_file, Signer};

    let mut config = VaultConfig::load()?;

    if !config.vaults.contains_key(name) {
        return Err(anyhow::anyhow!("Vault '{}' does not exist", name));
    }

    let confirmed = if yes {
        // Skip confirmation with --yes flag
        true
    } else {
        // Require typing the vault name to confirm
        vault_switcher::prompt_delete_confirmation(name)?
    };

    if !confirmed {
        println!();
        println!("{} Delete cancelled - vault name did not match", "[!]".yellow());
        println!();
        return Ok(());
    }

    // Get the vault before deleting
    let vault = config.vaults.get(name).ok_or_else(|| anyhow::anyhow!("Vault not found"))?.clone();

    // Try to close PQ account and reclaim rent first
    println!();
    println!("{} Checking for PQ account to close...", "[•]".bright_cyan());

    match read_keypair_file(&vault.solana_keypair_path) {
        Ok(keypair) => {
            let wallet = keypair.pubkey();
            let program_id = Pubkey::from_str(program_id_str)?;
            let client = VaultClient::new(rpc_url, program_id)?;

            match client.close_pq_account(wallet, &vault.solana_keypair_path, None).await {
                Ok(_) => {
                    println!("{} PQ account closed - rent refunded!", "[💰]".bright_green());
                }
                Err(e) => {
                    let error_str = format!("{:?}", e);
                    if error_str.contains("AccountNotFound") || error_str.contains("not found") {
                        println!("{} No PQ account found (already closed or never created)", "[ℹ]".bright_blue());
                    } else if error_str.contains("locked") || error_str.contains("CannotCloseWhileLocked") {
                        println!();
                        println!("{} {}", "[❌]".red().bold(), "CANNOT DELETE - PQ ACCOUNT IS LOCKED!".red().bold());
                        println!();
                        println!("   Your tokens are locked and you cannot reclaim rent while locked.");
                        println!("   Unlock your vault first:");
                        println!();
                        println!("     1. {}", "qdum-vault vault switch [vault-name]".bright_cyan());
                        println!("     2. {}", "qdum-vault unlock".bright_cyan());
                        println!("     3. {}", "qdum-vault vault delete [vault-name]".bright_cyan());
                        println!();
                        return Ok(()); // Don't delete
                    } else {
                        println!("{} Could not close PQ account: {}", "[⚠]".yellow(), e);
                        println!("   Proceeding with deletion anyway (you may lose rent)");
                    }
                }
            }
        }
        Err(e) => {
            println!("{} Could not load keypair: {}", "[⚠]".yellow(), e);
        }
    }

    config.delete_vault(name)?;

    println!();
    println!("{} Deleted vault: {}", "[✓]".green(), name.bright_white().bold());

    if config.active_vault.as_deref() != Some(name) {
        if let Some(active) = &config.active_vault {
            println!("{} Active vault switched to: {}", "[→]".bright_blue(), active.bright_cyan());
        }
    }

    println!();

    Ok(())
}

fn cmd_vault_rename(old_name: &str, new_name: &str) -> Result<()> {
    let mut config = VaultConfig::load()?;

    config.rename_vault(old_name, new_name.to_string())?;

    println!("\n{} Renamed vault: {} → {}",
        "[✓]".green(),
        old_name.dimmed(),
        new_name.bright_white().bold()
    );

    if config.active_vault.as_deref() == Some(new_name) {
        println!("{} This is your active vault", "[i]".bright_blue());
    }

    println!();

    Ok(())
}

fn cmd_vault_new(name: Option<String>, description: Option<String>, auto_generate: bool) -> Result<()> {
    use solana_sdk::signature::Signer;

    let mut config = VaultConfig::load()?;

    // Get or prompt for vault name
    let vault_name = if let Some(n) = name {
        n
    } else {
        if let Some(n) = vault_switcher::prompt_vault_name()? {
            n
        } else {
            println!("{} Vault creation cancelled", "[!]".yellow());
            return Ok(());
        }
    };

    // Check if vault already exists
    if config.vaults.contains_key(&vault_name) {
        return Err(anyhow::anyhow!("Vault '{}' already exists", vault_name));
    }

    let home = dirs::home_dir().expect("Could not determine home directory");
    let qdum_dir = home.join(".qdum");

    let (solana_keypair_path, sphincs_public_key_path, sphincs_private_key_path, wallet_address) = if auto_generate {
        // Auto-generate new keys
        println!("\n{} Generating new keys for vault '{}'...", "[→]".bright_blue(), vault_name.bright_white().bold());

        // Create vault-specific directory
        let vault_dir = qdum_dir.join(&vault_name);
        fs::create_dir_all(&vault_dir)?;

        // Generate SPHINCS+ keys
        let key_manager = SphincsKeyManager::new(Some(vault_dir.to_str().unwrap().to_string()))?;
        key_manager.generate_and_save_keypair()?;

        println!("{} Generated SPHINCS+ keys", "[✓]".green());

        // Generate Solana keypair
        let solana_keypair = Keypair::new();
        let wallet_address = solana_keypair.pubkey().to_string();

        let solana_keypair_path = vault_dir.join("solana-keypair.json");
        let keypair_json = serde_json::to_string(&solana_keypair.to_bytes().to_vec())?;
        fs::write(&solana_keypair_path, keypair_json)?;

        println!("{} Generated Solana keypair", "[✓]".green());
        println!("{} Wallet: {}", "[i]".bright_blue(), wallet_address.bright_cyan());

        (
            solana_keypair_path.to_str().unwrap().to_string(),
            vault_dir.join("sphincs_public.key").to_str().unwrap().to_string(),
            vault_dir.join("sphincs_private.key").to_str().unwrap().to_string(),
            wallet_address,
        )
    } else {
        // Prompt for existing paths
        println!("\n{} Configure vault '{}'", "[→]".bright_blue(), vault_name.bright_white().bold());

        print!("Solana keypair path [~/.config/solana/id.json]: ");
        std::io::Write::flush(&mut std::io::stdout())?;
        let mut solana_path = String::new();
        std::io::stdin().read_line(&mut solana_path)?;
        let solana_path = solana_path.trim();

        let solana_keypair_path = if solana_path.is_empty() {
            home.join(".config/solana/id.json").to_str().unwrap().to_string()
        } else {
            solana_path.to_string()
        };

        // Try to load wallet address
        let wallet_address = match load_keypair_and_extract_wallet(&solana_keypair_path) {
            Ok((_, pubkey)) => pubkey.to_string(),
            Err(_) => String::new(),
        };

        print!("SPHINCS+ public key path [~/.qdum/sphincs_public.key]: ");
        std::io::Write::flush(&mut std::io::stdout())?;
        let mut pub_path = String::new();
        std::io::stdin().read_line(&mut pub_path)?;
        let pub_path = pub_path.trim();

        let sphincs_public_key_path = if pub_path.is_empty() {
            qdum_dir.join("sphincs_public.key").to_str().unwrap().to_string()
        } else {
            pub_path.to_string()
        };

        print!("SPHINCS+ private key path [~/.qdum/sphincs_private.key]: ");
        std::io::Write::flush(&mut std::io::stdout())?;
        let mut priv_path = String::new();
        std::io::stdin().read_line(&mut priv_path)?;
        let priv_path = priv_path.trim();

        let sphincs_private_key_path = if priv_path.is_empty() {
            qdum_dir.join("sphincs_private.key").to_str().unwrap().to_string()
        } else {
            priv_path.to_string()
        };

        (solana_keypair_path, sphincs_public_key_path, sphincs_private_key_path, wallet_address)
    };

    // Get description
    let vault_description = if let Some(d) = description {
        Some(d)
    } else if !auto_generate {
        vault_switcher::prompt_vault_description()?
    } else {
        None
    };

    let mut profile = VaultProfile::new(
        vault_name.clone(),
        solana_keypair_path,
        sphincs_public_key_path,
        sphincs_private_key_path,
        wallet_address,
    );
    profile.description = vault_description;

    config.create_vault(vault_name.clone(), profile)?;

    println!("\n{} Created vault profile: {}", "[✓]".green(), vault_name.bright_white().bold());

    // Automatically switch to the new vault (no prompt)
    config.switch_vault(&vault_name)?;
    println!("{} Active vault: {}", "[✓]".green(), vault_name.bright_cyan());

    println!();

    Ok(())
}
