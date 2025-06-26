use crate::parser::AssignmentAst;
use crate::parser::OperatorAst;
use crate::parser::SegmentAst;
use std::rc::Rc;

#[derive(Debug)]
pub struct Assignment {
    pub path: Rc<Path>,
    pub value: String,
}

impl From<AssignmentAst> for Assignment {
    fn from(ast: AssignmentAst) -> Self {
        let path = ast.path.into_iter().map(|segment| segment.into()).collect();
        let value = if ast.operator == OperatorAst::Colon {
            ast.value
        } else {
            escape_string(&ast.value)
        };
        Assignment { path, value }
    }
}

#[derive(Debug, Eq, Hash, PartialEq)]
pub enum Segment {
    Index(u32),
    Key(String),
}

impl Segment {
    pub fn normalize(self: &Rc<Self>) -> Rc<Segment> {
        match &**self {
            Segment::Key(key) => Rc::new(Segment::Key(unescape_string(key))),
            _ => self.clone(),
        }
    }
}

impl std::fmt::Display for Segment {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Segment::Index(index) => write!(f, "{}", index),
            Segment::Key(key) => write!(f, "{}", key),
        }
    }
}

impl From<SegmentAst> for Segment {
    fn from(ast: SegmentAst) -> Self {
        match ast {
            SegmentAst::Index(index) => Segment::Index(index),
            SegmentAst::Key(key) => Segment::Key(key),
            SegmentAst::Identifier(identifier) => Segment::Key(escape_string(&identifier)),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Path {
    Root,
    Append(Rc<Path>, Rc<Segment>),
}

impl Path {
    pub fn root() -> Rc<Self> {
        Rc::new(Path::Root)
    }

    pub fn append<T: Into<Rc<Segment>>>(self: &Rc<Self>, segment: T) -> Rc<Self> {
        Rc::new(Path::Append(self.clone(), segment.into()))
    }

    pub fn prefix(&self) -> Option<Rc<Path>> {
        match self {
            Path::Root => None,
            Path::Append(prefix, _) => Some(prefix.clone()),
        }
    }

    pub fn split_last(&self) -> Option<(Rc<Path>, Rc<Segment>)> {
        match self {
            Path::Root => None,
            Path::Append(prefix, segment) => Some((prefix.clone(), segment.clone())),
        }
    }

    pub fn normalize(self: &Rc<Self>) -> Rc<Self> {
        match &**self {
            Path::Root => self.clone(),
            Path::Append(prefix, key) => prefix.normalize().append(key.normalize()),
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

impl FromIterator<Segment> for Rc<Path> {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = Segment>,
    {
        iter.into_iter()
            .fold(Path::root(), |path, segment| path.append(segment))
    }
}

fn escape_string(s: &str) -> String {
    let escaped = s.chars().flat_map(|c| {
        if c == '\\' || c == '"' {
            vec!['\\', c]
        } else {
            vec![c]
        }
    });
    Some('"')
        .into_iter()
        .chain(escaped)
        .chain(Some('"').into_iter())
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
