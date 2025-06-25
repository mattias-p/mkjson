pub mod parser;

use crate::parser::Assignment;
use crate::parser::Path;
use crate::parser::Segment;
use crate::parser::parse;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::hash_map::Entry;
use std::rc::Rc;

pub fn transform<'a>(assignments: impl Iterator<Item = String>) -> Result<String, String> {
    let mut results = vec![];
    for assignment in assignments {
        let assignment = parse(&assignment)
            .map_err(|e| format!("assignment \"{}\": {}", assignment.escape_default(), e))?;
        results.push(assignment);
    }

    check(results.as_slice()).map_err(|e| format!("{}", e))?;

    let results: Vec<_> = results
        .into_iter()
        .map(|assignment| format!("{:?}", assignment))
        .collect();
    Ok(results.join("\n"))
}

pub fn check(assignments: &[Assignment]) -> Result<(), String> {
    check_key_consistency(assignments)?;
    check_path_uniqueness(assignments)
}

fn check_key_consistency(assignments: &[Assignment]) -> Result<(), String> {
    let mut keys: HashMap<Rc<Path>, Rc<Segment>> = HashMap::new();
    for assignment in assignments {
        let mut given_path = assignment.path.clone();
        let mut normalized_path = given_path.normalize();

        while let Some((given_prefix, given_segment)) = given_path.split_last() {
            if let Segment::Key(_) = &*given_segment {
                match keys.entry(normalized_path.clone()) {
                    Entry::Vacant(vacant) => {
                        vacant.insert(given_segment.clone());
                    }
                    Entry::Occupied(occupied) => {
                        if given_segment != *occupied.get() {
                            Err(format!(
                                "path {} has equivalent but inconsistently escaped keys {} and {}",
                                given_prefix,
                                occupied.get(),
                                given_segment
                            ))?;
                        }
                    }
                }
            }
            given_path = given_prefix;
            normalized_path = normalized_path
                .prefix()
                .expect("normalized_path should track given_path");
        }
    }
    Ok(())
}

fn check_path_uniqueness(assignments: &[Assignment]) -> Result<(), String> {
    let mut paths = HashSet::new();

    for assignment in assignments {
        if !paths.insert(assignment.path.clone()) {
            Err(format!("multiple assignments to path {}", assignment.path))?;
        }
    }
    Ok(())
}
