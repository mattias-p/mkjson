pub mod assignment;
pub mod node;
pub mod parser;
pub mod validator;

use crate::assignment::Assignment;
use crate::node::build_tree;
use crate::parser::ParseError;
use crate::parser::parse_assignment;
use crate::validator::validate;

pub fn parse(input: &str) -> Result<Assignment, ParseError> {
    Ok(parse_assignment(input)?.0.into())
}

pub fn transform<'a>(inputs: impl Iterator<Item = String>) -> Result<String, String> {
    let mut assignments = vec![];
    for text in inputs {
        let assignment =
            parse(&text).map_err(|e| format!("assignment \"{}\": {}", text.escape_default(), e))?;
        assignments.push(assignment);
    }

    validate(assignments.as_slice()).map_err(|e| format!("{}", e))?;

    let tree = build_tree(assignments.into_iter());

    if let Some(node) = tree {
        Ok(format!("{}\n", node))
    } else {
        Ok("".to_string())
    }
}
