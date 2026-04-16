use clap::{Parser, Subcommand};

/// ocx - a secure Docker wrapper for OpenCode
#[derive(Parser)]
#[command(name = "ocx")]
#[command(about, long_about = None, version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Config {
        #[command(subcommand)]
        command: Option<ConfigCommands>,
    },
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Show the current configuration
    Show,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Config { command }) => {
            match command {
                Some(ConfigCommands::Show) | None => {
                    // Stub: just print a placeholder for now
                    println!("Configuration will be displayed here");
                    Ok(())
                }
            }
        }
        None => {
            // No subcommand provided, print help
            Cli::parse_from(["ocx", "--help"]);
            Ok(())
        }
    }
}
