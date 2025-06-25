use serde_json::Deserializer;
use serde_json::Value;
use std::num::ParseIntError;
use std::rc::Rc;
use unicode_ident::is_xid_continue;
use unicode_ident::is_xid_start;

#[derive(Debug)]
struct AssignmentAst {
    path: Vec<SegmentAst>,
    operator: OperatorAst,
    value: String,
}

#[derive(Debug, Eq, PartialEq)]
enum OperatorAst {
    Colon,
    EqualSign,
}

#[derive(Debug)]
enum SegmentAst {
    Index(u32),
    Identifier(String),
    Key(String),
}

pub struct ParseError {
    pub pos: usize,
    pub inner: ParseErrorInner,
}

impl ParseError {
    fn new<I: Into<ParseErrorInner>>(pos: usize, inner: I) -> Self {
        ParseError {
            pos,
            inner: inner.into(),
        }
    }

    fn offset(self, offset: usize) -> ParseError {
        ParseError {
            pos: self.pos + offset,
            inner: self.inner,
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "position {}: {}", self.pos, self.inner)
    }
}

pub enum ParseErrorInner {
    Message(String),
    Json(serde_json::Error),
    Int(ParseIntError),
}

impl From<String> for ParseErrorInner {
    fn from(message: String) -> Self {
        ParseErrorInner::Message(message)
    }
}

impl From<ParseIntError> for ParseErrorInner {
    fn from(error: ParseIntError) -> Self {
        ParseErrorInner::Int(error)
    }
}

impl From<serde_json::Error> for ParseErrorInner {
    fn from(error: serde_json::Error) -> Self {
        ParseErrorInner::Json(error)
    }
}

impl std::fmt::Display for ParseErrorInner {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ParseErrorInner::Message(message) => write!(f, "{}", message),
            ParseErrorInner::Int(error) => write!(f, "{}", error),
            ParseErrorInner::Json(error) => write!(f, "{}", error),
        }
    }
}

type ParseResult<'a, T> = Result<(T, usize, &'a str), ParseError>;

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

pub fn parse(input: &str) -> Result<Assignment, ParseError> {
    Ok(parse_assignment(input)?.0.into())
}

fn parse_assignment(input: &str) -> ParseResult<'_, AssignmentAst> {
    let (path, pos, input) = parse_path(input)?;
    let (operator, pos, input) = parse_operator(input).map_err(|e| e.offset(pos))?;

    if operator == OperatorAst::Colon {
        let de = Deserializer::from_str(input);
        let mut stream = de.into_iter::<Value>();

        match stream.next() {
            Some(Ok(_value)) => {
                // Position after the valid JSON
                let offset = stream.byte_offset();

                // Check for non-whitespace garbage
                let rest = &input[offset..];
                if let Some((end_pod, ch)) = rest.char_indices().find(|&(_, c)| !c.is_whitespace())
                {
                    Err(ParseError::new(
                        pos + offset + end_pod,
                        format!("unexpected character '{}'", ch),
                    ))?;
                }
            }
            Some(Err(e)) => Err(ParseError::new(pos, e))?,
            None => Err(ParseError::new(pos, "unexpected end of string".to_string()))?,
        }
    }

    Ok((
        AssignmentAst {
            path,
            operator,
            value: input.to_string(),
        },
        0,
        "",
    ))
}

fn parse_path(input: &str) -> ParseResult<'_, Vec<SegmentAst>> {
    if input.starts_with('.') {
        Ok((vec![], 1, &input[1..]))
    } else {
        let mut segments = vec![];

        let (first, mut pos, mut input) = parse_segment(input)?;
        segments.push(first);

        while input.starts_with('.') {
            let segment;
            let offset;
            (segment, offset, input) = parse_segment(&input[1..]).map_err(|e| e.offset(pos))?;
            segments.push(segment);
            pos += offset;
        }

        Ok((segments, pos, input))
    }
}

fn parse_segment(input: &str) -> ParseResult<'_, SegmentAst> {
    if input.starts_with('"') {
        #[derive(Eq, PartialEq)]
        enum State {
            Normal,
            Escaped,
        }
        let mut state = State::Normal;
        let split_index = input
            .char_indices()
            .skip(1)
            .find(|&(_, c)| {
                if state == State::Escaped {
                    state = State::Normal;
                    false
                } else if c == '\\' {
                    state = State::Escaped;
                    false
                } else {
                    c == '"'
                }
            })
            .map(|(i, _)| i + 1);
        if let Some(index) = split_index {
            let (segment, rest) = input.split_at(index);
            let _: serde_json::Value =
                serde_json::from_str(segment).map_err(|e| ParseError::new(index, e))?;
            Ok((SegmentAst::Key(segment.to_string()), index, rest))
        } else {
            Err(ParseError::new(
                input.len(),
                "unexpected end of string".to_string(),
            ))
        }
    } else if input.starts_with(is_xid_start) {
        let split_index = input
            .char_indices()
            .find(|&(_, c)| !is_xid_continue(c))
            .map(|(i, _)| i)
            .unwrap_or_else(|| input.len());
        let (index, rest) = input.split_at(split_index);
        Ok((SegmentAst::Identifier(index.to_string()), split_index, rest))
    } else if input.starts_with('0') {
        Ok((SegmentAst::Index(0), 1, &input[1..]))
    } else if input.starts_with(|ch: char| ch.is_ascii_digit()) {
        let split_index = input
            .char_indices()
            .find(|&(_, c)| !c.is_ascii_digit())
            .map(|(i, _)| i)
            .unwrap_or_else(|| input.len());
        let (index, rest) = input.split_at(split_index);
        let index = index.parse().map_err(|e| ParseError::new(0, e))?;
        Ok((SegmentAst::Index(index), split_index, rest))
    } else if let Some(first) = input.chars().next() {
        Err(ParseError::new(
            0,
            format!("unexpected character '{}'", first),
        ))
    } else {
        Err(ParseError::new(0, "unexpected end of string".to_string()))
    }
}

fn parse_operator(input: &str) -> ParseResult<OperatorAst> {
    if input.starts_with(':') {
        Ok((OperatorAst::Colon, 1, &input[1..]))
    } else if input.starts_with('=') {
        Ok((OperatorAst::EqualSign, 1, &input[1..]))
    } else if let Some(first) = input.chars().next() {
        Err(ParseError::new(
            0,
            format!("unexpected character '{}'", first),
        ))
    } else {
        Err(ParseError::new(0, "unexpected end of string".to_string()))
    }
}

// TESTS:
//  * Accept leading BOM
//  * Test behavior around unpaired UTF-16 surrogate (e.g., \uDEAD)
//  * Reject objects with conflicting representations of the same key (e.g., "a\\b" and "a\u005Cb")
