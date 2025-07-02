use clap::Parser;
use mkjson::composer::compose;
use mkjson::node::Node;
use mkjson::parser::is_xid_string;
use mkjson::parser::validate_json;
use std::process::ExitCode;
use std::rc::Rc;

/// Command-Line JSON-RPC Composer
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// "id" value
    #[arg(short, long, default_value = ":omit", value_parser = validate_id)]
    id: String,

    /// "method" value
    #[arg(short, long, value_parser = validate_method)]
    method: String,

    /// "params" directives (e.g., a.b:true c.0.d=foobar)
    #[arg(id = "DIRECTIVE")]
    directives: Vec<Vec<u8>>,
}

fn main() -> ExitCode {
    let args = Args::parse();

    match compose(args.directives.into_iter()) {
        Ok(tree) => {
            let mut attributes = vec![
                (
                    Rc::new("\"jsonrpc\"".to_string()),
                    Node::Value("\"2.0\"".to_string()),
                ),
                (Rc::new("\"method\"".to_string()), Node::Value(args.method)),
            ];
            if args.id != ":omit" {
                attributes.push((Rc::new("\"id\"".to_string()), Node::Value(args.id)));
            }
            if let Some(node) = tree {
                attributes.push((Rc::new("\"params\"".to_string()), node));
            }
            let request = Node::Object(attributes.into_iter().collect());

            println!("{}", request);

            ExitCode::from(0)
        }
        Err(message) => {
            eprintln!("input error: {}", message);
            ExitCode::from(2)
        }
    }
}

fn validate_method(input: &str) -> Result<String, String> {
    if is_xid_string(input) {
        Ok(format!("\"{}\"", input))
    } else if input.starts_with('"') {
        validate_json(1, input).map_err(|e| e.to_string())?;
        Ok(input.to_string())
    } else {
        Err("must be a string".to_string())
    }
}

fn validate_id(input: &str) -> Result<String, String> {
    if is_xid_string(input) {
        Ok(format!("\"{}\"", input))
    } else if input == ":null" {
        Ok("null".to_string())
    } else if input == ":omit" {
        Ok(":omit".to_string())
    } else if input.starts_with('"') || input.starts_with(|c: char| c.is_ascii_digit()) {
        validate_json(1, input).map_err(|e| e.to_string())?;
        Ok(input.to_string())
    } else {
        Err("must be a string, number, ':null' or ':omit'".to_string())
    }
}
