use serde_json::Deserializer;
use serde_json::Value;
use std::process::ExitCode;
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
    Index(String),
    Identifier(String),
    Key(String),
}

struct ParseError {
    pos: usize,
    inner: ParseErrorInner,
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

enum ParseErrorInner {
    Message(String),
    Json(serde_json::Error),
}

impl From<String> for ParseErrorInner {
    fn from(message: String) -> Self {
        ParseErrorInner::Message(message)
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
            ParseErrorInner::Json(error) => write!(f, "{}", error),
        }
    }
}

type ParseResult<'a, T> = Result<(T, usize, &'a str), ParseError>;

fn main() -> ExitCode {
    let mut assignments = std::env::args();
    let _command = assignments.next();

    match transform(assignments) {
        Ok(json) => {
            println!("{}", json);
            ExitCode::from(0)
        }
        Err(message) => {
            eprintln!("input error: {}", message);
            ExitCode::from(2)
        }
    }
}

fn transform<'a>(assignments: impl Iterator<Item = String>) -> Result<String, String> {
    let mut results = vec![];
    for assignment in assignments {
        let (assignment, _, _) = parse_assignment(&assignment).map_err(|e| {
            format!(
                "assignment \"{}\": {}",
                assignment.escape_default(),
                e.inner
            )
        })?;
        results.push(format!("{:?}", assignment));
    }
    Ok(results.join("\n"))
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
        Ok((SegmentAst::Index("0".to_string()), 1, &input[1..]))
    } else if input.starts_with(|ch: char| ch.is_ascii_digit()) {
        let split_index = input
            .char_indices()
            .find(|&(_, c)| !c.is_ascii_digit())
            .map(|(i, _)| i)
            .unwrap_or_else(|| input.len());
        let (index, rest) = input.split_at(split_index);
        Ok((SegmentAst::Index(index.to_string()), split_index, rest))
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
