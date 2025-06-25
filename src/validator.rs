use crate::parser::Assignment;
use crate::parser::Path;
use crate::parser::Segment;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::hash_map::Entry;
use std::rc::Rc;

pub fn validate(assignments: &[Assignment]) -> Result<(), String> {
    check_key_consistency(assignments)?;
    check_path_uniqueness(assignments)?;
    check_node_types(assignments)
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

fn check_node_types(assignments: &[Assignment]) -> Result<(), String> {
    #[derive(Eq, PartialEq)]
    enum Type {
        Object,
        Array,
        Value,
    }

    impl std::fmt::Display for Type {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            match self {
                Type::Object => write!(f, "object"),
                Type::Array => write!(f, "array"),
                Type::Value => write!(f, "value"),
            }
        }
    }

    let mut types: HashMap<Rc<Path>, Type> = HashMap::new();

    for assignment in assignments {
        let mut path = assignment.path.clone();

        match types.entry(path.clone()) {
            Entry::Vacant(vacant) => vacant.insert(Type::Value),
            Entry::Occupied(occupied) => Err(format!(
                "path {} referred to as both {} and value",
                path,
                occupied.get()
            ))?,
        };

        while let Some((prefix, segment)) = path.split_last() {
            let typ = match &*segment {
                Segment::Key(_) => Type::Object,
                Segment::Index(_) => Type::Array,
            };
            match types.entry(prefix.clone()) {
                Entry::Vacant(vacant) => {
                    vacant.insert(typ);
                }
                Entry::Occupied(occupied) if *occupied.get() == typ => {}
                Entry::Occupied(occupied) => Err(format!(
                    "path {} referred to as both {} and {}",
                    prefix,
                    occupied.get(),
                    typ
                ))?,
            };

            path = prefix;
        }
    }

    Ok(())
}
