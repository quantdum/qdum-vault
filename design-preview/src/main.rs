use colored::Colorize;
use comfy_table::{Table, presets::UTF8_FULL};
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

fn print_banner() {
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

    // ASCII Art Logo - QDUM style
    println!("{}", "  ╔══════════════════════════════════════════════════════════════════╗".bright_green().bold());
    println!("{}", "  ║                                                                  ║".bright_green());
    println!("{}", "  ║     ██████╗ ██████╗ ██╗   ██╗███╗   ███╗                        ║".bright_green().bold());
    println!("{}", "  ║    ██╔═══██╗██╔══██╗██║   ██║████╗ ████║                        ║".bright_green().bold());
    println!("{}", "  ║    ██║   ██║██║  ██║██║   ██║██╔████╔██║                        ║".bright_green().bold());
    println!("{}", "  ║    ██║▄▄ ██║██║  ██║██║   ██║██║╚██╔╝██║                        ║".bright_green().bold());
    println!("{}", "  ║    ╚██████╔╝██████╔╝╚██████╔╝██║ ╚═╝ ██║                        ║".bright_green().bold());
    println!("{}", "  ║     ╚══▀▀═╝ ╚═════╝  ╚═════╝ ╚═╝     ╚═╝                        ║".bright_green().bold());
    println!("{}", "  ║                                                                  ║".bright_green());
    println!("  ║                {}                       ║", "Q U A N T U M   V A U L T".bright_white().bold());
    println!("  ║           {}           ║", "Post-Quantum Security for Solana".bright_cyan());
    println!("{}", "  ║                                                                  ║".bright_green());
    println!("{}", "  ╠══════════════════════════════════════════════════════════════════╣".bright_green().bold());
    println!("{}", "  ║                                                                  ║".bright_green());

    // Quick stats with icons
    println!("  ║  {}  {}                   ║", "🔐".to_string(), format!("{:<56}", "SPHINCS+ (NIST FIPS 205) - Quantum Resistant".bright_white()));
    println!("  ║  {}  {}                   ║", "🌐".to_string(), format!("{:<56}", "Solana Devnet - On-Chain Verification".bright_white()));
    println!("  ║  {}  {}                   ║", "📦".to_string(), format!("{:<56}", "Version 1.0.0 - Production Ready".bright_white()));

    println!("{}", "  ║                                                                  ║".bright_green());
    println!("{}", "  ╚══════════════════════════════════════════════════════════════════╝".bright_green().bold());
    println!();

    // Quick start guide
    let mut guide_table = Table::new();
    guide_table.load_preset(UTF8_FULL);
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

fn print_command_header(text: &str, icon: &str) {
    println!();
    println!("{}", "╔".bright_green().to_string() + &"═".repeat(68).bright_green().to_string() + &"╗".bright_green().to_string());
    println!("║  {} {}  ║", icon.to_string(), format!("{:<60}", text).bright_white().bold());
    println!("{}", "╚".bright_green().to_string() + &"═".repeat(68).bright_green().to_string() + &"╝".bright_green().to_string());
    println!();
}

fn demo_status_command() {
    print_command_header("Vault Status", "[STATUS]");

    let mut status_table = Table::new();
    status_table.load_preset(UTF8_FULL);
    status_table.set_header(vec![
        "Property".bright_white().bold().to_string(),
        "Value".bright_white().bold().to_string()
    ]);

    status_table
        .add_row(vec![
            "Wallet".dimmed().to_string(),
            "GmyQgsEGQ5JcS6dMkdAPiX3i4d6eYJkfBMZgjVmLkETm".bright_cyan().to_string()
        ])
        .add_row(vec![
            "PQ Account (PDA)".dimmed().to_string(),
            "BCkyYwWXmDUNPeQDfNbFF688V8S8PApt8tcq9eEKCbLe".bright_cyan().to_string()
        ])
        .add_row(vec![
            "SPHINCS+ Public Key".dimmed().to_string(),
            "8c5e2c642b5d...f3a9b1c7e4d2".bright_green().to_string()
        ])
        .add_row(vec![
            "Algorithm".dimmed().to_string(),
            "SPHINCS+-SHA2-128s (2)".bright_green().to_string()
        ])
        .add_row(vec![
            "Vault Status".dimmed().to_string(),
            "🔒 LOCKED".red().bold().to_string()
        ]);

    println!("{}", status_table);
    println!();

    println!("{}", "⚠️  Vault is Locked".yellow().bold());
    println!();
    println!("  {} Your tokens cannot be transferred while locked.", "•".bright_yellow());
    println!("  {} Run {} to unlock", "•".bright_yellow(), "qdum-vault unlock".bright_green());
    println!();
    println!("{}", "Unlock Challenge:".dimmed());
    println!("  {}", "59148de4396f1aa53cdeb93b998363b89b020449a9bab0e50dc4303a90f1cca4".bright_cyan());
    println!();
}

fn demo_init_command() {
    print_command_header("Initialize Quantum Keypair", "[INIT]");

    println!("{} SPHINCS+ keypair generated", "[✓]".bright_green().bold());
    println!("{} Solana keypair created", "[✓]".bright_green().bold());
    println!();

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        "Component".bright_white().bold().to_string(),
        "Location".bright_white().bold().to_string(),
    ]);

    table
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
            "~/.qdum/solana-keypair.json".bright_cyan().to_string(),
        ]);

    println!("{}", table);
    println!();
    println!("{} {}", "Wallet:".dimmed(), "GmyQgsEGQ5JcS6dMkdAPiX3i4d6eYJkfBMZgjVmLkETm".bright_green().bold());
    println!();
}

fn main() {
    // Test different screens
    println!("\n{}\n", "=".repeat(70).bright_magenta());
    println!("{}", "TESTING: BANNER".bright_magenta().bold());
    println!("{}\n", "=".repeat(70).bright_magenta());
    print_banner();

    println!("\n{}\n", "=".repeat(70).bright_magenta());
    println!("{}", "TESTING: INIT COMMAND".bright_magenta().bold());
    println!("{}\n", "=".repeat(70).bright_magenta());
    demo_init_command();

    println!("\n{}\n", "=".repeat(70).bright_magenta());
    println!("{}", "TESTING: STATUS COMMAND".bright_magenta().bold());
    println!("{}\n", "=".repeat(70).bright_magenta());
    demo_status_command();
}
