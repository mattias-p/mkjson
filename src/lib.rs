pub mod parser;

use crate::parser::parse;

pub fn transform<'a>(assignments: impl Iterator<Item = String>) -> Result<String, String> {
    let mut results = vec![];
    for assignment in assignments {
        let assignment = parse(&assignment).map_err(|e| {
            format!(
                "assignment \"{}\": {}",
                assignment.escape_default(),
                e.inner
            )
        })?;
        results.push(format!("{:?}", assignment));
    }
    Ok(results.join("\n"))
}
