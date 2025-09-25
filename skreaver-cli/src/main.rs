use clap::{Parser, Subcommand};

mod agents;
mod perf;

use agents::{run_echo_agent, run_multi_agent, run_reasoning_agent, run_standard_tools_agent};
use perf::run_perf_command;

#[derive(Parser, Debug)]
#[command(name = "skreaver", version = "0.3.0")]
#[command(about = "Skreaver CLI - Agent infrastructure and performance tools")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run agent examples
    Agent {
        #[arg(long)]
        name: String,
    },
    /// Performance regression detection tools
    Perf {
        #[command(subcommand)]
        perf_command: PerfCommands,
    },
}

#[derive(Subcommand, Debug)]
enum PerfCommands {
    /// Run full analysis workflow (benchmark -> baseline -> detect)
    Run {
        /// Specific benchmark to run (optional)
        benchmark: Option<String>,
    },
    /// Create new performance baselines
    CreateBaseline {
        /// Specific benchmark to baseline (optional)
        benchmark: Option<String>,
    },
    /// Update existing baselines
    UpdateBaseline {
        /// Specific benchmark to update (optional)
        benchmark: Option<String>,
    },
    /// Check for regressions against existing baselines
    Check {
        /// Specific benchmark to check (optional)
        benchmark: Option<String>,
    },
    /// List all available baselines
    List,
    /// Show details of a specific baseline
    Show {
        /// Baseline name to show
        name: String,
    },
    /// Remove a baseline
    Remove {
        /// Baseline name to remove
        name: String,
    },
    /// Export baseline to file
    Export {
        /// Baseline name to export
        name: String,
        /// Output file path
        path: String,
    },
    /// Import baseline from file
    Import {
        /// Input file path
        path: String,
    },
    /// CI-friendly check (exits with error if regressions found)
    Ci {
        /// Specific benchmark to check (optional)
        benchmark: Option<String>,
    },
}

fn main() {
    // Initialize JSON logging once.
    let env_filter = tracing_subscriber::EnvFilter::from_default_env();
    let env_filter = match "info".parse() {
        Ok(directive) => env_filter.add_directive(directive),
        Err(_) => env_filter, // fallback to default if parsing fails
    };

    let _ = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .json()
        .try_init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Agent { name } => match name.as_str() {
            "echo" => {
                println!("Running echo agent...");
                run_echo_agent();
            }
            "multi" => {
                println!("Running multi-tool agent...");
                run_multi_agent();
            }
            "reasoning" => {
                println!("Running reasoning agent...");
                run_reasoning_agent();
            }
            "tools" => {
                println!("Running standard tools agent...");
                run_standard_tools_agent();
            }
            _ => {
                tracing::error!(agent_name = %name, "Unknown agent requested");
                std::process::exit(1);
            }
        },
        Commands::Perf { perf_command } => {
            if let Err(e) = run_perf_command(perf_command) {
                tracing::error!(error = %e, "Performance command failed");
                std::process::exit(1);
            }
        }
    }
}
