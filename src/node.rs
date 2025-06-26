use crate::assignment::Assignment;
use crate::assignment::Path;
use crate::assignment::Segment;
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::rc::Rc;

pub fn build_tree(mut assignments: impl Iterator<Item = Assignment>) -> Option<Node> {
    if let Some(first) = assignments.next() {
        let mut node = Node::create(&first.path, first.value.clone());
        for assignment in assignments {
            node.insert(&assignment.path, assignment.value.clone());
        }
        Some(node)
    } else {
        None
    }
}

#[derive(Debug)]
pub enum Node {
    Value(String),
    Array(BTreeMap<u32, Node>),
    Object(BTreeMap<Rc<String>, Node>),
}

impl Node {
    pub fn create(path: &Rc<Path>, value: String) -> Node {
        match path.split_first() {
            None => Node::Value(value),
            Some((first, rest)) => {
                let child = Node::create(&rest, value);
                match first {
                    Segment::Index(index) => Node::Array(BTreeMap::from([(index, child)])),
                    Segment::Key(key) => Node::Object(BTreeMap::from([(key, child)])),
                }
            }
        }
    }

    pub fn insert(&mut self, path: &Rc<Path>, value: String) -> bool {
        let Some((first, rest)) = path.split_first() else {
            return false;
        };
        match first {
            Segment::Index(index) => {
                let Node::Array(array) = self else {
                    return false;
                };
                match array.entry(index) {
                    Entry::Vacant(vacant) => {
                        vacant.insert(Node::create(&rest, value));
                        true
                    }
                    Entry::Occupied(mut occupied) => occupied.get_mut().insert(&rest, value),
                }
            }
            Segment::Key(key) => {
                let Node::Object(object) = self else {
                    return false;
                };
                match object.entry(key) {
                    Entry::Vacant(vacant) => {
                        vacant.insert(Node::create(&rest, value));
                        true
                    }
                    Entry::Occupied(mut occupied) => occupied.get_mut().insert(&rest, value),
                }
            }
        }
    }
}
