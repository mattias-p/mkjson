use crate::node::Node;
use crate::node::build_tree;
use crate::parser::SyntaxError;
use crate::parser::parse_assignment;
use crate::validator::PathError;
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
    Path { source: PathError },
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

    validate(assignments.as_slice()).context(PathSnafu)?;

    Ok(build_tree(assignments.into_iter()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assignment::Path;
    use crate::parser::SyntaxError::*;
    use crate::parser::parse_path;
    use crate::validator::NodeKind;
    use crate::validator::PathErrorVariant::*;
    use assert_matches::assert_matches;
    use std::rc::Rc;

    macro_rules! expect_json {
        ($input:expr, $expected:expr) => {
            assert_eq!(check(&$input).unwrap(), Some($expected.into()));
        };
    }

    macro_rules! expect_syntax_error {
        ($input:expr, $expected:pat_param) => {
            assert_matches!(
                check(&$input),
                Err(CompileError::Syntax {
                    source: $expected,
                    ..
                })
            );
        };
    }

    macro_rules! expect_path_error {
        ($input:expr, $path:expr, $expected:pat_param) => {
            assert_matches!(
                check(&$input),
                Err(CompileError::Path {
                    source: PathError {
                        path,
                        variant: $expected,
                    },
                })
                if path == new_path($path)
            );
        };
    }

    fn new_path(s: &str) -> Rc<Path> {
        let (asts, _, _) = parse_path(1, s).unwrap();
        asts.into_iter().map(|ast| ast.into()).collect()
    }

    fn check(input: &[&str]) -> CompileResult<Option<String>> {
        let input = input.into_iter().map(|s| s.to_string());
        compile(input).map(|tree| tree.map(|node| node.to_string()))
    }

    mod syntax {
        use super::*;

        mod segment {
            use super::*;

            #[test]
            fn test_index() {
                expect_json!(["0:42"], "[42]");
                expect_syntax_error!(["00=x"], UnexpectedCharacter { pos: 2, ch: '0' });
                expect_syntax_error!(["01=x"], UnexpectedCharacter { pos: 2, ch: '1' });
            }

            #[test]
            fn test_identifier_key() {
                expect_json!(["foo:42"], r#"{"foo":42}"#);
                expect_json!(["вишиванка:42"], r#"{"вишиванка":42}"#);
                expect_syntax_error!(["foo/bar:42"], UnexpectedCharacter { pos: 4, ch: '/' });
            }

            #[test]
            fn test_quoted_key() {
                expect_json!([r#""foo":42"#], r#"{"foo":42}"#);
                expect_syntax_error!(["\"unterminated"], UnexpectedEndOfString);
                expect_json!([r#""😀":42"#], r#"{"😀":42}"#);
                expect_json!([r#""foo.bar":42"#], r#"{"foo.bar":42}"#);
                expect_json!([r#""foo:bar":42"#], r#"{"foo:bar":42}"#);
            }

            #[test]
            fn test_numeric_key() {
                expect_json!([r#""0":42"#], r#"{"0":42}"#);
            }

            #[test]
            fn test_empty_key() {
                expect_json!([r#""":42"#], r#"{"":42}"#);
            }

            #[test]
            fn test_key_with_space() {
                expect_json!([r#"" foo bar ":42"#], r#"{" foo bar ":42}"#);
                expect_syntax_error!([" foobar=true"], UnexpectedCharacter { pos: 1, ch: ' ' });
                expect_syntax_error!(["foo bar:true"], UnexpectedCharacter { pos: 4, ch: ' ' });
                expect_syntax_error!(["foobar :true"], UnexpectedCharacter { pos: 7, ch: ' ' });
            }

            #[test]
            fn test_key_with_two_character_escapes() {
                expect_json!([r#""\b\f\n\r\t\/\\\"":42"#], r#"{"\b\f\n\r\t\/\\\"":42}"#);
            }

            #[test]
            fn test_key_with_six_character_escape() {
                expect_json!([r#""\u2600":42"#], r#"{"\u2600":42}"#);
            }

            #[test]
            fn test_escaped_control_character_in_error_message() {
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
            }
        }

        mod path {
            use super::*;

            #[test]
            fn test_root_assignment() {
                expect_json!([".:42"], "42");
            }

            #[test]
            fn test_object_keys() {
                expect_json!(["foo:42"], r#"{"foo":42}"#);
                expect_json!(["foo.bar.baz:42"], r#"{"foo":{"bar":{"baz":42}}}"#);
            }

            #[test]
            fn test_array_indices() {
                expect_json!(["0:42"], "[42]");
                expect_json!(["0.0:42"], "[[42]]");
                expect_json!(["0.0.0:42"], "[[[42]]]");
            }

            #[test]
            fn test_mixed_segments() {
                expect_json!(["foo.0:42"], r#"{"foo":[42]}"#);
                expect_json!(["0.foo:42"], r#"[{"foo":42}]"#);
            }

            #[test]
            fn test_empty_segment() {
                expect_syntax_error!([":42"], UnexpectedCharacter { pos: 1, ch: ':' });
                expect_syntax_error!([".foo:42"], UnexpectedCharacter { pos: 2, ch: 'f' });
                expect_syntax_error!(["foo.:42"], UnexpectedCharacter { pos: 5, ch: ':' });
                expect_syntax_error!(["foo..bar:42"], UnexpectedCharacter { pos: 5, ch: '.' });
            }
        }

        mod values {
            use super::*;

            #[test]
            fn test_null() {
                expect_json!([".:null"], "null");
            }

            #[test]
            fn test_true() {
                expect_json!([".:true"], "true");
            }

            #[test]
            fn test_false() {
                expect_json!([".:false"], "false");
            }

            mod numbers {
                use super::*;

                #[test]
                fn test_positive_zero() {
                    expect_json!([".:0"], "0");
                }

                #[test]
                fn test_negative_zero() {
                    expect_json!([".:-0"], "-0");
                }

                #[test]
                fn test_with_fraction() {
                    expect_json!([".:1.1"], "1.1");
                }

                #[test]
                fn test_with_scientific_notation() {
                    expect_json!([".:6.02e23"], "6.02e23");
                }

                #[test]
                fn test_just_within_precision_of_ieee_754_double_precision() {
                    expect_json!([".:3.141592653589793116"], "3.141592653589793116");
                }

                #[test]
                fn test_beyond_precision_of_ieee_754_double_precision() {
                    expect_json!(
                        [".:3.141592653589793238462643383279"],
                        "3.141592653589793238462643383279"
                    );
                }

                #[test]
                fn test_just_within_range_of_ieee_754() {
                    expect_json!([".:1.7976931348623157e308"], "1.7976931348623157e308");
                }

                // 2^128
                #[test]
                fn test_beyond_precision_of_128_bit_integer() {
                    expect_json!(
                        [".:340282366920938463463374607431768211456"],
                        "340282366920938463463374607431768211456"
                    );
                }

                #[test]
                #[ignore]
                fn test_beyond_ieee_754_double_precision_range() {
                    expect_json!([".:1e400"], "1e400");
                }

                #[test]
                fn test_trailing_zeros() {
                    expect_json!([".:1.00"], "1.00");
                }

                #[test]
                fn test_nan() {
                    expect_syntax_error!([".:NaN"], InvalidJsonValue { pos: 3, .. });
                }

                #[test]
                fn test_infinity() {
                    expect_syntax_error!([".:Infinity"], InvalidJsonValue { pos: 3, .. });
                }

                #[test]
                fn test_hex() {
                    expect_syntax_error!([".:0xFF"], InvalidJsonValue { pos: 3, .. });
                }

                #[test]
                fn test_trailing_garbage() {
                    expect_syntax_error!([".:42,"], UnexpectedCharacter { pos: 5, ch: ',' });
                }
            }

            mod strings {
                use super::*;

                #[test]
                fn test_empty() {
                    expect_json!([r#".:"""#], r#""""#);
                    expect_json!([".="], r#""""#);
                }

                #[test]
                fn test_numeric() {
                    expect_json!([r#".:"1""#], r#""1""#);
                    expect_json!([".=1"], r#""1""#);
                }

                #[test]
                fn test_equals_operator() {
                    expect_json!([r#".=foo:bar"#], r#""foo:bar""#);
                    expect_json!([r#".="quoted""#], r#""\"quoted\"""#);
                }

                #[test]
                fn test_basic_multilingual_plane() {
                    expect_json!([r#".:"\u2600""#], r#""\u2600""#);

                    // U+2600
                    expect_json!([r#".:"☀""#], r#""☀""#);

                    // U+2600
                    expect_json!([r#".=☀"#], r#""☀""#);
                }

                mod two_character_escapes {
                    use super::*;

                    #[test]
                    fn test_quotation_mark() {
                        expect_json!([r#".:"\"""#], r#""\"""#);
                        expect_json!([r#".=""#], r#""\"""#);
                    }

                    #[test]
                    fn test_reverse_solidus() {
                        expect_json!([r#".:"\\""#], r#""\\""#);
                        expect_json!([r#".=\"#], r#""\\""#);
                    }

                    #[test]
                    fn test_solidus() {
                        expect_json!([r#".:"/""#], r#""/""#);
                        expect_json!([r#".:"\/""#], r#""\/""#);
                        expect_json!([r#".=/"#], r#""/""#);
                    }

                    #[test]
                    #[ignore] // FIXME
                    fn test_backspace() {
                        expect_syntax_error!(
                            ["\"\x08\"=x"],
                            UnexpectedCharacter { pos: 2, ch: '\x08' }
                        );
                        expect_json!([r#".:"\b""#], r#""\b""#);
                        expect_json!([".=\x08"], r#""\b""#);
                    }

                    #[test]
                    #[ignore] // FIXME
                    fn test_form_feed() {
                        expect_syntax_error!(
                            ["\"\x0c\"=x"],
                            UnexpectedCharacter { pos: 2, ch: '\x0c' }
                        );
                        expect_json!([r#".:"\f""#], r#""\f""#);
                        expect_json!([".=\x0c"], r#""\f""#);
                    }

                    #[test]
                    #[ignore] // FIXME
                    fn test_line_feed() {
                        expect_syntax_error!(
                            ["\"\x0a\"=x"],
                            UnexpectedCharacter { pos: 2, ch: '\x0a' }
                        );
                        expect_json!([r#".:"\n""#], r#""\n""#);
                        expect_json!([".=\x0a"], r#""\n""#);
                    }

                    #[test]
                    #[ignore] // FIXME
                    fn test_carriage_return() {
                        expect_syntax_error!(
                            ["\"\x0d\"=x"],
                            UnexpectedCharacter { pos: 2, ch: '\x0d' }
                        );
                        expect_json!([r#".:"\r""#], r#""\r""#);
                        expect_json!([".=\x0d"], r#""\r""#);
                    }

                    #[test]
                    #[ignore] // FIXME
                    fn test_tab() {
                        expect_syntax_error!(
                            ["\"\x09\"=x"],
                            UnexpectedCharacter { pos: 2, ch: '\x09' }
                        );
                        expect_json!([r#".:"\t""#], r#""\t""#);
                        expect_json!([".=\x09"], r#""\t""#);
                    }
                }

                mod six_character_escapes {
                    use super::*;

                    #[test]
                    #[ignore] // FIXME
                    fn test_nul() {
                        expect_syntax_error!(
                            ["\"\x00\"=x"],
                            UnexpectedCharacter { pos: 2, ch: '\x00' }
                        );
                        expect_json!([r#".:"\u0000""#], r#""\u0000""#);
                        expect_json!([".=\x00"], r#""\u0000""#);
                    }

                    #[test]
                    #[ignore] // FIXME
                    fn test_etx() {
                        expect_syntax_error!(
                            ["\"\x04\"=x"],
                            UnexpectedCharacter { pos: 2, ch: '\x04' }
                        );
                        expect_json!([r#".:"\u0004""#], r#""\u0004""#);
                        expect_json!([".=\x04"], r#""\u0004""#);
                    }

                    #[test]
                    #[ignore] // FIXME
                    fn test_syn() {
                        expect_syntax_error!(
                            ["\"\x16\"=x"],
                            UnexpectedCharacter { pos: 2, ch: '\x16' }
                        );
                        expect_json!([r#".:"\u0016""#], r#""\u0016""#);
                        expect_json!([".=\x16"], r#""\u0016""#);
                    }

                    #[test]
                    fn test_del() {
                        // DEL (U+007F) is not a control character per RFC 8259.
                        expect_json!([r#".:"\u007f""#], r#""\u007f""#);
                        expect_json!([".:\"\x7f\""], "\"\x7f\"");
                        expect_json!([".=\x7f"], "\"\x7f\"");
                    }

                    // Surrogates are not legal Unicode values (since RFC 3629).
                    // We assume here that UTF-8 decoding rejects inputs containing surrogates, and
                    // so we skip testing such strings.
                    // However, JSON bases its syntax for escaping codepoints beyond the BMP on
                    // surrogate pairs.
                    #[test]
                    fn test_surrogate_pairs() {
                        expect_json!([r#".:"\ud83d\ude0a""#], r#""\ud83d\ude0a""#);
                        expect_syntax_error!(
                            [r#".:"\ud83d.\ude0a""#],
                            InvalidJsonValue { pos: 3, .. }
                        );
                        expect_json!([".:\"\u{1f60a}\""], "\"\u{1f60a}\"");
                        expect_json!([".=\u{1f60a}"], "\"\u{1f60a}\"");
                    }
                }
            }

            #[test]
            fn test_object() {
                expect_json!([".:{}"], "{}");
                expect_json!([r#".:{"foo":42}"#], r#"{"foo":42}"#);
            }

            #[test]
            fn test_array() {
                expect_json!([".:[]"], "[]");
                expect_json!([".:[42]"], "[42]");
            }

            #[test]
            fn test_invalid_json_value() {
                expect_syntax_error!([".:hello"], InvalidJsonValue { pos: 3, .. });
                expect_syntax_error!([".:[1,2,]"], InvalidJsonValue { pos: 3, .. });
                expect_syntax_error!([".:{foo=42}"], InvalidJsonValue { pos: 3, .. });
                expect_syntax_error!(["\"unterminated"], UnexpectedEndOfString);
            }
        }

        mod assignment {
            use super::*;

            #[test]
            fn test_operators() {
                expect_json!([".=42"], r#""42""#);
                expect_json!([".:42"], "42");
                expect_json!(["x=42"], r#"{"x":"42"}"#);
                expect_json!(["x:42"], r#"{"x":42}"#);
                expect_json!(["0=42"], r#"["42"]"#);
                expect_json!(["0:42"], "[42]");
            }

            #[test]
            fn test_incomplete_expression() {
                expect_syntax_error!([""], UnexpectedEndOfString);
                expect_syntax_error!(["foo"], UnexpectedEndOfString);
            }
        }
    }

    mod semantics {
        use super::*;

        mod validation {
            use super::*;

            #[test]
            fn test_colliding_root_assignment() {
                expect_path_error!([".:42", ".:43"], ".", CollidingAssignments);
            }

            #[test]
            fn test_objects() {
                expect_path_error!(["a:42", "a:42"], "a", CollidingAssignments);
                expect_path_error!(["a:42", r#""a":42"#], "a", CollidingAssignments);
            }

            #[test]
            fn test_path_escaping_consistency() {
                expect_path_error!(
                    ["a:42", r#""\u0061":42"#],
                    ".",
                    InconsistentKeyEscaping { .. } // FIXME: check the keys too
                );

                // LATIN SMALL LETTER A WITH DIAERESIS
                expect_json!(
                    [
                        "a\u{308}:42", // NFD and NFKD
                        "\u{e4}:42",   // NFC and NFKC
                    ],
                    "{\"a\u{308}\":42,\"\u{e4}\":42}"
                );

                // LATIN SMALL LIGATURE FI + COMBINING ACUTE ACCENT
                expect_json!(
                    [
                        "fi\u{301}:42",       // NFKC and NFKD
                        "\u{fb01}\u{301}:42", // NFC and NFD
                    ],
                    "{\"fi\u{301}\":42,\"\u{fb01}\u{301}\":42}"
                );
            }

            #[test]
            fn test_arrays() {
                expect_path_error!(["0:42", "0:43"], "0", CollidingAssignments);
            }

            #[test]
            fn test_inconsistent_structure() {
                expect_path_error!(
                    ["foo.0=x", "foo.bar=y"],
                    "foo",
                    InconsistentNodeKind {
                        kind1: NodeKind::Array,
                        kind2: NodeKind::Object,
                    }
                );
                expect_path_error!(
                    ["foo.bar=x", "foo.0=y"],
                    "foo",
                    InconsistentNodeKind {
                        kind1: NodeKind::Object,
                        kind2: NodeKind::Array,
                    }
                );
                expect_path_error!(
                    ["0=x", "foo=y"],
                    ".",
                    InconsistentNodeKind {
                        kind1: NodeKind::Array,
                        kind2: NodeKind::Object,
                    }
                );
                expect_path_error!(
                    ["foo=x", "0=y"],
                    ".",
                    InconsistentNodeKind {
                        kind1: NodeKind::Object,
                        kind2: NodeKind::Array,
                    }
                );

                expect_path_error!(
                    [".={}", "a=x"],
                    ".",
                    InconsistentNodeKind {
                        kind1: NodeKind::Value,
                        kind2: NodeKind::Object,
                    }
                );
                expect_path_error!(
                    ["a=x", ".={}"],
                    ".",
                    InconsistentNodeKind {
                        kind1: NodeKind::Object,
                        kind2: NodeKind::Value,
                    }
                );
                expect_path_error!(
                    [".=[]", "0=x"],
                    ".",
                    InconsistentNodeKind {
                        kind1: NodeKind::Value,
                        kind2: NodeKind::Array,
                    }
                );
                expect_path_error!(
                    ["0=x", ".=[]"],
                    ".",
                    InconsistentNodeKind {
                        kind1: NodeKind::Array,
                        kind2: NodeKind::Value,
                    }
                );
            }

            #[test]
            fn test_array_completeness() {
                expect_path_error!(
                    ["1=x"],
                    ".",
                    IncompleteArray {
                        index_seen: 1,
                        index_missing: 0,
                    }
                );
                expect_path_error!(
                    ["foo.2=x"],
                    "foo",
                    IncompleteArray {
                        index_seen: 2,
                        index_missing: 0,
                    }
                );
                expect_path_error!(
                    ["foo.0=x", "foo.2=y"],
                    "foo",
                    IncompleteArray {
                        index_seen: 2,
                        index_missing: 1,
                    }
                );
                expect_path_error!(
                    ["2=x"],
                    ".",
                    IncompleteArray {
                        index_seen: 2,
                        index_missing: 0,
                    }
                );
            }
        }

        mod merging {
            use super::*;

            #[test]
            fn test_empty_expression_set() {
                assert_eq!(check(&[]).unwrap(), None);
            }

            #[test]
            fn test_objects() {
                expect_json!(["foo:42", "bar:43"], r#"{"bar":43,"foo":42}"#);

                expect_json!(["0.foo:42", "0.bar:43"], r#"[{"bar":43,"foo":42}]"#);

                expect_json!(["a.foo:42", "a.bar:43"], r#"{"a":{"bar":43,"foo":42}}"#);
            }

            #[test]
            fn test_arrays() {
                expect_json!(["0:42", "1:true"], r#"[42,true]"#);
                expect_json!(["1.0:42", "1.1:true", "0:{}"], r#"[{},[42,true]]"#);
            }
        }

        mod normalization {
            use super::*;

            #[test]
            #[ignore] // FIXME
            fn test_whitespace_characters() {
                expect_json!([".:[\x20\x09\x0a\x0d]"], "[]");
            }

            // JSON text allows for leading and trailing whitespace.
            #[test]
            #[ignore] // FIXME
            fn test_leading_and_trailing_whitespace() {
                expect_json!([".: 42"], "[]");
                expect_json!([".:42 "], "[]");
            }

            #[test]
            #[ignore] // FIXME
            fn test_inner_whitespace() {
                expect_json!([".:{ }"], "{}");
                expect_json!([r#".:{ "foo" : 42 }"#], r#"{"foo":42}"#);
                expect_json!([".:[ ]"], "{}");
                expect_json!([r#".:[ 42 , 42 ] }"#], r#"[42,42]"#);
            }

            #[test]
            #[ignore] // FIXME
            fn test_sort_object_key() {
                expect_json!(
                    [r#".:{"A":"1","B":"2","a":"3","é":4,"€":5}"#],
                    r#""{"A":"1","B":"2","a":"3","é":4,"€":5}""#
                );
                expect_json!(
                    [r#".:{"cat":1,"catalog":2,"car":3,"can":4}"#],
                    r#"{"can":4,"car":3,"cat":1,"catalog":2}"#
                );
                expect_json!(
                    [r#".:{"abc":1,"ab":2,"abcd":3}"#],
                    r#"{"ab":2,"abc":1,"abcd":3}"#
                );
                expect_json!(
                    [r#".:{"apple":1,"Ápple":2,"äpple":3,"banana":4}"#],
                    r#"{"apple":1,"banana":4,"Ápple":2,"äpple":3}"#
                );
                expect_json!(
                    [r#".:{"":1,"a":2,"A":3," ":4}"#],
                    r#"{"":1," ":4,"A":3,"a":2}"#
                );
            }

            #[test]
            fn test_preserve_array_order() {
                expect_json!([r#".:["","a","A"," "]"#], r#"["","a","A"," "]"#);
            }
        }
    }
}
