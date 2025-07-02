use clap::Parser;
use mkjson::composer::compose;
use std::process::ExitCode;

/// Command-Line JSON Composer
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// Directives (e.g., a.b:true c.0.d=foobar)
    #[arg(id = "DIRECTIVE")]
    directives: Vec<Vec<u8>>,
}

fn main() -> ExitCode {
    let args = Args::parse();

    match compose(args.directives.into_iter()) {
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
