use crate::directive::Directive;
use crate::directive::Path;
use crate::directive::Segment;
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
    #[snafu(display("path has the same key with different encodings {encoding1} and {encoding2}"))]
    InconsistentKeyEncodings {
        encoding1: Segment,
        encoding2: Segment,
    },

    #[snafu(display("conflicting directives"))]
    ConflictingDirectives,

    #[snafu(display("path referred to as both {kind1} and {kind2}"))]
    StructuralConflict { kind1: NodeKind, kind2: NodeKind },

    #[snafu(display("array at path has index {index_seen} but lacks index {index_missing}",))]
    IncompleteArray { index_seen: u32, index_missing: u32 },
}

type ValidationResult = Result<(), PathError>;

pub fn validate(directives: &[Directive]) -> ValidationResult {
    check_key_consistency(directives)?;
    check_path_uniqueness(directives)?;
    check_node_types(directives)?;
    check_array_completeness(directives)
}

fn check_key_consistency(directives: &[Directive]) -> ValidationResult {
    let mut keys: HashMap<Rc<Path>, Segment> = HashMap::new();
    for directive in directives {
        let mut given_path = directive.path.clone();
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
                                variant: PathErrorVariant::InconsistentKeyEncodings {
                                    encoding1: occupied.get().clone(),
                                    encoding2: given_segment,
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

fn check_path_uniqueness(directives: &[Directive]) -> ValidationResult {
    let mut paths = HashSet::new();

    for directive in directives {
        if !paths.insert(directive.path.clone()) {
            Err(PathError {
                variant: PathErrorVariant::ConflictingDirectives,
                path: directive.path.clone(),
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

fn check_node_types(directives: &[Directive]) -> ValidationResult {
    let mut types: HashMap<Rc<Path>, NodeKind> = HashMap::new();

    for directive in directives {
        let mut path = directive.path.clone();

        match types.entry(path.clone()) {
            Entry::Vacant(vacant) => vacant.insert(NodeKind::Value),
            Entry::Occupied(occupied) => Err(PathError {
                path: path.clone(),
                variant: PathErrorVariant::StructuralConflict {
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
                    variant: PathErrorVariant::StructuralConflict {
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

fn check_array_completeness(directives: &[Directive]) -> ValidationResult {
    let mut arrays: HashMap<Rc<Path>, BTreeSet<u32>> = HashMap::new();

    for directive in directives {
        let mut path = directive.path.clone();

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
