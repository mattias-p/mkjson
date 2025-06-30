use crate::assignment::Assignment;
use crate::assignment::Path;
use crate::assignment::Segment;
use snafu::prelude::*;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::hash_map::Entry;
use std::rc::Rc;

#[derive(Debug, Snafu)]
#[snafu(display("path {path}: {variant}"))]
pub struct PathError {
    pub path: Rc<Path>,
    pub variant: PathErrorVariant,
}

#[derive(Debug, Snafu)]
pub enum PathErrorVariant {
    #[snafu(display("path has equivalent but inconsistently escaped keys {key1} and {key2}"))]
    InconsistentKeyEscaping { key1: Segment, key2: Segment },

    #[snafu(display("colliding assignments to path"))]
    CollidingAssignments,

    #[snafu(display("path referred to as both {kind1} and {kind2}"))]
    InconsistentNodeKind { kind1: NodeKind, kind2: NodeKind },

    #[snafu(display("array at path has index {index_seen} but lacks index {index_missing}",))]
    IncompleteArray { index_seen: u32, index_missing: u32 },
}

type ValidationResult = Result<(), PathError>;

pub fn validate(assignments: &[Assignment]) -> ValidationResult {
    check_key_consistency(assignments)?;
    check_path_uniqueness(assignments)?;
    check_node_types(assignments)?;
    check_array_completeness(assignments)
}

fn check_key_consistency(assignments: &[Assignment]) -> ValidationResult {
    let mut keys: HashMap<Rc<Path>, Segment> = HashMap::new();
    for assignment in assignments {
        let mut given_path = assignment.path.clone();
        let mut normalized_path = given_path.unescape();

        while let Some((given_prefix, given_segment)) = given_path.split_last() {
            if let Segment::Key(_) = given_segment {
                match keys.entry(normalized_path.clone()) {
                    Entry::Vacant(vacant) => {
                        vacant.insert(given_segment.clone());
                    }
                    Entry::Occupied(occupied) => {
                        if given_segment != *occupied.get() {
                            Err(PathError {
                                path: given_prefix.clone(),
                                variant: PathErrorVariant::InconsistentKeyEscaping {
                                    key1: occupied.get().clone(),
                                    key2: given_segment,
                                },
                            })?;
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

fn check_path_uniqueness(assignments: &[Assignment]) -> ValidationResult {
    let mut paths = HashSet::new();

    for assignment in assignments {
        if !paths.insert(assignment.path.clone()) {
            Err(PathError {
                variant: PathErrorVariant::CollidingAssignments,
                path: assignment.path.clone(),
            })?;
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NodeKind {
    Object,
    Array,
    Value,
}

impl std::fmt::Display for NodeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            NodeKind::Object => write!(f, "object"),
            NodeKind::Array => write!(f, "array"),
            NodeKind::Value => write!(f, "value"),
        }
    }
}

fn check_node_types(assignments: &[Assignment]) -> ValidationResult {
    let mut types: HashMap<Rc<Path>, NodeKind> = HashMap::new();

    for assignment in assignments {
        let mut path = assignment.path.clone();

        match types.entry(path.clone()) {
            Entry::Vacant(vacant) => vacant.insert(NodeKind::Value),
            Entry::Occupied(occupied) => Err(PathError {
                path: path.clone(),
                variant: PathErrorVariant::InconsistentNodeKind {
                    kind1: *occupied.get(),
                    kind2: NodeKind::Value,
                },
            })?,
        };

        while let Some((prefix, segment)) = path.split_last() {
            let kind = match segment {
                Segment::Key(_) => NodeKind::Object,
                Segment::Index(_) => NodeKind::Array,
            };
            match types.entry(prefix.clone()) {
                Entry::Vacant(vacant) => {
                    vacant.insert(kind);
                }
                Entry::Occupied(occupied) if *occupied.get() == kind => {}
                Entry::Occupied(occupied) => Err(PathError {
                    path: prefix.clone(),
                    variant: PathErrorVariant::InconsistentNodeKind {
                        kind1: *occupied.get(),
                        kind2: kind,
                    },
                })?,
            };

            path = prefix;
        }
    }

    Ok(())
}

fn check_array_completeness(assignments: &[Assignment]) -> ValidationResult {
    let mut arrays: HashMap<Rc<Path>, BTreeSet<u32>> = HashMap::new();

    for assignment in assignments {
        let mut path = assignment.path.clone();

        while let Some((ref prefix, segment)) = path.split_last() {
            match segment {
                Segment::Index(index) => {
                    arrays.entry(prefix.clone()).or_default().insert(index);
                }
                Segment::Key(_) => {}
            };
            path = prefix.clone();
        }
    }

    for (prefix, indices) in arrays {
        let indices: Vec<_> = indices.into_iter().collect();

        let first = *indices.first().expect("non-empty");

        if first != 0 {
            Err(PathError {
                path: prefix.clone(),
                variant: PathErrorVariant::IncompleteArray {
                    index_seen: first,
                    index_missing: 0,
                },
            })?;
        }

        for pair in indices.windows(2) {
            let [left, right] = pair else { unreachable!() };
            if *left != right - 1 {
                Err(PathError {
                    path: prefix.clone(),
                    variant: PathErrorVariant::IncompleteArray {
                        index_seen: *right,
                        index_missing: left + 1,
                    },
                })?;
            }
        }
    }

    Ok(())
}
