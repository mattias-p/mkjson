use clap::Parser;
use mkjson::compiler::compile;
use std::process::ExitCode;

/// Construct JSON from paths on the shell
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// Builder expressions (e.g., a.b:true c.0.d=foobar)
    #[arg(id = "ASSIGNMENT")]
    assignments: Vec<String>,
}

fn main() -> ExitCode {
    let args = Args::parse();

    match compile(args.assignments.into_iter()) {
        Ok(tree) => {
            if let Some(node) = tree {
                println!("{}", node);
            }
            ExitCode::from(0)
        }
        Err(message) => {
            eprintln!("input error: {}", message);
            ExitCode::from(2)
        }
    }
}
