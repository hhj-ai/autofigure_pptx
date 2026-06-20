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
            print_result("methodfig run complete", &result);
        }
        Command::Resume(args) => {
            let result = resume_pipeline(args.run)?;
            print_result("methodfig resume complete", &result);
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

fn print_result(label: &str, result: &methodfig::pipeline::PipelineResult) {
    let run_dir = std::fs::canonicalize(&result.run_dir).unwrap_or_else(|_| result.run_dir.clone());
    let final_dir =
        std::fs::canonicalize(&result.final_dir).unwrap_or_else(|_| result.final_dir.clone());
    let pptx = final_dir.join("figure.pptx");
    let png = final_dir.join("figure.png");
    let status = final_dir.join("status.json");

    println!(
        "{}: accepted={}, rounds={}, reason={}",
        label, result.accepted, result.rounds, result.reason
    );
    println!("run_dir: {}", run_dir.display());
    println!("final_dir: {}", final_dir.display());
    println!("pptx: {}", pptx.display());
    println!("png: {}", png.display());
    println!("status: {}", status.display());
    println!("open_pptx: open \"{}\"", pptx.display());
}
