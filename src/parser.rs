use serde_json::Deserializer;
use serde_json::Value;
use snafu::prelude::*;
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

#[derive(Debug, Snafu)]
pub enum ParseError {
    #[snafu(display("position {pos}: unexpected character '{ch}'"))]
    UnexpectedCharacter { pos: usize, ch: char },

    #[snafu(display("unexpected end of string"))]
    UnexpectedEndOfString,

    #[snafu(display("position {pos}: invalid index"))]
    InvalidIndex {
        source: std::num::ParseIntError,
        pos: usize,
    },

    #[snafu(display("position {pos}: invalid key"))]
    InvalidKey {
        source: serde_json::Error,
        pos: usize,
    },

    #[snafu(display("position {pos}: invalid json value"))]
    InvalidJsonValue {
        source: serde_json::Error,
        pos: usize,
    },
}

impl ParseError {
    fn offset(self, offset: usize) -> ParseError {
        match self {
            ParseError::UnexpectedCharacter { pos, ch } => ParseError::UnexpectedCharacter {
                pos: pos + offset,
                ch,
            },
            ParseError::UnexpectedEndOfString => ParseError::UnexpectedEndOfString,
            ParseError::InvalidIndex { pos, source } => ParseError::InvalidIndex {
                pos: pos + offset,
                source,
            },
            ParseError::InvalidKey { pos, source } => ParseError::InvalidKey {
                pos: pos + offset,
                source,
            },
            ParseError::InvalidJsonValue { pos, source } => ParseError::InvalidJsonValue {
                pos: pos + offset,
                source,
            },
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
                Err(ParseError::UnexpectedCharacter {
                    pos: pos + offset + end_pos,
                    ch,
                })?;
            }
            Ok(((), pos + input.len(), ""))
        }
        Some(Err(e)) => Err(ParseError::InvalidJsonValue { pos, source: e }),
        None => Err(ParseError::UnexpectedEndOfString),
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
                serde_json::from_str(segment).context(InvalidKeySnafu { pos: index })?;
            Ok((SegmentAst::Key(segment.to_string()), index, rest))
        } else {
            Err(ParseError::UnexpectedEndOfString)
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
        let index = index
            .parse()
            .context(InvalidIndexSnafu { pos: split_index })?;
        Ok((SegmentAst::Index(index), split_index, rest))
    } else if let Some(first) = input.chars().next() {
        Err(ParseError::UnexpectedCharacter { pos: 0, ch: first })
    } else {
        Err(ParseError::UnexpectedEndOfString)
    }
}

pub fn parse_operator(input: &str) -> ParseResult<OperatorAst> {
    if input.starts_with(':') {
        Ok((OperatorAst::Colon, 1, &input[1..]))
    } else if input.starts_with('=') {
        Ok((OperatorAst::EqualSign, 1, &input[1..]))
    } else if let Some(first) = input.chars().next() {
        Err(ParseError::UnexpectedCharacter { pos: 0, ch: first })
    } else {
        Err(ParseError::UnexpectedEndOfString)
    }
}

pub fn is_xid_string(s: &str) -> bool {
    s.starts_with(is_xid_start) && s.chars().find(|c| !is_xid_continue(*c)).is_none()
}

// TESTS:
//  * Accept leading BOM
//  * Test behavior around unpaired UTF-16 surrogate (e.g., \uDEAD)
//  * Reject objects with conflicting representations of the same key (e.g., "a\\b" and "a\u005Cb")
