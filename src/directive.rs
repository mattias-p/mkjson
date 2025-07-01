use crate::parser::DirectiveAst;
use crate::parser::OperatorAst;
use crate::parser::SegmentAst;
use crate::parser::is_xid_string;
use std::cmp::Ordering;
use std::collections::VecDeque;
use std::rc::Rc;

#[derive(Debug)]
pub struct Directive {
    pub path: Rc<Path>,
    pub value: String,
}

impl From<DirectiveAst> for Directive {
    fn from(ast: DirectiveAst) -> Self {
        let path = ast.path.into_iter().map(|segment| segment.into()).collect();
        let value = if ast.operator == OperatorAst::Colon {
            ast.value
        } else {
            format!(r#""{}""#, escape_string(&ast.value))
        };
        Directive { path, value }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Segment {
    Index(u32),
    Key(Rc<String>),
}

impl Segment {
    pub fn unescape(&self) -> Segment {
        match self {
            Segment::Key(key) => Segment::Key(Rc::new(unescape_string(key))),
            _ => self.clone(),
        }
    }

    pub fn as_unquoted(&self) -> Option<&str> {
        match self {
            Segment::Key(key) => Some(&key),
            _ => None,
        }
    }
}

impl std::fmt::Display for Segment {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Segment::Index(index) => write!(f, "{}", index),
            Segment::Key(key) => {
                if is_xid_string(key) {
                    write!(f, "{}", key)
                } else {
                    write!(f, r#""{}""#, key)
                }
            }
        }
    }
}

impl From<SegmentAst> for Segment {
    fn from(ast: SegmentAst) -> Self {
        match ast {
            SegmentAst::ArrayIndex(index) => Segment::Index(index),
            SegmentAst::QuotedKey(quoted) => {
                Segment::Key(Rc::new(quoted[1..quoted.len() - 1].to_string()))
            }
            SegmentAst::BareKey(bare) => Segment::Key(Rc::new(escape_string(&bare))),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq)]
pub enum Path {
    Root,
    Append(Rc<Path>, Segment),
}

impl Path {
    pub fn root() -> Rc<Self> {
        Rc::new(Path::Root)
    }

    pub fn append<T: Into<Segment>>(self: &Rc<Self>, segment: T) -> Rc<Self> {
        Rc::new(Path::Append(self.clone(), segment.into()))
    }

    pub fn prefix(&self) -> Option<Rc<Path>> {
        match self {
            Path::Root => None,
            Path::Append(prefix, _) => Some(prefix.clone()),
        }
    }

    pub fn split_last(&self) -> Option<(Rc<Path>, Segment)> {
        match self {
            Path::Root => None,
            Path::Append(prefix, segment) => Some((prefix.clone(), segment.clone())),
        }
    }

    pub fn split_first(&self) -> Option<(Segment, Rc<Path>)> {
        let mut segments = VecDeque::new();
        let mut path = self;
        while let Path::Append(prefix, segment) = path {
            segments.push_front(segment);
            path = prefix;
        }
        match segments.pop_front() {
            None => None,
            Some(first) => Some((
                (*first).clone(),
                segments.iter().cloned().cloned().collect(),
            )),
        }
    }

    pub fn unescape(self: &Rc<Self>) -> Rc<Self> {
        match &**self {
            Path::Root => self.clone(),
            Path::Append(prefix, key) => prefix.unescape().append(key.unescape()),
        }
    }

    pub fn iter(self: &Rc<Path>) -> impl Iterator<Item = (Rc<Path>, Segment)> {
        PathIter { path: self.clone() }
    }

    pub fn len(&self) -> usize {
        match self {
            Path::Root => 0,
            Path::Append(prefix, _) => prefix.len() + 1,
        }
    }
}

impl PartialOrd for Path {
    fn partial_cmp(&self, rhs: &Self) -> Option<std::cmp::Ordering> {
        match (self, rhs) {
            (Path::Root, Path::Root) => Some(Ordering::Equal),
            (Path::Root, _) => Some(Ordering::Greater),
            (_, Path::Root) => Some(Ordering::Less),
            (Path::Append(lhs_prefix, lhs_segment), Path::Append(rhs_prefix, rhs_segment)) => {
                match lhs_prefix.cmp(rhs_prefix) {
                    Ordering::Equal => lhs_segment.partial_cmp(rhs_segment),
                    ordering => Some(ordering),
                }
            }
        }
    }
}

impl std::fmt::Display for Path {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let Some((prefix, segment)) = self.split_last() {
            fn aux(f: &mut std::fmt::Formatter, path: &Rc<Path>) -> std::fmt::Result {
                match &**path {
                    Path::Root => Ok(()),
                    Path::Append(prefix, segment) => {
                        aux(f, prefix)?;
                        write!(f, "{}.", segment)
                    }
                }
            }
            aux(f, &prefix)?;
            write!(f, "{}", segment)
        } else {
            write!(f, ".")
        }
    }
}

struct PathIter {
    path: Rc<Path>,
}

impl Iterator for PathIter {
    type Item = (Rc<Path>, Segment);

    fn next(&mut self) -> Option<Self::Item> {
        let path = self.path.clone();
        match &*path {
            Path::Root => None,
            Path::Append(prefix, segment) => {
                self.path = prefix.clone();
                Some((prefix.clone(), segment.clone()))
            }
        }
    }
}

impl FromIterator<Segment> for Rc<Path> {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = Segment>,
    {
        iter.into_iter()
            .fold(Path::root(), |path, segment| path.append(segment))
    }
}

fn nibble_to_hex(n: u8) -> char {
    debug_assert!(n < 16);
    match n {
        0..=9 => (b'0' + n) as char,
        10..=15 => (b'a' + (n - 10)) as char, // use b'A' for uppercase
        _ => unreachable!(),
    }
}

fn escape_string(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            '\\' | '"' => vec!['\\', c],
            '\n' => vec!['\\', 'n'],
            '\r' => vec!['\\', 'r'],
            '\t' => vec!['\\', 't'],
            '\x0c' => vec!['\\', 'f'],
            '\x08' => vec!['\\', 'b'],
            '\x00'..='\x1f' => {
                let byte = c as u8;
                let high_nibble = byte >> 4; // upper 4 bits
                let low_nibble = byte & 0x0f; // lower 4 bits
                vec![
                    '\\',
                    'u',
                    '0',
                    '0',
                    nibble_to_hex(high_nibble),
                    nibble_to_hex(low_nibble),
                ]
            }
            _ => vec![c],
        })
        .collect()
}

fn unescape_string(s: &str) -> String {
    #[derive(Clone, Copy)]
    enum State {
        Normal,
        Escaped,
        Hexcode0,
        Hexcode1,
        Hexcode2,
        Hexcode3,
    }
    impl State {
        fn next(self) -> Self {
            match self {
                State::Normal => State::Escaped,
                State::Escaped => State::Hexcode0,
                State::Hexcode0 => State::Hexcode1,
                State::Hexcode1 => State::Hexcode2,
                State::Hexcode2 => State::Hexcode3,
                State::Hexcode3 => State::Normal,
            }
        }
    }
    let mut state = State::Normal;
    let mut acc = 0u32;
    let unescaped = s.chars().flat_map(|c| match (state, c) {
        (State::Normal, '\\') => {
            state = State::Escaped;
            vec![]
        }
        (State::Normal, _)
        | (State::Escaped, '"')
        | (State::Escaped, '\\')
        | (State::Escaped, '/') => {
            state = State::Normal;
            vec![c]
        }
        (State::Escaped, 'b') => vec![char::from_u32(0x08).expect("valid codepoint")],
        (State::Escaped, 'f') => vec![char::from_u32(0x0c).expect("valid codepoint")],
        (State::Escaped, 'n') => vec![char::from_u32(0x0a).expect("valid codepoint")],
        (State::Escaped, 'r') => vec![char::from_u32(0x0d).expect("valid codepoint")],
        (State::Escaped, 't') => vec![char::from_u32(0x09).expect("valid codepoint")],
        (State::Escaped, 'u') => {
            state = State::Hexcode0;
            vec![]
        }
        (State::Hexcode0 | State::Hexcode1 | State::Hexcode2, _) => {
            state = state.next();
            acc = acc << 4
                | c.to_digit(16)
                    .expect("caller is responsible for only unescaping valid strings");
            vec![]
        }
        (State::Hexcode3, _) => {
            state = State::Normal;
            let unescaped = char::from_u32(
                acc << 4
                    | c.to_digit(16)
                        .expect("caller is responsible for only unescaping valid strings"),
            )
            .expect("caller is responsible for only unescaping valid strings");
            acc = 0;
            vec![unescaped]
        }
        _ => unreachable!(),
    });
    Some('"')
        .into_iter()
        .chain(unescaped)
        .chain(Some('"').into_iter())
        .collect()
}
