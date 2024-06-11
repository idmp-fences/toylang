use std::fs;
use std::path::PathBuf;
use clap::{Args, Parser, Subcommand};

#[derive(Args, Debug, Clone)]
struct RandomArgs {
    // Number of threads to generate
    #[arg()]
    threads: usize,

    // Number of instructions per thread
    #[arg()]
    instructions: usize,

    // Number of global variables
    #[arg()]
    global_variables: usize,

    // Maximum depth of the instructions
    #[arg(long, default_value_t = 2)]
    max_depth: usize,
}

#[derive(Args, Debug, Clone)]
struct LamportArgs {
    // Number of threads to generate
    #[arg()]
    threads: usize,
}

#[derive(Args, Debug, Clone)]
struct PetersonArgs {
    // Number of threads to generate
    #[arg()]
    threads: usize,
}

#[derive(Subcommand, Debug, Clone)]
#[clap(rename_all = "kebab-case")]
enum Generator {
    // Random program
    Random(RandomArgs),
    // Lamport's algorithm
    Lamport(LamportArgs),
    // Peterson's algorithm
    Peterson(PetersonArgs),
}

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
struct Cli {
    // Generator for the toy program
    #[command(subcommand)]
    generator: Generator,

    // Output path
    #[arg(short, long, global = true)]
    output: Option<PathBuf>,
}

fn main() {
    let args = Cli::parse();

    let program = match args.generator {
        Generator::Random(ra) => generator::generate_random_program(ra.threads, ra.instructions, ra.global_variables, ra.max_depth),
        Generator::Lamport(la) => generator::generate_lamport_program(la.threads),
        Generator::Peterson(pa) => generator::generate_peterson_program(pa.threads),
    };

    if let Some(output) = args.output {
        fs::write(output, program.to_string()).unwrap();
    } else {
        println!("{}", program);
    }
}
