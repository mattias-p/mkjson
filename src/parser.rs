use serde_json::Deserializer;
use serde_json::Value;
use std::num::ParseIntError;
use unicode_ident::is_xid_continue;
use unicode_ident::is_xid_start;

#[derive(Debug)]
pub struct AssignmentAst {
    pub path: Vec<SegmentAst>,
    pub operator: OperatorAst,
    pub value: String,
}

#[derive(Debug, Eq, PartialEq)]
pub enum OperatorAst {
    Colon,
    EqualSign,
}

#[derive(Debug)]
pub enum SegmentAst {
    Index(u32),
    Identifier(String),
    Key(String),
}

#[derive(Debug)]
pub struct ParseError {
    pub pos: usize,
    pub inner: ParseErrorInner,
}

impl ParseError {
    pub fn new<I: Into<ParseErrorInner>>(pos: usize, inner: I) -> Self {
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

impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.inner {
            ParseErrorInner::Message(_) => None,
            ParseErrorInner::Json(e) => Some(e),
            ParseErrorInner::Int(e) => Some(e),
        }
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "position {}: {}", self.pos, self.inner)
    }
}

#[derive(Debug)]
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

pub fn validate_json(pos: usize, input: &str) -> ParseResult<'_, ()> {
    let de = Deserializer::from_str(input);
    let mut stream = de.into_iter::<Value>();

    match stream.next() {
        Some(Ok(_value)) => {
            // Position after the valid JSON
            let offset = stream.byte_offset();

            // Check for non-whitespace garbage
            let rest = &input[offset..];
            if let Some((end_pos, ch)) = rest.char_indices().find(|&(_, c)| !c.is_whitespace()) {
                Err(ParseError::new(
                    pos + offset + end_pos,
                    format!("unexpected character '{}'", ch),
                ))?;
            }
            Ok(((), pos + input.len(), ""))
        }
        Some(Err(e)) => Err(ParseError::new(pos, e)),
        None => Err(ParseError::new(pos, "unexpected end of string".to_string())),
    }
}

pub fn parse_assignment(input: &str) -> ParseResult<'_, AssignmentAst> {
    let (path, pos, input) = parse_path(input)?;
    let (operator, pos, input) = parse_operator(input).map_err(|e| e.offset(pos))?;

    if operator == OperatorAst::Colon {
        validate_json(pos, input)?;
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

pub fn parse_path(input: &str) -> ParseResult<'_, Vec<SegmentAst>> {
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

pub fn parse_segment(input: &str) -> ParseResult<'_, SegmentAst> {
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

pub fn parse_operator(input: &str) -> ParseResult<OperatorAst> {
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

pub fn is_xid_string(s: &str) -> bool {
    s.starts_with(is_xid_start) && s.chars().find(|c| !is_xid_continue(*c)).is_none()
}

// TESTS:
//  * Accept leading BOM
//  * Test behavior around unpaired UTF-16 surrogate (e.g., \uDEAD)
//  * Reject objects with conflicting representations of the same key (e.g., "a\\b" and "a\u005Cb")
