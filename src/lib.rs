pub mod assignment;
pub mod node;
pub mod parser;
pub mod validator;

use crate::assignment::Assignment;
use crate::node::Node;
use crate::node::build_tree;
use crate::parser::ParseError;
use crate::parser::parse_assignment;
use crate::validator::validate;

pub fn parse(input: &str) -> Result<Assignment, ParseError> {
    Ok(parse_assignment(input)?.0.into())
}

pub fn transform<'a>(inputs: impl Iterator<Item = String>) -> Result<Option<Node>, String> {
    let mut assignments = vec![];
    for text in inputs {
        let assignment =
            parse(&text).map_err(|e| format!("assignment \"{}\": {}", text.escape_default(), e))?;
        assignments.push(assignment);
    }

    validate(assignments.as_slice()).map_err(|e| format!("{}", e))?;

    Ok(build_tree(assignments.into_iter()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(input: &[&str]) -> Result<Option<String>, String> {
        let input = input.into_iter().map(|s| s.to_string());
        let json = transform(input)?.map(|node| node.to_string());
        Ok(json)
    }

    #[test]
    fn test() {
        assert_eq!(check(&[]), Ok(None), "valid empty builder expression set");
        assert_eq!(
            check(&[".=hello"]),
            Ok(Some(r#""hello""#.into())),
            "root JSON literal"
        );
        assert_eq!(check(&[".:true"]), Ok(Some(r#"true"#.into())), "root array");
        assert_eq!(check(&["0=x", "1=y"]), Ok(Some(r#"["x","y"]"#.into())));
        assert_eq!(
            check(&[r#"""="#]),
            Ok(Some(r#"{"":""}"#.into())),
            "empty key and empty value"
        );
        assert_eq!(
            check(&["0:null", "1:true", "2:false"]),
            Ok(Some(r#"[null,true,false]"#.into())),
            "root array from JSON literals"
        );

        // nested objects and arrays
        assert_eq!(
            check(&["foo.bar=x"]),
            Ok(Some(r#"{"foo":{"bar":"x"}}"#.into()))
        );
        assert_eq!(
            check(&["döner.kebab=x"]),
            Ok(Some(r#"{"döner":{"kebab":"x"}}"#.into()))
        );
        assert_eq!(
            check(&["foo.bar=x", "foo.baz=y"]),
            Ok(Some(r#"{"foo":{"bar":"x","baz":"y"}}"#.into()))
        );
        assert_eq!(
            check(&["foo.0.bar.0.baz=x"]),
            Ok(Some(r#"{"foo":[{"bar":[{"baz":"x"}]}]}"#.into()))
        );
        assert_eq!(check(&["0.bar=x"]), Ok(Some(r#"[{"bar":"x"}]"#.into())));
        assert_eq!(check(&["0.0=x"]), Ok(Some(r#"[["x"]]"#.into())));
        assert_eq!(check(&["foo.0=x"]), Ok(Some(r#"{"foo":["x"]}"#.into())));
        assert_eq!(
            check(&["foo.0=x", "foo.1=y"]),
            Ok(Some(r#"{"foo":["x","y"]}"#.into()))
        );
        assert_eq!(
            check(&["0.foo=x", "0.bar=y"]),
            Ok(Some(r#"[{"bar":"y","foo":"x"}]"#.into()))
        );
        assert_eq!(
            check(&["0.0=x", "0.1=y"]),
            Ok(Some(r#"[["x","y"]]"#.into()))
        );
        assert_eq!(check(&["emoji=😀"]), Ok(Some(r#"{"emoji":"😀"}"#.into())));
        assert_eq!(
            check(&["foo.bar.0:1", "foo.bar.1:2", "foo.bar.2:3"]),
            Ok(Some(r#"{"foo":{"bar":[1,2,3]}}"#.into()))
        );

        // empty collections
        assert_eq!(
            check(&[".:{}"]),
            Ok(Some(r#"{}"#.into())),
            "empty object at root"
        );
        assert_eq!(
            check(&[".:[]"]),
            Ok(Some(r#"[]"#.into())),
            "empty array at root"
        );

        // string literal assignments (=)
        assert_eq!(check(&[".=string"]), Ok(Some(r#""string""#.into())),);
        assert_eq!(
            check(&[r#".="quoted""#]),
            Ok(Some(r#""\"quoted\"""#.into())),
        );
        assert_eq!(check(&[".=1"]), Ok(Some(r#""1""#.into())),);
        assert_eq!(check(&[r#"""=1"#]), Ok(Some(r#"{"":"1"}"#.into())),);
        assert_eq!(check(&["foo=123"]), Ok(Some(r#"{"foo":"123"}"#.into())),);
        assert_eq!(
            check(&[r#""foo"=123"#]),
            Ok(Some(r#"{"foo":"123"}"#.into())),
        );
        assert_eq!(check(&[r#""0"=123"#]), Ok(Some(r#"{"0":"123"}"#.into())),);

        // raw JSON assignments (:)
        assert_eq!(check(&[".:[1,2,3]"]), Ok(Some(r#"[1,2,3]"#.into())),);
        assert_eq!(
            check(&[r#".:{"foo":"x"}"#]),
            Ok(Some(r#"{"foo":"x"}"#.into())),
        );
        assert_eq!(
            check(&[r#"foo:"123""#]),
            Ok(Some(r#"{"foo":"123"}"#.into())),
        );
        assert_eq!(check(&[r#"foo:123"#]), Ok(Some(r#"{"foo":123}"#.into())),);
        assert_eq!(
            check(&[r#"foo:[1,2,3]"#]),
            Ok(Some(r#"{"foo":[1,2,3]}"#.into())),
        );
        assert_eq!(
            check(&[r#"a.b.c:1"#]),
            Ok(Some(r#"{"a":{"b":{"c":1}}}"#.into())),
        );

        // array assignments
        assert_eq!(check(&["0=null"]), Ok(Some(r#"["null"]"#.into())),);
        assert_eq!(check(&["0:null"]), Ok(Some(r#"[null]"#.into())),);
        assert_eq!(
            check(&["0:null", "1:true"]),
            Ok(Some(r#"[null,true]"#.into())),
        );

        // quoted and escaped segments and values
        assert_eq!(
            check(&[r#""foo.bar"=baz"#]),
            Ok(Some(r#"{"foo.bar":"baz"}"#.into())),
            "quoted segment with dot"
        );

        // RFC 8259 JSON type coverage
        assert_eq!(check(&[".:null"]), Ok(Some("null".into())), "null literal");
        assert_eq!(check(&[".:true"]), Ok(Some("true".into())), "boolean true");
        assert_eq!(
            check(&[".:false"]),
            Ok(Some("false".into())),
            "boolean false"
        );
        assert_eq!(check(&[".:0"]), Ok(Some("0".into())), "zero number literal");
        assert_eq!(
            check(&[".:-1"]),
            Ok(Some("-1".into())),
            "negative number literal"
        );
        assert_eq!(
            check(&[".:-0"]),
            Ok(Some("-0".into())),
            "negative zero number literal"
        );
        assert_eq!(
            check(&[".:1.1"]),
            Ok(Some(r#"1.1"#.into())),
            "fractional number literal"
        );
        assert_eq!(
            check(&[".:1.00"]),
            Ok(Some(r#"1.00"#.into())),
            "fractional number literal with trailing zero decimals"
        );
        assert_eq!(
            check(&[".:3.141592653589793116"]),
            Ok(Some(r#"3.141592653589793116"#.into())),
            "closest approximation to π (IEEE 754 double precision)"
        );
        assert_eq!(
            check(&[".:3.141592653589793238462643383279"]),
            Ok(Some(r#"3.141592653589793238462643383279"#.into())),
            "approximation to π (beyond IEEE 754 double precision)"
        );
        assert_eq!(
            check(&[".:1.7976931348623157e308"]),
            Ok(Some(r#"1.7976931348623157e308"#.into())),
            "largest normal number (IEEE 754 double precision)"
        );
        assert_eq!(
            check(&[".:340282366920938463463374607431768211457"]),
            Ok(Some(r#"340282366920938463463374607431768211457"#.into())),
            "2^128+1"
        );

        if false {
            // TODO: fixme
            assert_eq!(
                check(&[".:1e400"]),
                Ok(Some(r#"1e400"#.into())),
                "very large normal number (beyond IEEE 754 double precision)"
            );
        }
        assert_eq!(
            check(&[r#".:"""#]),
            Ok(Some(r#""""#.into())),
            "string literal"
        );
        assert_eq!(
            check(&[r#".:"\u0041""#]),
            Ok(Some(r#""\u0041""#.into())),
            "string literal with Unicode escape"
        );
        assert_eq!(
            check(&[r#".:"\b\f\n\r\t""#]),
            Ok(Some(r#""\b\f\n\r\t""#.into())),
            "string literal with control character escapes"
        );
        assert_eq!(
            check(&[r#".:"\"\\""#]),
            Ok(Some(r#""\"\\""#.into())),
            "string literal with escape character escapes"
        );
        assert_eq!(
            check(&[r#".:"\ud83d\ude0a""#]),
            Ok(Some(r#""\ud83d\ude0a""#.into())),
            "string literal with escaped surrogate pair"
        );
        assert_eq!(
            check(&[r#".:"\u200b""#]),
            Ok(Some(r#""\u200b""#.into())),
            "string literal with escaped zero-width space"
        );
        assert_eq!(
            check(&[".:\"\u{200b}\""]),
            Ok(Some("\"\u{200b}\"".into())),
            "string literal with unescaped zero-width space"
        );
        assert_eq!(
            check(&[r#".:"abc\u203erev""#]),
            Ok(Some(r#""abc\u203erev""#.into())),
            "string literal with escaped bidi control character"
        );
        assert_eq!(
            check(&[".:\"abc\u{203e}rev\""]),
            Ok(Some("\"abc\u{203e}rev\"".into())),
            "string literal with unescaped bidi control character"
        );
        assert_eq!(
            check(&[r#".:"\u0001\u007f\u00a0\u2028""#]),
            Ok(Some(r#""\u0001\u007f\u00a0\u2028""#.into())),
            "string literal with mixed escaped unicode and control characters"
        );
        assert_eq!(check(&[".:{}"]), Ok(Some("{}".into())), "object literal");
        assert_eq!(check(&[".:[]"]), Ok(Some("[]".into())), "array literal");
    }
}
