pub mod assignment;
pub mod parser;
pub mod validator;

use crate::assignment::Assignment;
use crate::parser::ParseError;
use crate::parser::parse_assignment;
use crate::validator::validate;

pub fn parse(input: &str) -> Result<Assignment, ParseError> {
    Ok(parse_assignment(input)?.0.into())
}

pub fn transform<'a>(assignments: impl Iterator<Item = String>) -> Result<String, String> {
    let mut results = vec![];
    for assignment in assignments {
        let assignment = parse(&assignment)
            .map_err(|e| format!("assignment \"{}\": {}", assignment.escape_default(), e))?;
        results.push(assignment);
    }

    validate(results.as_slice()).map_err(|e| format!("{}", e))?;

    let results: Vec<_> = results
        .into_iter()
        .map(|assignment| format!("{:?}", assignment))
        .collect();
    Ok(results.join("\n"))
}
