use aeg::AbstractEventGraph;
use clap::{Parser, ValueEnum};
use interpreter::MemoryModel;
use std::fs;
use std::path::PathBuf;

#[derive(ValueEnum, Default, Debug, Clone)]
#[clap(rename_all = "kebab-case")]
enum ArgMemoryModel {
    // Sequential consistency
    #[default]
    Sc,
    // Total store order
    Tso,
}

impl From<ArgMemoryModel> for MemoryModel {
    fn from(arg: ArgMemoryModel) -> Self {
        match arg {
            ArgMemoryModel::Sc => MemoryModel::Sc,
            ArgMemoryModel::Tso => MemoryModel::Tso,
        }
    }
}

#[derive(Parser, Debug, Clone)]
struct Args {
    // Input toy program
    #[arg(short, long)]
    input: PathBuf,

    // Memory model
    #[arg(short, long, default_value_t, value_enum)]
    memory_model: ArgMemoryModel,

    // Parse only
    #[arg(short, long, action)]
    parse_only: bool,

    // Return AEG + Critical cycles
    #[arg(short, long, action)]
    aeg_only: bool,
}

fn main() {
    let args = Args::parse();
    let source = fs::read_to_string(&args.input).expect("Failed to read input file!");
    let program = parser::parse(&source).unwrap();
    if args.parse_only {
        println!("Parsed without errors");
        return;
    }
    if args.aeg_only {
        let aeg = AbstractEventGraph::from(&program);
        let ccs = match args.memory_model {
            ArgMemoryModel::Sc => unimplemented!(),
            ArgMemoryModel::Tso => aeg.tso_critical_cycles(),
        };
        println!(
            "{{\"aeg\":{},\"critical_cycles\":{}}}",
            serde_json::to_string(&aeg.graph).unwrap(),
            serde_json::to_string(&ccs).unwrap()
        );
        return;
    }
    interpreter::execute(&program, MemoryModel::from(args.memory_model));
}
