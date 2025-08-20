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
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("info".parse().unwrap()),
        )
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
        _ => eprintln!("Unknown agent: {}", cli.name),
    }
}
