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
