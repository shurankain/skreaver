use clap::Parser;

mod agents;
use agents::{run_echo_agent, run_multi_agent, run_reasoning_agent};

#[derive(Parser, Debug)]
#[command(name = "skreaver", version)]
struct Cli {
    #[arg(long)]
    name: String,
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

    match cli.name.as_str() {
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
        _ => {
            tracing::error!(agent_name = %cli.name, "Unknown agent requested");
            std::process::exit(1);
        }
    }
}
