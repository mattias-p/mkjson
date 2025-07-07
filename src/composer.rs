use crate::node::Node;
use crate::node::build_tree;
use crate::parser::SyntaxError;
use crate::parser::parse_directive;
use crate::validator::PathError;
use crate::validator::validate;
use snafu::prelude::*;
use std::str::Utf8Error;
use unicode_general_category::GeneralCategory;
use unicode_general_category::get_general_category;

#[derive(Debug, Snafu)]
pub enum BuildError {
    #[snafu(display("directive \"{directive}\": {source}"))]
    Encoding {
        source: Utf8Error,
        directive: String,
    },

    #[snafu(display("directive \"{directive}\": {source}"))]
    Syntax {
        source: SyntaxError,
        directive: String,
    },

    #[snafu(display("validating: {source}"))]
    Path { source: PathError },
}

type BuildResult<T> = Result<T, BuildError>;

fn should_escape(c: char) -> bool {
    matches!(
        get_general_category(c),
        GeneralCategory::Control
            | GeneralCategory::Format
            | GeneralCategory::Surrogate
            | GeneralCategory::PrivateUse
            | GeneralCategory::Unassigned
    )
}

fn safe_bytes_display(bytes: &[u8]) -> String {
    bytes
        .into_iter()
        .cloned()
        .map(|b| match b {
            b'"' => r#"\""#.to_string(),
            b'\\' => r#"\\"#.to_string(),
            b'\x09' => r#"\t"#.to_string(),
            b'\x0a' => r#"\n"#.to_string(),
            b'\x0d' => r#"\r"#.to_string(),
            b'\x20'..=b'\x7e' => format!("{}", char::from(b)),
            _ => format!("\\x{:02x}", b),
        })
        .collect()
}

fn safe_unicode_display(chars: &str) -> String {
    chars
        .chars()
        .map(|c| {
            if should_escape(c) {
                if c <= '\u{ffff}' {
                    format!("\\u{:04X}", c as u32)
                } else {
                    format!("\\U{:08X}", c as u32)
                }
            } else {
                c.to_string()
            }
        })
        .collect()
}

pub fn compose<'a>(inputs: impl Iterator<Item = Vec<u8>>) -> BuildResult<Option<Node>> {
    let mut directives = vec![];
    for bytes in inputs {
        let text = str::from_utf8(&bytes).context(EncodingSnafu {
            directive: safe_bytes_display(&bytes),
        })?;
        let (ast, _, _) = parse_directive(1, text).context(SyntaxSnafu {
            directive: safe_unicode_display(text),
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
        let directives = directives.into_iter().map(|s| s.bytes().collect());
        compose(directives).map(|tree| tree.map(|node| node.to_string()))
    }

    mod syntax {
        use super::*;

        mod array_index {
            use super::*;

            #[test]
            fn accept_index_segment() {
                expect_json!(["0:42"], "[42]");
            }

            #[test]
            fn reject_index_segment_with_leading_zeros() {
                expect_syntax_error!(["00=x"], UnexpectedChar { pos: 2, ch: '0' });
                expect_syntax_error!(["01=x"], UnexpectedChar { pos: 2, ch: '1' });
            }
        }

        mod bare_key {
            use super::*;

            #[test]
            fn accept_bare_key_segment() {
                expect_json!(["foo:42"], r#"{"foo":42}"#);
                expect_json!(["вишиванка:42"], r#"{"вишиванка":42}"#);
            }

            #[test]
            fn reject_bare_key_segment_with_special_characters() {
                expect_syntax_error!(["foo/bar:42"], UnexpectedChar { pos: 4, ch: '/' });
                expect_syntax_error!([" foobar=true"], UnexpectedChar { pos: 1, ch: ' ' });
                expect_syntax_error!(["foo bar:true"], UnexpectedChar { pos: 4, ch: ' ' });
                expect_syntax_error!(["foobar :true"], UnexpectedChar { pos: 7, ch: ' ' });
            }

            #[test]
            fn show_escaped_control_character_in_error_message() {
                assert_matches!(
                    check(&["foo.\u{0010}=x"]),
                    Err(BuildError::Syntax {
                        source: SyntaxError::UnexpectedChar {
                            pos: 5,
                            ch: '\u{0010}'
                        },
                        directive,
                    })
                    if directive == "foo.\\u0010=x"
                );
            }
        }

        mod quoted_key {
            use super::*;

            #[test]
            fn accept_quoted_key_segment() {
                expect_json!([r#""foo":42"#], r#"{"foo":42}"#);
                expect_syntax_error!(["\"unterminated"], UnexpectedEndOfString);
                expect_json!([r#""😀":42"#], r#"{"😀":42}"#);
                expect_json!([r#""foo.bar":42"#], r#"{"foo.bar":42}"#);
                expect_json!([r#""foo:bar":42"#], r#"{"foo:bar":42}"#);
                expect_json!([r#""":42"#], r#"{"":42}"#);
                expect_json!([r#"" foo bar ":42"#], r#"{" foo bar ":42}"#);
            }

            #[test]
            fn accept_two_character_escapes() {
                expect_json!([r#""\b\f\n\r\t\/\\\"":42"#], r#"{"\b\f\n\r\t\/\\\"":42}"#);
            }

            #[test]
            fn accept_six_character_escape() {
                expect_json!([r#""\u2600":42"#], r#"{"\u2600":42}"#);
            }

            #[test]
            fn reject_unescaped_control_character() {
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
            fn accept_root_path() {
                expect_json!([".:42"], "42");
            }

            #[test]
            fn accept_single_segment_paths() {
                expect_json!(["foo:42"], r#"{"foo":42}"#);
                expect_json!(["0:42"], "[42]");
            }

            #[test]
            fn accept_nested_paths() {
                expect_json!(["foo.bar.baz:42"], r#"{"foo":{"bar":{"baz":42}}}"#);
                expect_json!(["0.0:42"], "[[42]]");
                expect_json!(["0.0.0:42"], "[[[42]]]");
                expect_json!(["foo.0:42"], r#"{"foo":[42]}"#);
                expect_json!(["0.foo:42"], r#"[{"foo":42}]"#);
            }

            #[test]
            fn reject_paths_with_unquoted_empty_segments() {
                expect_syntax_error!([":42"], UnexpectedChar { pos: 1, ch: ':' });
                expect_syntax_error!([".foo:42"], UnexpectedChar { pos: 2, ch: 'f' });
                expect_syntax_error!(["foo.:42"], UnexpectedChar { pos: 5, ch: ':' });
                expect_syntax_error!(["foo..bar:42"], UnexpectedChar { pos: 5, ch: '.' });
            }
        }

        mod values {
            use super::*;

            #[test]
            fn accept_null() {
                expect_json!([".:null"], "null");
            }

            #[test]
            fn accept_true() {
                expect_json!([".:true"], "true");
            }

            #[test]
            fn accept_false() {
                expect_json!([".:false"], "false");
            }

            mod numbers {
                use super::*;

                #[test]
                fn accept_zero_and_preserve_sign() {
                    expect_json!([".:0"], "0");
                    expect_json!([".:-0"], "-0");
                }

                #[test]
                fn accept_fractions() {
                    expect_json!([".:1.1"], "1.1");
                }

                #[test]
                fn accept_and_preserve_scientific_notation() {
                    expect_json!([".:6.02e23"], "6.02e23");
                }

                #[test]
                fn reject_nan() {
                    expect_syntax_error!([".:NaN"], InvalidJsonValue { pos: 3, .. });
                }

                #[test]
                fn reject_infinity() {
                    expect_syntax_error!([".:Infinity"], InvalidJsonValue { pos: 3, .. });
                }

                #[test]
                fn reject_hexadecimal_notation() {
                    expect_syntax_error!([".:0xFF"], InvalidJsonValue { pos: 3, .. });
                }

                #[test]
                fn reject_trailing_garbage_after_value() {
                    expect_syntax_error!([".:null,"], UnexpectedChar { pos: 7, ch: ',' });
                    expect_syntax_error!([".:null ,"], UnexpectedChar { pos: 8, ch: ',' });
                    expect_syntax_error!([".:true,"], UnexpectedChar { pos: 7, ch: ',' });
                    expect_syntax_error!([".:true ,"], UnexpectedChar { pos: 8, ch: ',' });
                    expect_syntax_error!([".:false,"], UnexpectedChar { pos: 8, ch: ',' });
                    expect_syntax_error!([".:false ,"], UnexpectedChar { pos: 9, ch: ',' });
                    expect_syntax_error!([".:42,"], UnexpectedChar { pos: 5, ch: ',' });
                    expect_syntax_error!([".:42 ,"], UnexpectedChar { pos: 6, ch: ',' });
                    expect_syntax_error!([r#".:"x","#], UnexpectedChar { pos: 6, ch: ',' });
                    expect_syntax_error!([r#".:"x" ,"#], UnexpectedChar { pos: 7, ch: ',' });
                    expect_syntax_error!([".:[],"], UnexpectedChar { pos: 5, ch: ',' });
                    expect_syntax_error!([".:[] ,"], UnexpectedChar { pos: 6, ch: ',' });
                    expect_syntax_error!([".:{},"], UnexpectedChar { pos: 5, ch: ',' });
                    expect_syntax_error!([".:{} ,"], UnexpectedChar { pos: 6, ch: ',' });
                }
            }

            mod typed_strings {
                use super::*;

                #[test]
                fn accept_unescaped_string() {
                    expect_json!([r#".:"""#], r#""""#);
                    expect_json!([r#".:"foo""#], r#""foo""#);
                    expect_json!([r#".:"😀""#], r#""😀""#);
                }

                #[test]
                fn accept_and_preserve_unicode_escape() {
                    expect_json!([r#".:"\u0000""#], r#""\u0000""#);
                    expect_json!([r#".:"\u0041""#], r#""\u0041""#);
                    expect_json!([r#".:"\u007f""#], r#""\u007f""#);
                    expect_json!([r#".:"\u2600""#], r#""\u2600""#);
                }

                #[test]
                fn accept_and_preserve_escaped_quotation_mark() {
                    expect_json!([r#".:"\"""#], r#""\"""#);
                }

                #[test]
                fn accept_and_preserve_escaped_reverse_solidus() {
                    expect_json!([r#".:"\\""#], r#""\\""#);
                }

                #[test]
                fn accept_and_preserve_escaped_and_unescaped_solidus() {
                    expect_json!([r#".:"\/""#], r#""\/""#);
                    expect_json!([r#".:"/""#], r#""/""#);
                }

                #[test]
                fn accept_and_preserve_escaped_backspace() {
                    expect_json!([r#".:"\b""#], r#""\b""#);
                }

                #[test]
                fn accept_and_preserve_escaped_form_feed() {
                    expect_json!([r#".:"\f""#], r#""\f""#);
                }

                #[test]
                fn accept_and_preserve_escaped_line_feed() {
                    expect_json!([r#".:"\n""#], r#""\n""#);
                }

                #[test]
                fn accept_and_preserve_escaped_carriage_return() {
                    expect_json!([r#".:"\r""#], r#""\r""#);
                }

                #[test]
                fn accept_and_preserve_escaped_tab() {
                    expect_json!([r#".:"\t""#], r#""\t""#);
                }

                #[test]
                fn accept_and_preserve_unescaped_del() {
                    // DEL (U+007F) is not a control character per RFC 8259.
                    expect_json!([".:\"\x7f\""], "\"\x7f\"");
                }
            }

            mod string_assignment_operator {
                use super::*;

                #[test]
                fn accept_string_assignment() {
                    expect_json!([".="], r#""""#);
                    expect_json!([".=😀"], r#""😀""#);
                    expect_json!([r#".=foo:bar"#], r#""foo:bar""#);
                }

                #[test]
                fn escape_quotation_mark_to_preserve_it() {
                    expect_json!([r#".=""#], r#""\"""#);
                }

                #[test]
                fn escape_reverse_solidus_to_preserve_it() {
                    expect_json!([r#".=\"#], r#""\\""#);
                }

                #[test]
                fn avoid_escaping_solidus_even_though_escape_sequence_exists() {
                    expect_json!([r#".=/"#], r#""/""#);
                }

                #[test]
                fn escape_control_character_to_preserve_it() {
                    expect_json!([".=\x00"], r#""\u0000""#);
                    expect_json!([".=\x01"], r#""\u0001""#);
                    expect_json!([".=\x02"], r#""\u0002""#);
                    expect_json!([".=\x03"], r#""\u0003""#);
                    expect_json!([".=\x04"], r#""\u0004""#);
                    expect_json!([".=\x05"], r#""\u0005""#);
                    expect_json!([".=\x06"], r#""\u0006""#);
                    expect_json!([".=\x07"], r#""\u0007""#);
                    expect_json!([".=\x08"], r#""\b""#); // backspace
                    expect_json!([".=\x09"], r#""\t""#); // tab
                    expect_json!([".=\x0a"], r#""\n""#); // line feed
                    expect_json!([".=\x0b"], r#""\u000b""#);
                    expect_json!([".=\x0c"], r#""\f""#); // form feed
                    expect_json!([".=\x0d"], r#""\r""#); // carriage return
                    expect_json!([".=\x0e"], r#""\u000e""#);
                    expect_json!([".=\x0f"], r#""\u000f""#);
                    expect_json!([".=\x10"], r#""\u0010""#);
                    expect_json!([".=\x11"], r#""\u0011""#);
                    expect_json!([".=\x12"], r#""\u0012""#);
                    expect_json!([".=\x13"], r#""\u0013""#);
                    expect_json!([".=\x14"], r#""\u0014""#);
                    expect_json!([".=\x15"], r#""\u0015""#);
                    expect_json!([".=\x16"], r#""\u0016""#);
                    expect_json!([".=\x17"], r#""\u0017""#);
                    expect_json!([".=\x1b"], r#""\u001b""#);
                    expect_json!([".=\x1d"], r#""\u001d""#);
                    expect_json!([".=\x1e"], r#""\u001e""#);
                    expect_json!([".=\x1f"], r#""\u001f""#);
                }

                #[test]
                fn accept_and_preserve_del_character() {
                    // DEL (U+007F) is not a control character per RFC 8259.
                    expect_json!([".=\x7f"], "\"\x7f\"");
                }
            }

            #[test]
            fn accept_empty_object() {
                expect_json!([".:{}"], "{}");
            }

            #[test]
            fn reject_non_empty_object() {
                expect_syntax_error!([r#".:{"foo":42}"#], UnexpectedChar { pos: 4, ch: '"' });
            }

            #[test]
            fn accept_empty_array() {
                expect_json!([".:[]"], "[]");
            }

            #[test]
            fn reject_non_empty_array() {
                expect_syntax_error!([".:[42]"], UnexpectedChar { pos: 4, ch: '4' });
            }
        }

        mod directive {
            use super::*;

            #[test]
            fn reject_incomplete_directive() {
                expect_syntax_error!([""], UnexpectedEndOfString);
                expect_syntax_error!(["foo"], UnexpectedEndOfString);
            }
        }
    }

    mod semantics {
        use super::*;

        mod colliding_assignments {
            use super::*;

            #[test]
            fn reject_conflicting_root_assignments() {
                expect_path_error!([".:42", ".:43"], ".", ConflictingDirectives);
            }

            #[test]
            fn reject_duplicate_object_keys() {
                expect_path_error!(["a:42", "a:42"], "a", ConflictingDirectives);
                expect_path_error!(["a:42", r#""a":42"#], "a", ConflictingDirectives);
                expect_path_error!([r#""a":42"#, r#""a":42"#], "a", ConflictingDirectives);
            }

            #[test]
            fn reject_ambiguous_escape_encodings() {
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
            }

            #[test]
            fn accept_nfc_nfd_nfkc_nfkd_encodings_distinct_keys_per_rfc_8259() {
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
            fn reject_duplicate_array_indices() {
                expect_path_error!(["0:42", "0:43"], "0", ConflictingDirectives);
            }
        }

        mod type_conflicts {
            use super::*;

            #[test]
            fn reject_inconsistent_object_and_array_structures() {
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
        }

        mod array_completeness {
            use super::*;

            #[test]
            fn reject_arrays_with_missing_indices() {
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
            fn return_none_for_empty_directive_set() {
                assert_eq!(check(&[]).unwrap(), None);
            }

            #[test]
            fn merge_distinct_object_keys() {
                expect_json!(["foo:42", "bar:43"], r#"{"bar":43,"foo":42}"#);
                expect_json!(["0.foo:42", "0.bar:43"], r#"[{"bar":43,"foo":42}]"#);
                expect_json!(["a.foo:42", "a.bar:43"], r#"{"a":{"bar":43,"foo":42}}"#);
            }

            #[test]
            fn merge_complete_and_distinct_array_indices() {
                expect_json!(["0:42", "1:true"], r#"[42,true]"#);
                expect_json!(["1.0:42", "1.1:true", "0:{}"], r#"[{},[42,true]]"#);
            }
        }

        mod normalization {
            use super::*;

            #[test]
            #[ignore] // FIXME
            fn remove_unnecessary_whitespace_in_values() {
                expect_json!([".: \t\n\r{ \t\n\r} \t\n\r"], "{}");
                expect_json!([".: \t\n\r[ \t\n\r] \t\n\r"], "[]");
            }

            #[test]
            fn sort_object_keys_in_codepoint_order() {
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

        mod edge_cases {
            use super::*;

            mod number_precision {
                use super::*;

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
                fn test_beyond_ieee_754_double_precision_range() {
                    expect_json!([".:1e400"], "1e400");
                }

                #[test]
                fn preserve_trailing_zeros() {
                    expect_json!([".:1.00"], "1.00");
                }
            }

            mod unicode_surrogates {
                use super::*;

                // JSON syntax for escaping codepoints beyond the BMP (basic multilingual plane) is
                // based on surrogate pairs.
                #[test]
                fn accept_and_preserve_escaped_surrogate_pairs() {
                    expect_json!([r#".:"\ud83d\ude0a""#], r#""\ud83d\ude0a""#);
                }

                // JSON is based on UTF-8, and surrogate codepoints are illegal in UTF-8.
                #[test]
                fn reject_escaped_surrogate_pairs() {
                    expect_syntax_error!([r#".:"\ud83d.\ude0a""#], InvalidJsonValue { pos: 3, .. });
                }
            }
        }
    }

    // these are good candidates for howto guides, but deemed redundant in the context of unit
    // tests.
    mod howto {
        use super::*;

        #[test]
        fn numeric_key() {
            expect_json!([r#""0":42"#], r#"{"0":42}"#);
        }

        #[test]
        fn empty_key() {
            expect_json!([r#""":42"#], r#"{"":42}"#);
        }

        #[test]
        fn numeric_string() {
            expect_json!([r#".:"1""#], r#""1""#);
            expect_json!([".=1"], r#""1""#);
        }
    }
}
