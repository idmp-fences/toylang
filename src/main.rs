use aeg::{AbstractEventGraph, AegConfig, Architecture};
use clap::{Parser, Subcommand, ValueEnum};
use interpreter::MemoryModel;
use std::fs;
use std::path::PathBuf;

#[derive(ValueEnum, Default, Debug, Clone)]
#[clap(rename_all = "kebab-case")]
enum ArgMemoryModel {
    /// Sequential consistency
    Sc,
    /// Total store order
    #[default]
    Tso,
}

impl From<&ArgMemoryModel> for MemoryModel {
    fn from(arg: &ArgMemoryModel) -> Self {
        match arg {
            ArgMemoryModel::Sc => MemoryModel::Sc,
            ArgMemoryModel::Tso => MemoryModel::Tso,
        }
    }
}

#[derive(Parser)]
// #[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Interpret a toy program
    Run {
        /// Toy program to run
        file: PathBuf,

        /// Memory model to use
        #[arg(short, long, default_value_t, value_enum)]
        memory_model: ArgMemoryModel,
    },
    /// Find the critical cycles of a toy program and return the AEG + Critical cycles
    FindCycles {
        /// Toy program to find cycles in
        file: PathBuf,

        /// Memory model to use, in order to calculate the appropriate delays
        #[arg(short, long, default_value_t, value_enum)]
        memory_model: ArgMemoryModel,

        /// By default we skip over ifs and whiles when considering fence placement.
        /// Turning this on may result in much more critical cycles especially for
        /// programs with a high branch factor. However, the resulting potential fence
        /// placements are more correct as they contain every possible code execution path.
        #[arg(short, long, default_value = "false")]
        all_branches: bool,
    },
}

fn main() {
    let args = Args::parse();

    match &args.command {
        Commands::Run { file, memory_model } => {
            let source = fs::read_to_string(&file).expect("Failed to read input file!");
            let program = parser::parse(&source).unwrap();
            interpreter::execute(&program, MemoryModel::from(memory_model));
        }
        Commands::FindCycles {
            file,
            memory_model,
            all_branches,
        } => {
            let source = fs::read_to_string(&file).expect("Failed to read input file!");
            let program = parser::parse(&source).unwrap();
            let architecture = match memory_model {
                ArgMemoryModel::Sc => panic!("There are no critical cycles under SC"),
                ArgMemoryModel::Tso => Architecture::Tso,
            };
            let aeg = AbstractEventGraph::with_config(
                &program,
                AegConfig {
                    architecture,
                    skip_branches: !*all_branches,
                },
            );
            let ccs = aeg.find_critical_cycles();
            println!(
                "{{\"aeg\":{},\"critical_cycles\":{}}}",
                serde_json::to_string(&aeg.graph).unwrap(),
                serde_json::to_string(&ccs).unwrap()
            );
        }
    }
}
