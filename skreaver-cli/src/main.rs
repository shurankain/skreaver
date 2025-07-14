use clap::Parser;
mod agents;
use agents::echo::run_echo_agent;

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
        _ => eprintln!("Unknown agent: {}", cli.name),
    }
}
