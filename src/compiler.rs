use crate::node::Node;
use crate::node::build_tree;
use crate::parser::SyntaxError;
use crate::parser::parse_assignment;
use crate::validator::SemanticError;
use crate::validator::validate;
use snafu::prelude::*;

#[derive(Debug, Snafu)]
pub enum CompileError {
    #[snafu(display("assignment \"{assignment}\": {source}"))]
    Syntax {
        source: SyntaxError,
        assignment: String,
    },

    #[snafu(display("validating: {source}"))]
    Semantic { source: SemanticError },
}

type CompileResult<T> = Result<T, CompileError>;

pub fn compile<'a>(inputs: impl Iterator<Item = String>) -> CompileResult<Option<Node>> {
    let mut assignments = vec![];
    for text in inputs {
        let (ast, _, _) = parse_assignment(1, &text).context(SyntaxSnafu {
            assignment: text.escape_default().to_string(),
        })?;
        assignments.push(ast.into());
    }

    validate(assignments.as_slice()).context(SemanticSnafu)?;

    Ok(build_tree(assignments.into_iter()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assignment::Path;
    use crate::parser::parse_path;
    use crate::validator::NodeKind;
    use assert_matches::assert_matches;
    use std::rc::Rc;

    fn new_path(s: &str) -> Rc<Path> {
        let (asts, _, _) = parse_path(1, s).unwrap();
        asts.into_iter().map(|ast| ast.into()).collect()
    }

    fn check(input: &[&str]) -> CompileResult<Option<String>> {
        let input = input.into_iter().map(|s| s.to_string());
        compile(input).map(|tree| tree.map(|node| node.to_string()))
    }

    #[test]
    fn test() {
        assert_eq!(
            check(&[]).unwrap(),
            None,
            "valid empty builder expression set"
        );
        assert_eq!(
            check(&[".=hello"]).unwrap(),
            Some(r#""hello""#.into()),
            "root JSON literal"
        );
        assert_eq!(
            check(&[".:true"]).unwrap(),
            Some(r#"true"#.into()),
            "root array"
        );
        assert_eq!(check(&["0=x", "1=y"]).unwrap(), Some(r#"["x","y"]"#.into()));
        assert_eq!(
            check(&[r#"""="#]).unwrap(),
            Some(r#"{"":""}"#.into()),
            "empty key and empty value"
        );
        assert_eq!(
            check(&["0:null", "1:true", "2:false"]).unwrap(),
            Some(r#"[null,true,false]"#.into()),
            "root array from JSON literals"
        );

        // nested objects and arrays
        assert_eq!(
            check(&["foo.bar=x"]).unwrap(),
            Some(r#"{"foo":{"bar":"x"}}"#.into())
        );
        assert_eq!(
            check(&["döner.kebab=x"]).unwrap(),
            Some(r#"{"döner":{"kebab":"x"}}"#.into())
        );
        assert_eq!(
            check(&["foo.bar=x", "foo.baz=y"]).unwrap(),
            Some(r#"{"foo":{"bar":"x","baz":"y"}}"#.into())
        );
        assert_eq!(
            check(&["foo.0.bar.0.baz=x"]).unwrap(),
            Some(r#"{"foo":[{"bar":[{"baz":"x"}]}]}"#.into())
        );
        assert_eq!(
            check(&["0.bar=x"]).unwrap(),
            Some(r#"[{"bar":"x"}]"#.into())
        );
        assert_eq!(check(&["0.0=x"]).unwrap(), Some(r#"[["x"]]"#.into()));
        assert_eq!(
            check(&["foo.0=x"]).unwrap(),
            Some(r#"{"foo":["x"]}"#.into())
        );
        assert_eq!(
            check(&["foo.0=x", "foo.1=y"]).unwrap(),
            Some(r#"{"foo":["x","y"]}"#.into())
        );
        assert_eq!(
            check(&["0.foo=x", "0.bar=y"]).unwrap(),
            Some(r#"[{"bar":"y","foo":"x"}]"#.into())
        );
        assert_eq!(
            check(&["0.0=x", "0.1=y"]).unwrap(),
            Some(r#"[["x","y"]]"#.into())
        );
        assert_eq!(
            check(&["emoji=😀"]).unwrap(),
            Some(r#"{"emoji":"😀"}"#.into())
        );
        assert_eq!(
            check(&["foo.bar.0:1", "foo.bar.1:2", "foo.bar.2:3"]).unwrap(),
            Some(r#"{"foo":{"bar":[1,2,3]}}"#.into())
        );

        // empty collections
        assert_eq!(
            check(&[".:{}"]).unwrap(),
            Some(r#"{}"#.into()),
            "empty object at root"
        );
        assert_eq!(
            check(&[".:[]"]).unwrap(),
            Some(r#"[]"#.into()),
            "empty array at root"
        );

        // string literal assignments (=)
        assert_eq!(check(&[".=string"]).unwrap(), Some(r#""string""#.into()));
        assert_eq!(
            check(&[r#".="quoted""#]).unwrap(),
            Some(r#""\"quoted\"""#.into())
        );
        assert_eq!(check(&[".=1"]).unwrap(), Some(r#""1""#.into()));
        assert_eq!(check(&[r#"""=1"#]).unwrap(), Some(r#"{"":"1"}"#.into()));
        assert_eq!(
            check(&["foo=123"]).unwrap(),
            Some(r#"{"foo":"123"}"#.into())
        );
        assert_eq!(
            check(&[r#""foo"=123"#]).unwrap(),
            Some(r#"{"foo":"123"}"#.into())
        );
        assert_eq!(
            check(&[r#""0"=123"#]).unwrap(),
            Some(r#"{"0":"123"}"#.into())
        );

        // raw JSON assignments (:)
        assert_eq!(check(&[".:[1,2,3]"]).unwrap(), Some(r#"[1,2,3]"#.into()));
        assert_eq!(
            check(&[r#".:{"foo":"x"}"#]).unwrap(),
            Some(r#"{"foo":"x"}"#.into())
        );
        assert_eq!(
            check(&[r#"foo:"123""#]).unwrap(),
            Some(r#"{"foo":"123"}"#.into())
        );
        assert_eq!(
            check(&[r#"foo:123"#]).unwrap(),
            Some(r#"{"foo":123}"#.into())
        );
        assert_eq!(
            check(&[r#"foo:[1,2,3]"#]).unwrap(),
            Some(r#"{"foo":[1,2,3]}"#.into())
        );
        assert_eq!(
            check(&[r#"a.b.c:1"#]).unwrap(),
            Some(r#"{"a":{"b":{"c":1}}}"#.into())
        );

        // array assignments
        assert_eq!(check(&["0=null"]).unwrap(), Some(r#"["null"]"#.into()));
        assert_eq!(check(&["0:null"]).unwrap(), Some(r#"[null]"#.into()));
        assert_eq!(
            check(&["0:null", "1:true"]).unwrap(),
            Some(r#"[null,true]"#.into())
        );

        // quoted and escaped segments and values
        assert_eq!(
            check(&[r#""foo.bar"=baz"#]).unwrap(),
            Some(r#"{"foo.bar":"baz"}"#.into()),
            "quoted segment with dot"
        );

        // RFC 8259 JSON type coverage
        assert_eq!(
            check(&[".:null"]).unwrap(),
            Some("null".into()),
            "null literal"
        );
        assert_eq!(
            check(&[".:true"]).unwrap(),
            Some("true".into()),
            "boolean true"
        );
        assert_eq!(
            check(&[".:false"]).unwrap(),
            Some("false".into()),
            "boolean false"
        );
        assert_eq!(
            check(&[".:0"]).unwrap(),
            Some("0".into()),
            "zero number literal"
        );
        assert_eq!(
            check(&[".:-1"]).unwrap(),
            Some("-1".into()),
            "negative number literal"
        );
        assert_eq!(
            check(&[".:-0"]).unwrap(),
            Some("-0".into()),
            "negative zero number literal"
        );
        assert_eq!(
            check(&[".:1.1"]).unwrap(),
            Some(r#"1.1"#.into()),
            "fractional number literal"
        );
        assert_eq!(
            check(&[".:1.00"]).unwrap(),
            Some(r#"1.00"#.into()),
            "fractional number literal with trailing zero decimals"
        );
        assert_eq!(
            check(&[".:3.141592653589793116"]).unwrap(),
            Some(r#"3.141592653589793116"#.into()),
            "closest approximation to π (IEEE 754 double precision)"
        );
        assert_eq!(
            check(&[".:3.141592653589793238462643383279"]).unwrap(),
            Some(r#"3.141592653589793238462643383279"#.into()),
            "approximation to π (beyond IEEE 754 double precision)"
        );
        assert_eq!(
            check(&[".:1.7976931348623157e308"]).unwrap(),
            Some(r#"1.7976931348623157e308"#.into()),
            "largest normal number (IEEE 754 double precision)"
        );
        assert_eq!(
            check(&[".:340282366920938463463374607431768211457"]).unwrap(),
            Some(r#"340282366920938463463374607431768211457"#.into()),
            "2^128+1"
        );

        if false {
            // TODO: fixme
            assert_eq!(
                check(&[".:1e400"]).unwrap(),
                Some(r#"1e400"#.into()),
                "very large normal number (beyond IEEE 754 double precision)"
            );
        }
        assert_eq!(
            check(&[r#".:"""#]).unwrap(),
            Some(r#""""#.into()),
            "string literal"
        );
        assert_eq!(
            check(&[r#".:"\u0041""#]).unwrap(),
            Some(r#""\u0041""#.into()),
            "string literal with Unicode escape"
        );
        assert_eq!(
            check(&[r#".:"\b\f\n\r\t""#]).unwrap(),
            Some(r#""\b\f\n\r\t""#.into()),
            "string literal with control character escapes"
        );
        assert_eq!(
            check(&[r#".:"\"\\""#]).unwrap(),
            Some(r#""\"\\""#.into()),
            "string literal with escape character escapes"
        );
        assert_eq!(
            check(&[r#".:"\ud83d\ude0a""#]).unwrap(),
            Some(r#""\ud83d\ude0a""#.into()),
            "string literal with escaped surrogate pair"
        );
        assert_eq!(
            check(&[r#".:"\u200b""#]).unwrap(),
            Some(r#""\u200b""#.into()),
            "string literal with escaped zero-width space"
        );
        assert_eq!(
            check(&[".:\"\u{200b}\""]).unwrap(),
            Some("\"\u{200b}\"".into()),
            "string literal with unescaped zero-width space"
        );
        assert_eq!(
            check(&[r#".:"abc\u203erev""#]).unwrap(),
            Some(r#""abc\u203erev""#.into()),
            "string literal with escaped bidi control character"
        );
        assert_eq!(
            check(&[".:\"abc\u{203e}rev\""]).unwrap(),
            Some("\"abc\u{203e}rev\"".into()),
            "string literal with unescaped bidi control character"
        );
        assert_eq!(
            check(&[r#".:"\u0001\u007f\u00a0\u2028""#]).unwrap(),
            Some(r#""\u0001\u007f\u00a0\u2028""#.into()),
            "string literal with mixed escaped unicode and control characters"
        );
        assert_eq!(
            check(&[".:{}"]).unwrap(),
            Some("{}".into()),
            "object literal"
        );
        assert_eq!(
            check(&[".:[]"]).unwrap(),
            Some("[]".into()),
            "array literal"
        );

        // path syntax errors
        assert!(matches!(
            check(&[""]),
            Err(CompileError::Syntax {
                source: SyntaxError::UnexpectedEndOfString,
                ..
            })
        ));
        assert!(matches!(
            check(&["foo"]),
            Err(CompileError::Syntax {
                source: SyntaxError::UnexpectedEndOfString,
                ..
            })
        ));
        assert!(matches!(
            check(&["\"unterminated"]),
            Err(CompileError::Syntax {
                source: SyntaxError::UnexpectedEndOfString,
                ..
            })
        ));
        assert_matches!(
            check(&["foo/bar=x"]),
            Err(CompileError::Syntax {
                source: SyntaxError::UnexpectedCharacter { pos: 4, ch: '/' },
                ..
            })
        );
        assert_matches!(
            check(&["00=x"]),
            Err(CompileError::Syntax {
                source: SyntaxError::UnexpectedCharacter { pos: 2, ch: '0' },
                ..
            })
        );
        assert_matches!(
            check(&["01=x"]),
            Err(CompileError::Syntax {
                source: SyntaxError::UnexpectedCharacter { pos: 2, ch: '1' },
                ..
            })
        );
        assert_matches!(
            check(&["=x"]),
            Err(CompileError::Syntax {
                source: SyntaxError::UnexpectedCharacter { pos: 1, ch: '=' },
                ..
            })
        );
        assert_matches!(
            check(&[".foo=x"]),
            Err(CompileError::Syntax {
                source: SyntaxError::UnexpectedCharacter { pos: 2, ch: 'f' },
                ..
            })
        );
        assert_matches!(
            check(&["foo.=x"]),
            Err(CompileError::Syntax {
                source: SyntaxError::UnexpectedCharacter { pos: 5, ch: '=' },
                ..
            })
        );
        assert_matches!(
            check(&["foo..bar=x"]),
            Err(CompileError::Syntax {
                source: SyntaxError::UnexpectedCharacter { pos: 5, ch: '.' },
                ..
            })
        );
        assert_matches!(
            check(&["foo.\u{0010}=x"]),
            Err(CompileError::Syntax {
                source: SyntaxError::UnexpectedCharacter {
                    pos: 5,
                    ch: '\u{0010}'
                },
                assignment,
            })
            if assignment == "foo.\\u{10}=x"
        );

        // JSON value syntax errors
        assert_matches!(
            check(&[".:hello"]),
            Err(CompileError::Syntax {
                source: SyntaxError::InvalidJsonValue { pos: 3, .. },
                ..
            })
        );
        assert_matches!(
            check(&[".:[1,2,]"]),
            Err(CompileError::Syntax {
                source: SyntaxError::InvalidJsonValue { pos: 3, .. },
                ..
            })
        );
        assert_matches!(
            check(&[".:{foo:bar}"]),
            Err(CompileError::Syntax {
                source: SyntaxError::InvalidJsonValue { pos: 3, .. },
                ..
            })
        );
        assert_matches!(
            check(&["\"unterminated"]),
            Err(CompileError::Syntax {
                source: SyntaxError::UnexpectedEndOfString,
                ..
            })
        );

        // conflicting implicit structure (array vs object)
        assert_matches!(
            check(&["foo.0=x", "foo.bar=y"]),
            Err(CompileError::Semantic {
                source: SemanticError::InconsistentNodeKind {
                    path,
                    kind1: NodeKind::Array,
                    kind2: NodeKind::Object,
                },
                ..
            })
            if path == new_path("foo")
        );
        assert_matches!(
            check(&["foo.bar=x", "foo.0=y"]),
            Err(CompileError::Semantic {
                source: SemanticError::InconsistentNodeKind {
                    path,
                    kind1: NodeKind::Object,
                    kind2: NodeKind::Array,
                },
                ..
            })
            if path == new_path("foo")
        );
        assert_matches!(
            check(&["0=x", "foo=y"]),
            Err(CompileError::Semantic {
                source: SemanticError::InconsistentNodeKind {
                    path,
                    kind1: NodeKind::Array,
                    kind2: NodeKind::Object,
                },
                ..
            })
            if path == new_path(".")
        );
        assert_matches!(
            check(&["foo=x", "0=y"]),
            Err(CompileError::Semantic {
                source: SemanticError::InconsistentNodeKind {
                    kind1: NodeKind::Object,
                    kind2: NodeKind::Array,
                    ..
                },
                ..
            })
        );

        // explicit vs implicit structure conflicts
        assert_matches!(
            check(&[".={}", "a=x"]),
            Err(CompileError::Semantic {
                source: SemanticError::InconsistentNodeKind {
                    path,
                    kind1: NodeKind::Value,
                    kind2: NodeKind::Object,
                },
                ..
            })
            if path == new_path(".")
        );
        assert_matches!(
            check(&["a=x", ".={}"]),
            Err(CompileError::Semantic {
                source: SemanticError::InconsistentNodeKind {
                    path,
                    kind1: NodeKind::Object,
                    kind2: NodeKind::Value,
                },
                ..
            })
            if path == new_path(".")
        );
        assert_matches!(
            check(&[".=[]", "0=x"]),
            Err(CompileError::Semantic {
                source: SemanticError::InconsistentNodeKind {
                    path,
                    kind1: NodeKind::Value,
                    kind2: NodeKind::Array,
                },
                ..
            })
            if path == new_path(".")
        );
        assert_matches!(
            check(&["0=x", ".=[]"]),
            Err(CompileError::Semantic {
                source: SemanticError::InconsistentNodeKind {
                    path,
                    kind1: NodeKind::Array,
                    kind2: NodeKind::Value,
                },
                ..
            })
            if path == new_path(".")
        );

        // conflicting explicit definitions
        assert_matches!(
            check(&[".=x", ".=y"]),
            Err(CompileError::Semantic {
                source: SemanticError::CollidingAssignments { path },
                ..
            })
            if path == new_path(".")
        );
        assert_matches!(
            check(&["foo=x", "foo=y"]),
            Err(CompileError::Semantic {
                source: SemanticError::CollidingAssignments { path },
                ..
            })
            if path == new_path("foo")
        );
        assert_matches!(
            check(&["0=x", "0=y"]),
            Err(CompileError::Semantic {
                source: SemanticError::CollidingAssignments { path },
                ..
            })
            if path == new_path("0")
        );

        // array structure errors
        assert_matches!(
            check(&["foo.2=x"]),
            Err(CompileError::Semantic {
                source: SemanticError::IncompleteArray {
                    path,
                    index_seen: 2,
                    index_missing: 0,
                },
                ..
            })
            if path == new_path("foo")
        );
        assert_matches!(
            check(&["foo.0=x", "foo.2=y"]),
            Err(CompileError::Semantic {
                source: SemanticError::IncompleteArray {
                    path,
                    index_seen: 2,
                    index_missing: 1,
                },
                ..
            })
            if path == new_path("foo")
        );
        assert_matches!(
            check(&["2=x"]),
            Err(CompileError::Semantic {
                source: SemanticError::IncompleteArray {
                    path,
                    index_seen: 2,
                    index_missing: 0,
                },
                ..
            })
            if path == new_path(".")
        );
    }
}
