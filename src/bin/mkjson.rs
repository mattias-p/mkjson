use mkjson::transform;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut assignments = std::env::args();
    let _command = assignments.next();

    match transform(assignments) {
        Ok(tree) => {
            if let Some(node) = tree {
                println!("{}\n", node);
            }
            ExitCode::from(0)
        }
        Err(message) => {
            eprintln!("input error: {}", message);
            ExitCode::from(2)
        }
    }
}
