use clap::Parser;
use mkjson::node::Node;
use mkjson::parser::ParseError;
use mkjson::parser::is_xid_string;
use mkjson::parser::validate_json;
use mkjson::transform;
use std::process::ExitCode;
use std::rc::Rc;

/// Simple CLI tool
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// JSON-RPC identifier
    #[arg(short, long, value_parser = validate_id)]
    id: Option<String>,

    /// JSON-RPC method
    #[arg(short, long, value_parser = validate_method)]
    method: String,

    /// JSON-RPC params assignments
    #[arg()]
    args: Vec<String>,
}

fn main() -> ExitCode {
    let args = Args::parse();

    match transform(args.args.into_iter()) {
        Ok(tree) => {
            let mut attributes = vec![
                (
                    Rc::new("\"jsonrpc\"".to_string()),
                    Node::Value("\"2.0\"".to_string()),
                ),
                (Rc::new("\"method\"".to_string()), Node::Value(args.method)),
            ];
            if let Some(id) = args.id {
                attributes.push((Rc::new("\"id\"".to_string()), Node::Value(id)));
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

fn validate_method(input: &str) -> Result<String, ParseError> {
    if is_xid_string(input) {
        Ok(format!("\"{}\"", input))
    } else if input.starts_with('"') {
        validate_json(1, input)?;
        Ok(input.to_string())
    } else {
        Err(ParseError::new(1, "must be a string".to_string()))
    }
}

fn validate_id(input: &str) -> Result<String, ParseError> {
    if is_xid_string(input) {
        Ok(format!("\"{}\"", input))
    } else if input == ":null" {
        Ok("null".to_string())
    } else if !input.starts_with('"') || input.starts_with(|c: char| c.is_ascii_digit()) {
        validate_json(1, input)?;
        Ok(input.to_string())
    } else {
        Err(ParseError::new(
            1,
            "must be a string, number or null".to_string(),
        ))
    }
}
