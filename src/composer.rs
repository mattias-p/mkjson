use crate::node::Node;
use crate::node::build_tree;
use crate::parser::SyntaxError;
use crate::parser::parse_directive;
use crate::validator::PathError;
use crate::validator::validate;
use snafu::prelude::*;

#[derive(Debug, Snafu)]
pub enum BuildError {
    #[snafu(display("directive \"{directive}\": {source}"))]
    Syntax {
        source: SyntaxError,
        directive: String,
    },

    #[snafu(display("validating: {source}"))]
    Path { source: PathError },
}

type BuildResult<T> = Result<T, BuildError>;

pub fn compose<'a>(inputs: impl Iterator<Item = String>) -> BuildResult<Option<Node>> {
    let mut directives = vec![];
    for text in inputs {
        let (ast, _, _) = parse_directive(1, &text).context(SyntaxSnafu {
            directive: text.escape_default().to_string(),
        })?;
        directives.push(ast.into());
    }

    validate(directives.as_slice()).context(PathSnafu)?;

    Ok(build_tree(directives.into_iter()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::directive::Path;
    use crate::parser::SyntaxError::*;
    use crate::parser::parse_path;
    use crate::validator::NodeKind;
    use crate::validator::PathErrorVariant::*;
    use assert_matches::assert_matches;
    use std::rc::Rc;

    macro_rules! expect_json {
        ($directives:expr, $expected:expr) => {
            assert_eq!(check(&$directives).unwrap(), Some($expected.into()));
        };
    }

    macro_rules! expect_syntax_error {
        ($directives:expr, $expected:pat_param) => {
            assert_matches!(
                check(&$directives),
                Err(BuildError::Syntax {
                    source: $expected,
                    ..
                })
            );
        };
    }

    macro_rules! expect_path_error {
        ($directives:expr, $path:expr, $expected:pat_param) => {
            assert_matches!(
                check(&$directives),
                Err(BuildError::Path {
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

    fn check(directives: &[&str]) -> BuildResult<Option<String>> {
        let directives = directives.into_iter().map(|s| s.to_string());
        compose(directives).map(|tree| tree.map(|node| node.to_string()))
    }

    mod syntax {
        use super::*;

        mod segment {
            use super::*;

            #[test]
            fn test_index() {
                expect_json!(["0:42"], "[42]");
                expect_syntax_error!(["00=x"], UnexpectedChar { pos: 2, ch: '0' });
                expect_syntax_error!(["01=x"], UnexpectedChar { pos: 2, ch: '1' });
            }

            #[test]
            fn test_bare_key() {
                expect_json!(["foo:42"], r#"{"foo":42}"#);
                expect_json!(["вишиванка:42"], r#"{"вишиванка":42}"#);
                expect_syntax_error!(["foo/bar:42"], UnexpectedChar { pos: 4, ch: '/' });
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
                expect_syntax_error!([" foobar=true"], UnexpectedChar { pos: 1, ch: ' ' });
                expect_syntax_error!(["foo bar:true"], UnexpectedChar { pos: 4, ch: ' ' });
                expect_syntax_error!(["foobar :true"], UnexpectedChar { pos: 7, ch: ' ' });
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
                    Err(BuildError::Syntax {
                        source: SyntaxError::UnexpectedChar {
                            pos: 5,
                            ch: '\u{0010}'
                        },
                        directive,
                    })
                    if directive == "foo.\\u{10}=x"
                );
            }

            #[test]
            fn test_unescaped_control_character_in_quoted_segment() {
                expect_syntax_error!(["\"\x08\"=x"], UnexpectedChar { pos: 2, ch: '\x08' });
                expect_syntax_error!(["\"\x0c\"=x"], UnexpectedChar { pos: 2, ch: '\x0c' });
                expect_syntax_error!(["\"\x0a\"=x"], UnexpectedChar { pos: 2, ch: '\x0a' });
                expect_syntax_error!(["\"\x0d\"=x"], UnexpectedChar { pos: 2, ch: '\x0d' });
                expect_syntax_error!(["\"\x09\"=x"], UnexpectedChar { pos: 2, ch: '\x09' });
                expect_syntax_error!(["\"\x00\"=x"], UnexpectedChar { pos: 2, ch: '\x00' });
                expect_syntax_error!(["\"\x04\"=x"], UnexpectedChar { pos: 2, ch: '\x04' });
                expect_syntax_error!(["\"\x16\"=x"], UnexpectedChar { pos: 2, ch: '\x16' });
            }
        }

        mod path {
            use super::*;

            #[test]
            fn test_root_path() {
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
                expect_syntax_error!([":42"], UnexpectedChar { pos: 1, ch: ':' });
                expect_syntax_error!([".foo:42"], UnexpectedChar { pos: 2, ch: 'f' });
                expect_syntax_error!(["foo.:42"], UnexpectedChar { pos: 5, ch: ':' });
                expect_syntax_error!(["foo..bar:42"], UnexpectedChar { pos: 5, ch: '.' });
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
                    expect_syntax_error!([".:42,"], UnexpectedChar { pos: 5, ch: ',' });
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
                    fn test_backspace() {
                        expect_json!([r#".:"\b""#], r#""\b""#);
                        expect_json!([".=\x08"], r#""\b""#);
                    }

                    #[test]
                    fn test_form_feed() {
                        expect_json!([r#".:"\f""#], r#""\f""#);
                        expect_json!([".=\x0c"], r#""\f""#);
                    }

                    #[test]
                    fn test_line_feed() {
                        expect_json!([r#".:"\n""#], r#""\n""#);
                        expect_json!([".=\x0a"], r#""\n""#);
                    }

                    #[test]
                    fn test_carriage_return() {
                        expect_json!([r#".:"\r""#], r#""\r""#);
                        expect_json!([".=\x0d"], r#""\r""#);
                    }

                    #[test]
                    fn test_tab() {
                        expect_json!([r#".:"\t""#], r#""\t""#);
                        expect_json!([".=\x09"], r#""\t""#);
                    }
                }

                mod six_character_escapes {
                    use super::*;

                    #[test]
                    fn test_nul() {
                        expect_json!([r#".:"\u0000""#], r#""\u0000""#);
                        expect_json!([".=\x00"], r#""\u0000""#);
                    }

                    #[test]
                    fn test_etx() {
                        expect_json!([r#".:"\u0004""#], r#""\u0004""#);
                        expect_json!([".=\x04"], r#""\u0004""#);
                    }

                    #[test]
                    fn test_syn() {
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
                expect_syntax_error!([r#".:{"foo":42}"#], UnexpectedChar { pos: 4, ch: '"' });
                expect_syntax_error!([r#".:{ }"#], UnexpectedChar { pos: 4, ch: ' ' });
            }

            #[test]
            fn test_array() {
                expect_json!([".:[]"], "[]");
                expect_syntax_error!([".:[42]"], UnexpectedChar { pos: 4, ch: '4' });
                expect_syntax_error!([".:[ ]"], UnexpectedChar { pos: 4, ch: ' ' });
            }
        }

        mod directive {
            use super::*;

            #[test]
            fn test_assignment_operators() {
                expect_json!([".=42"], r#""42""#);
                expect_json!([".:42"], "42");
                expect_json!(["x=42"], r#"{"x":"42"}"#);
                expect_json!(["x:42"], r#"{"x":42}"#);
                expect_json!(["0=42"], r#"["42"]"#);
                expect_json!(["0:42"], "[42]");
            }

            #[test]
            fn test_incomplete_directive() {
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
                expect_path_error!([".:42", ".:43"], ".", ConflictingDirectives);
            }

            #[test]
            fn test_objects() {
                expect_path_error!(["a:42", "a:42"], "a", ConflictingDirectives);
                expect_path_error!(["a:42", r#""a":42"#], "a", ConflictingDirectives);
            }

            #[test]
            fn test_rfc_8259_string_comparison_should_be_respected() {
                expect_path_error!(
                    ["a:42", r#""\u0061":42"#],
                    ".",
                    InconsistentKeyEncodings { .. } // FIXME: check the encodings too
                );
                expect_path_error!(
                    [r#""\u006a":42"#, r#""\u006A":42"#],
                    ".",
                    InconsistentKeyEncodings { .. } // FIXME: check the encodings too
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
                expect_path_error!(["0:42", "0:43"], "0", ConflictingDirectives);
            }

            #[test]
            fn test_inconsistent_structure() {
                expect_path_error!(
                    ["foo.0=x", "foo.bar=y"],
                    "foo",
                    StructuralConflict {
                        kind1: NodeKind::Array,
                        kind2: NodeKind::Object,
                    }
                );
                expect_path_error!(
                    ["foo.bar=x", "foo.0=y"],
                    "foo",
                    StructuralConflict {
                        kind1: NodeKind::Object,
                        kind2: NodeKind::Array,
                    }
                );
                expect_path_error!(
                    ["0=x", "foo=y"],
                    ".",
                    StructuralConflict {
                        kind1: NodeKind::Array,
                        kind2: NodeKind::Object,
                    }
                );
                expect_path_error!(
                    ["foo=x", "0=y"],
                    ".",
                    StructuralConflict {
                        kind1: NodeKind::Object,
                        kind2: NodeKind::Array,
                    }
                );

                expect_path_error!(
                    [".={}", "a=x"],
                    ".",
                    StructuralConflict {
                        kind1: NodeKind::Value,
                        kind2: NodeKind::Object,
                    }
                );
                expect_path_error!(
                    ["a=x", ".={}"],
                    ".",
                    StructuralConflict {
                        kind1: NodeKind::Object,
                        kind2: NodeKind::Value,
                    }
                );
                expect_path_error!(
                    [".=[]", "0=x"],
                    ".",
                    StructuralConflict {
                        kind1: NodeKind::Value,
                        kind2: NodeKind::Array,
                    }
                );
                expect_path_error!(
                    ["0=x", ".=[]"],
                    ".",
                    StructuralConflict {
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
            fn test_empty_directive_set() {
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
                expect_syntax_error!([".:[\x20\x09\x0a\x0d]"], UnexpectedChar { pos: 4, ch: ' ' });
            }

            #[test]
            #[ignore] // FIXME
            fn test_leading_and_trailing_whitespace() {
                expect_syntax_error!([".:42 "], UnexpectedChar { pos: 5, ch: ' ' });
                expect_syntax_error!([".: 42"], UnexpectedChar { pos: 3, ch: ' ' });
            }

            #[test]
            fn test_object_key_sorting_unicode_order() {
                expect_json!(
                    [
                        r#""":1"#,  // empty string
                        r#"" ":2"#, // space
                        "A:3",      // capital Latin letter
                        "B:4",
                        "a:5", // lowercase Latin letter
                        "apple:6",
                        "banana:7",
                        r#""Zebra":8"#, // quoted capital word
                        "Ápple:9",      // Latin capital A with acute (U+00C1)
                        "äpple:10",     // Latin small a with diaeresis (U+00E4)
                        "é:11",         // Latin small e with acute (U+00E9)
                        r#""€":12"#     // Euro sign (U+20AC)
                    ],
                    r#"{"":1," ":2,"A":3,"B":4,"Zebra":8,"a":5,"apple":6,"banana":7,"Ápple":9,"äpple":10,"é":11,"€":12}"#
                );
            }
        }
    }
}
