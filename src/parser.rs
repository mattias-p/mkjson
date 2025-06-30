use serde_json::Deserializer;
use serde_json::Value;
use snafu::prelude::*;
use unicode_ident::is_xid_continue;
use unicode_ident::is_xid_start;

#[derive(Debug)]
pub struct DirectiveAst {
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
pub enum SyntaxError {
    #[snafu(display("position {pos}: unexpected character '{ch}'"))]
    UnexpectedChar { pos: usize, ch: char },

    #[snafu(display("unexpected end of string"))]
    UnexpectedEndOfString,

    #[snafu(display("position {pos}: invalid index"))]
    InvalidIndex {
        pos: usize,
        source: std::num::ParseIntError,
    },

    #[snafu(display("position {pos}: invalid key"))]
    InvalidKey {
        pos: usize,
        source: serde_json::Error,
    },

    #[snafu(display("position {pos}: invalid json value"))]
    InvalidJsonValue {
        pos: usize, // TODO: remove this once we can have origin-aware JSON parsing errors
        source: serde_json::Error,
    },
}

type ParseResult<'a, T> = Result<(T, usize, &'a str), SyntaxError>;

pub fn validate_json(start_pos: usize, input: &str) -> ParseResult<'_, ()> {
    if (input.starts_with('{') || input.starts_with('['))
        && !input.starts_with("{}")
        && !input.starts_with("[]")
    {
        if let Some(ch) = input.chars().nth(1) {
            Err(SyntaxError::UnexpectedChar {
                pos: start_pos + 1,
                ch,
            })?;
        } else {
            Err(SyntaxError::UnexpectedEndOfString)?;
        }
    }

    let de = Deserializer::from_str(input);
    let mut stream = de.into_iter::<Value>();

    match stream.next() {
        Some(Ok(_)) => {
            // Position after the valid JSON
            let offset = stream.byte_offset();

            // Check for non-whitespace garbage
            let rest = &input[offset..];
            if let Some((end_index, ch)) = rest.char_indices().find(|&(_, c)| !c.is_whitespace()) {
                Err(SyntaxError::UnexpectedChar {
                    pos: start_pos + offset + end_index,
                    ch,
                })?;
            }
            Ok(((), start_pos + input.len(), ""))
        }
        Some(Err(e)) => Err(SyntaxError::InvalidJsonValue {
            pos: start_pos,
            source: e,
        }),
        None => Err(SyntaxError::UnexpectedEndOfString),
    }
}

pub fn parse_directive(start_pos: usize, input: &str) -> ParseResult<'_, DirectiveAst> {
    let (path, pos, input) = parse_path(start_pos, input)?;
    let (operator, pos, input) = parse_operator(pos, input)?;

    if operator == OperatorAst::Colon {
        validate_json(pos, input)?;
    }

    Ok((
        DirectiveAst {
            path,
            operator,
            value: input.to_string(),
        },
        start_pos + input.len(),
        "",
    ))
}

pub fn parse_path(start_pos: usize, input: &str) -> ParseResult<'_, Vec<SegmentAst>> {
    if input.starts_with('.') {
        Ok((vec![], start_pos + 1, &input[1..]))
    } else {
        let mut segments = vec![];

        let (first, mut pos, mut input) = parse_segment(start_pos, input)?;
        segments.push(first);

        while input.starts_with('.') {
            let segment;
            (segment, pos, input) = parse_segment(pos + 1, &input[1..])?;
            segments.push(segment);
        }

        Ok((segments, pos, input))
    }
}

pub fn parse_segment(start_pos: usize, input: &str) -> ParseResult<'_, SegmentAst> {
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
        if let Some(split_index) = split_index {
            let (segment, rest) = input.split_at(split_index);
            let _: serde_json::Value = serde_json::from_str(segment).context(InvalidKeySnafu {
                pos: start_pos + split_index,
            })?;
            Ok((
                SegmentAst::Key(segment.to_string()),
                start_pos + split_index,
                rest,
            ))
        } else {
            Err(SyntaxError::UnexpectedEndOfString)
        }
    } else if input.starts_with(is_xid_start) {
        let split_index = input
            .char_indices()
            .find(|&(_, c)| !is_xid_continue(c))
            .map(|(i, _)| i)
            .unwrap_or_else(|| input.len());
        let (index, rest) = input.split_at(split_index);
        Ok((
            SegmentAst::Identifier(index.to_string()),
            start_pos + split_index,
            rest,
        ))
    } else if input.starts_with('0') {
        Ok((SegmentAst::Index(0), start_pos + 1, &input[1..]))
    } else if input.starts_with(|ch: char| ch.is_ascii_digit()) {
        let split_index = input
            .char_indices()
            .find(|&(_, c)| !c.is_ascii_digit())
            .map(|(i, _)| i)
            .unwrap_or_else(|| input.len());
        let (index, rest) = input.split_at(split_index);
        let index = index.parse().context(InvalidIndexSnafu {
            pos: start_pos + split_index,
        })?;
        Ok((SegmentAst::Index(index), start_pos + split_index, rest))
    } else if let Some(first) = input.chars().next() {
        Err(SyntaxError::UnexpectedChar {
            pos: start_pos,
            ch: first,
        })
    } else {
        Err(SyntaxError::UnexpectedEndOfString)
    }
}

pub fn parse_operator(pos: usize, input: &str) -> ParseResult<OperatorAst> {
    if input.starts_with(':') {
        Ok((OperatorAst::Colon, pos + 1, &input[1..]))
    } else if input.starts_with('=') {
        Ok((OperatorAst::EqualSign, pos + 1, &input[1..]))
    } else if let Some(first) = input.chars().next() {
        Err(SyntaxError::UnexpectedChar { pos, ch: first })
    } else {
        Err(SyntaxError::UnexpectedEndOfString)
    }
}

pub fn is_xid_string(s: &str) -> bool {
    s.starts_with(is_xid_start) && s.chars().find(|c| !is_xid_continue(*c)).is_none()
}

// TESTS:
//  * Accept leading BOM
//  * Test behavior around unpaired UTF-16 surrogate (e.g., \uDEAD)
//  * Reject objects with conflicting representations of the same key (e.g., "a\\b" and "a\u005Cb")
