use serde_json::Deserializer;
use serde_json::Value;
use std::num::ParseIntError;
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

#[derive(Debug)]
pub struct Assignment {
    path: Vec<Segment>,
    value: String,
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

#[derive(Debug)]
pub enum Segment {
    Index(u32),
    Key(String),
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
