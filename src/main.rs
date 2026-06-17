use anyhow::Result;
use clap::Parser;
use methodfig::cli::{Cli, Command};
use methodfig::pipeline::{resume_pipeline, run_pipeline};
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Run(args) => {
            let result = run_pipeline(args.into_options())?;
            println!(
                "methodfig run complete: accepted={}, rounds={}, final={}",
                result.accepted,
                result.rounds,
                result.run_dir.join("final").display()
            );
        }
        Command::Resume(args) => {
            let result = resume_pipeline(args.run)?;
            println!(
                "methodfig resume complete: accepted={}, rounds={}, final={}",
                result.accepted,
                result.rounds,
                result.run_dir.join("final").display()
            );
        }
        Command::Doctor => {
            let report = methodfig::tools::doctor::run_doctor()?;
            println!("{}", report.to_human_string());
            if report.has_errors() {
                std::process::exit(1);
            }
        }
        Command::Schema(args) => {
            if args.print {
                println!("{}", methodfig::schema::figure_plan_schema_json()?);
            }
        }
    }

    Ok(())
}
