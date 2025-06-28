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

    mod segment_syntax {
        use super::*;

        #[test]
        fn test_index() {
            assert_eq!(check(&["0:42"]).unwrap(), Some("[42]".into()));
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
        }

        #[test]
        fn test_identifier_key() {
            assert_eq!(check(&["foo:42"]).unwrap(), Some(r#"{"foo":42}"#.into()));
            assert_eq!(
                check(&["вишиванка:42"]).unwrap(),
                Some(r#"{"вишиванка":42}"#.into())
            );
            assert_matches!(
                check(&["foo/bar:42"]),
                Err(CompileError::Syntax {
                    source: SyntaxError::UnexpectedCharacter { pos: 4, ch: '/' },
                    ..
                })
            );
        }

        #[test]
        fn test_quoted_key() {
            assert_eq!(
                check(&[r#""foo":42"#]).unwrap(),
                Some(r#"{"foo":42}"#.into())
            );
            assert!(matches!(
                check(&["\"unterminated"]),
                Err(CompileError::Syntax {
                    source: SyntaxError::UnexpectedEndOfString,
                    ..
                })
            ));
            assert_eq!(check(&[r#""😀":42"#]).unwrap(), Some(r#"{"😀":42}"#.into()));
            assert_eq!(
                check(&[r#""foo.bar":42"#]).unwrap(),
                Some(r#"{"foo.bar":42}"#.into())
            );
        }

        #[test]
        fn test_numeric_key() {
            assert_eq!(check(&[r#""0":42"#]).unwrap(), Some(r#"{"0":42}"#.into()));
        }

        #[test]
        fn test_empty_key() {
            assert_eq!(check(&[r#""":42"#]).unwrap(), Some(r#"{"":42}"#.into()));
        }

        #[test]
        fn test_key_with_space() {
            assert_eq!(
                check(&[r#"" foo bar ":42"#]).unwrap(),
                Some(r#"{" foo bar ":42}"#.into())
            );
            assert_matches!(
                check(&[" foobar=true"]),
                Err(CompileError::Syntax {
                    source: SyntaxError::UnexpectedCharacter { pos: 1, ch: ' ' },
                    ..
                })
            );
            assert_matches!(
                check(&["foo bar:true"]),
                Err(CompileError::Syntax {
                    source: SyntaxError::UnexpectedCharacter { pos: 4, ch: ' ' },
                    ..
                })
            );
            assert_matches!(
                check(&["foobar :true"]),
                Err(CompileError::Syntax {
                    source: SyntaxError::UnexpectedCharacter { pos: 7, ch: ' ' },
                    ..
                })
            );
        }

        #[test]
        fn test_key_with_two_character_escapes() {
            assert_eq!(
                check(&[r#""\b\f\n\r\t\/\\\"":42"#]).unwrap(),
                Some(r#"{"\b\f\n\r\t\/\\\"":42}"#.into())
            );
        }

        #[test]
        fn test_key_with_six_character_escape() {
            assert_eq!(
                check(&[r#""\u2600":42"#]).unwrap(),
                Some(r#"{"\u2600":42}"#.into())
            );
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

    mod path_syntax {
        use super::*;

        #[test]
        fn test_root() {
            assert_eq!(check(&[".:42"]).unwrap(), Some("42".into()));
        }

        #[test]
        fn test_nested_keys() {
            assert_eq!(check(&["foo:42"]).unwrap(), Some(r#"{"foo":42}"#.into()));
            assert_eq!(
                check(&["foo.bar:42"]).unwrap(),
                Some(r#"{"foo":{"bar":42}}"#.into())
            );
            assert_eq!(
                check(&["foo.bar.baz:42"]).unwrap(),
                Some(r#"{"foo":{"bar":{"baz":42}}}"#.into())
            );
        }

        #[test]
        fn test_nested_indices() {
            assert_eq!(check(&["0:42"]).unwrap(), Some("[42]".into()));
            assert_eq!(check(&["0.0:42"]).unwrap(), Some("[[42]]".into()));
            assert_eq!(check(&["0.0.0:42"]).unwrap(), Some("[[[42]]]".into()));
        }

        #[test]
        fn test_nested_mixed_segments() {
            assert_eq!(
                check(&["foo.0:42"]).unwrap(),
                Some(r#"{"foo":[42]}"#.into())
            );
            assert_eq!(
                check(&["0.foo:42"]).unwrap(),
                Some(r#"[{"foo":42}]"#.into())
            );
        }

        #[test]
        fn test_empty_segment() {
            assert_matches!(
                check(&[":42"]),
                Err(CompileError::Syntax {
                    source: SyntaxError::UnexpectedCharacter { pos: 1, ch: ':' },
                    ..
                })
            );
            assert_matches!(
                check(&[".foo:42"]),
                Err(CompileError::Syntax {
                    source: SyntaxError::UnexpectedCharacter { pos: 2, ch: 'f' },
                    ..
                })
            );
            assert_matches!(
                check(&["foo.:42"]),
                Err(CompileError::Syntax {
                    source: SyntaxError::UnexpectedCharacter { pos: 5, ch: ':' },
                    ..
                })
            );
            assert_matches!(
                check(&["foo..bar:42"]),
                Err(CompileError::Syntax {
                    source: SyntaxError::UnexpectedCharacter { pos: 5, ch: '.' },
                    ..
                })
            );
        }
    }

    mod json_value_type_coverage {
        use super::*;

        #[test]
        fn test_null() {
            assert_eq!(check(&[".:null"]).unwrap(), Some("null".into()));
        }

        #[test]
        fn test_true() {
            assert_eq!(check(&[".:true"]).unwrap(), Some("true".into()));
        }

        #[test]
        fn test_false() {
            assert_eq!(check(&[".:false"]).unwrap(), Some("false".into()));
        }

        mod numbers {
            use super::*;

            #[test]
            fn test_positive_zero() {
                assert_eq!(check(&[".:0"]).unwrap(), Some("0".into()));
            }

            #[test]
            fn test_negative_zero() {
                assert_eq!(check(&[".:-0"]).unwrap(), Some("-0".into()));
            }

            #[test]
            fn test_with_fraction() {
                assert_eq!(check(&[".:1.1"]).unwrap(), Some("1.1".into()));
            }

            #[test]
            fn test_with_scientific_notation() {
                assert_eq!(check(&[".:6.02e23"]).unwrap(), Some("6.02e23".into()));
            }

            #[test]
            fn test_just_within_precision_of_ieee_754_double_precision() {
                assert_eq!(
                    check(&[".:3.141592653589793116"]).unwrap(),
                    Some("3.141592653589793116".into()),
                );
            }

            #[test]
            fn test_beyond_precision_of_ieee_754_double_precision() {
                assert_eq!(
                    check(&[".:3.141592653589793238462643383279"]).unwrap(),
                    Some("3.141592653589793238462643383279".into()),
                );
            }

            #[test]
            fn test_just_within_range_of_ieee_754() {
                assert_eq!(
                    check(&[".:1.7976931348623157e308"]).unwrap(),
                    Some("1.7976931348623157e308".into()),
                );
            }

            #[test]
            fn test_beyond_precision_of_128_bit_integer() {
                assert_eq!(
                    check(&[".:340282366920938463463374607431768211456"]).unwrap(),
                    Some("340282366920938463463374607431768211456".into()),
                    "2^128"
                );
            }

            #[test]
            #[ignore]
            fn test_beyond_ieee_754_double_precision_range() {
                assert_eq!(check(&[".:1e400"]).unwrap(), Some("1e400".into()));
            }

            #[test]
            fn test_trailing_zeros() {
                assert_eq!(check(&[".:1.00"]).unwrap(), Some("1.00".into()));
            }
        }

        mod strings {
            use super::*;

            #[test]
            fn test_empty() {
                assert_eq!(check(&[r#".:"""#]).unwrap(), Some(r#""""#.into()));
                assert_eq!(check(&[".="]).unwrap(), Some(r#""""#.into()));
            }

            #[test]
            fn test_numeric() {
                assert_eq!(check(&[r#".:"1""#]).unwrap(), Some(r#""1""#.into()));
                assert_eq!(check(&[".=1"]).unwrap(), Some(r#""1""#.into()));
            }

            #[test]
            fn test_quotes() {
                assert_eq!(
                    check(&[r#".="quoted""#]).unwrap(),
                    Some(r#""\"quoted\"""#.into())
                );
            }

            #[test]
            fn test_basic_multilingual_plane() {
                assert_eq!(
                    check(&[r#".:"\u2600""#]).unwrap(),
                    Some(r#""\u2600""#.into()),
                );

                // U+2600
                assert_eq!(check(&[r#".:"☀""#]).unwrap(), Some(r#""☀""#.into()));

                // U+2600
                assert_eq!(check(&[r#".=☀"#]).unwrap(), Some(r#""☀""#.into()));
            }

            mod two_character_escapes {
                use super::*;

                #[test]
                fn test_quotation_mark() {
                    assert_eq!(check(&[r#".:"\"""#]).unwrap(), Some(r#""\"""#.into()),);
                    assert_eq!(check(&[r#".=""#]).unwrap(), Some(r#""\"""#.into()),);
                }

                #[test]
                fn test_reverse_solidus() {
                    assert_eq!(check(&[r#".:"\\""#]).unwrap(), Some(r#""\\""#.into()),);
                    assert_eq!(check(&[r#".=\"#]).unwrap(), Some(r#""\\""#.into()),);
                }

                #[test]
                fn test_solidus() {
                    assert_eq!(check(&[r#".:"/""#]).unwrap(), Some(r#""/""#.into()),);
                    assert_eq!(check(&[r#".:"\/""#]).unwrap(), Some(r#""\/""#.into()),);
                    assert_eq!(check(&[r#".=/"#]).unwrap(), Some(r#""/""#.into()),);
                }

                #[test]
                #[ignore] // FIXME
                fn test_backspace() {
                    assert_matches!(
                        check(&["\"\x08\"=x"]),
                        Err(CompileError::Syntax {
                            source: SyntaxError::UnexpectedCharacter { pos: 2, ch: '\x08' },
                            ..
                        })
                    );
                    assert_eq!(check(&[r#".:"\b""#]).unwrap(), Some(r#""\b""#.into()),);
                    assert_eq!(check(&[".=\x08"]).unwrap(), Some(r#""\b""#.into()),);
                }

                #[test]
                #[ignore] // FIXME
                fn test_form_feed() {
                    assert_matches!(
                        check(&["\"\x0c\"=x"]),
                        Err(CompileError::Syntax {
                            source: SyntaxError::UnexpectedCharacter { pos: 2, ch: '\x0c' },
                            ..
                        })
                    );
                    assert_eq!(check(&[r#".:"\f""#]).unwrap(), Some(r#""\f""#.into()),);
                    assert_eq!(check(&[".=\x0c"]).unwrap(), Some(r#""\f""#.into()),);
                }

                #[test]
                #[ignore] // FIXME
                fn test_line_feed() {
                    assert_matches!(
                        check(&["\"\x0a\"=x"]),
                        Err(CompileError::Syntax {
                            source: SyntaxError::UnexpectedCharacter { pos: 2, ch: '\x0a' },
                            ..
                        })
                    );
                    assert_eq!(check(&[r#".:"\n""#]).unwrap(), Some(r#""\n""#.into()),);
                    assert_eq!(check(&[".=\x0a"]).unwrap(), Some(r#""\n""#.into()),);
                }

                #[test]
                #[ignore] // FIXME
                fn test_carriage_return() {
                    assert_matches!(
                        check(&["\"\x0d\"=x"]),
                        Err(CompileError::Syntax {
                            source: SyntaxError::UnexpectedCharacter { pos: 2, ch: '\x0d' },
                            ..
                        })
                    );
                    assert_eq!(check(&[r#".:"\r""#]).unwrap(), Some(r#""\r""#.into()),);
                    assert_eq!(check(&[".=\x0d"]).unwrap(), Some(r#""\r""#.into()),);
                }

                #[test]
                #[ignore] // FIXME
                fn test_tab() {
                    assert_matches!(
                        check(&["\"\x09\"=x"]),
                        Err(CompileError::Syntax {
                            source: SyntaxError::UnexpectedCharacter { pos: 2, ch: '\x09' },
                            ..
                        })
                    );
                    assert_eq!(check(&[r#".:"\t""#]).unwrap(), Some(r#""\t""#.into()),);
                    assert_eq!(check(&[".=\x09"]).unwrap(), Some(r#""\t""#.into()),);
                }
            }

            mod six_character_escapes {
                use super::*;

                #[test]
                #[ignore] // FIXME
                fn test_nul() {
                    assert_matches!(
                        check(&["\"\x00\"=x"]),
                        Err(CompileError::Syntax {
                            source: SyntaxError::UnexpectedCharacter { pos: 2, ch: '\x00' },
                            ..
                        })
                    );
                    assert_eq!(
                        check(&[r#".:"\u0000""#]).unwrap(),
                        Some(r#""\u0000""#.into()),
                    );
                    assert_eq!(check(&[".=\x00"]).unwrap(), Some(r#""\u0000""#.into()),);
                }

                #[test]
                #[ignore] // FIXME
                fn test_etx() {
                    assert_matches!(
                        check(&["\"\x04\"=x"]),
                        Err(CompileError::Syntax {
                            source: SyntaxError::UnexpectedCharacter { pos: 2, ch: '\x04' },
                            ..
                        })
                    );
                    assert_eq!(
                        check(&[r#".:"\u0004""#]).unwrap(),
                        Some(r#""\u0004""#.into()),
                    );
                    assert_eq!(check(&[".=\x04"]).unwrap(), Some(r#""\u0004""#.into()));
                }

                #[test]
                #[ignore] // FIXME
                fn test_syn() {
                    assert_matches!(
                        check(&["\"\x16\"=x"]),
                        Err(CompileError::Syntax {
                            source: SyntaxError::UnexpectedCharacter { pos: 2, ch: '\x16' },
                            ..
                        })
                    );
                    assert_eq!(
                        check(&[r#".:"\u0016""#]).unwrap(),
                        Some(r#""\u0016""#.into()),
                    );
                    assert_eq!(check(&[".=\x16"]).unwrap(), Some(r#""\u0016""#.into()));
                }

                // DEL is not considered a control character by RFC 8259.
                #[test]
                fn test_del() {
                    assert_eq!(
                        check(&[r#".:"\u007f""#]).unwrap(),
                        Some(r#""\u007f""#.into())
                    );
                    assert_eq!(check(&[".:\"\x7f\""]).unwrap(), Some("\"\x7f\"".into()));
                    assert_eq!(check(&[".=\x7f"]).unwrap(), Some("\"\x7f\"".into()));
                }

                // Surrogates are not legal Unicode values (since RFC 3629).
                // We assume here that UTF-8 decoding rejects inputs containing surrogates, and
                // so we skip testing such strings.
                // However, JSON bases its syntax for escaping codepoints beyond the BMP on
                // surrogate pairs.
                #[test]
                fn test_surrogate_pairs() {
                    assert_eq!(
                        check(&[r#".:"\ud83d\ude0a""#]).unwrap(),
                        Some(r#""\ud83d\ude0a""#.into())
                    );
                    assert_matches!(
                        check(&[r#".:"\ud83d.\ude0a""#]),
                        Err(CompileError::Syntax {
                            source: SyntaxError::InvalidJsonValue { pos: 3, .. },
                            ..
                        })
                    );
                    assert_eq!(
                        check(&[".:\"\u{1f60a}\""]).unwrap(),
                        Some("\"\u{1f60a}\"".into())
                    );
                    assert_eq!(
                        check(&[".=\u{1f60a}"]).unwrap(),
                        Some("\"\u{1f60a}\"".into())
                    );
                }
            }
        }

        #[test]
        fn test_object() {
            assert_eq!(check(&[".:{}"]).unwrap(), Some("{}".into()));
            assert_eq!(
                check(&[r#".:{"foo":42}"#]).unwrap(),
                Some(r#"{"foo":42}"#.into())
            );
        }

        #[test]
        fn test_array() {
            assert_eq!(check(&[".:[]"]).unwrap(), Some("[]".into()));
            assert_eq!(check(&[".:[42]"]).unwrap(), Some("[42]".into()));
        }

        #[test]
        fn test_invalid_json_value() {
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
                check(&[".:{foo=42}"]),
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
        }
    }

    mod json_value_normalization {
        use super::*;

        #[test]
        #[ignore] // FIXME
        fn test_whitespace_characters() {
            assert_eq!(check(&[".:[\x20\x09\x0a\x0d]"]).unwrap(), Some("[]".into()));
        }

        // JSON text allows for leading and trailing whitespace.
        #[test]
        #[ignore] // FIXME
        fn test_leading_and_trailing_whitespace() {
            assert_eq!(check(&[".: 42"]).unwrap(), Some("[]".into()));
            assert_eq!(check(&[".:42 "]).unwrap(), Some("[]".into()));
        }

        #[test]
        #[ignore] // FIXME
        fn test_inner_whitespace() {
            assert_eq!(check(&[".:{ }"]).unwrap(), Some("{}".into()));
            assert_eq!(
                check(&[r#".:{ "foo" : 42 }"#]).unwrap(),
                Some(r#"{"foo":42}"#.into())
            );
            assert_eq!(check(&[".:[ ]"]).unwrap(), Some("{}".into()));
            assert_eq!(
                check(&[r#".:[ 42 , 42 ] }"#]).unwrap(),
                Some(r#"[42,42]"#.into())
            );
        }

        #[test]
        #[ignore] // FIXME
        fn test_sort_object_key() {
            assert_eq!(
                check(&[r#".:{"A":"1","B":"2","a":"3","é":4,"€":5}"#]).unwrap(),
                Some(r#""{"A":"1","B":"2","a":"3","é":4,"€":5}""#.into())
            );
            assert_eq!(
                check(&[r#".:{"cat":1,"catalog":2,"car":3,"can":4}"#]).unwrap(),
                Some(r#"{"can":4,"car":3,"cat":1,"catalog":2}"#.into())
            );
            assert_eq!(
                check(&[r#".:{"abc":1,"ab":2,"abcd":3}"#]).unwrap(),
                Some(r#"{"ab":2,"abc":1,"abcd":3}"#.into())
            );
            assert_eq!(
                check(&[r#".:{"apple":1,"Ápple":2,"äpple":3,"banana":4}"#]).unwrap(),
                Some(r#"{"apple":1,"banana":4,"Ápple":2,"äpple":3}"#.into())
            );
            assert_eq!(
                check(&[r#".:{"":1,"a":2,"A":3," ":4}"#]).unwrap(),
                Some(r#"{"":1," ":4,"A":3,"a":2}"#.into())
            );
        }

        #[test]
        fn test_preserve_array_order() {
            assert_eq!(
                check(&[r#".:["","a","A"," "]"#]).unwrap(),
                Some(r#"["","a","A"," "]"#.into())
            );
        }
    }

    #[test]
    fn test_operators() {
        assert_eq!(check(&[".=42"]).unwrap(), Some(r#""42""#.into()));
        assert_eq!(check(&[".:42"]).unwrap(), Some("42".into()));
        assert_eq!(check(&["x=42"]).unwrap(), Some(r#"{"x":"42"}"#.into()));
        assert_eq!(check(&["x:42"]).unwrap(), Some(r#"{"x":42}"#.into()));
        assert_eq!(check(&["0=42"]).unwrap(), Some(r#"["42"]"#.into()));
        assert_eq!(check(&["0:42"]).unwrap(), Some("[42]".into()));
    }

    #[test]
    fn test_incomplete_expression() {
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
    }

    mod merging {
        use super::*;

        #[test]
        fn test_empty_expression_set() {
            assert_eq!(check(&[]).unwrap(), None);
        }

        #[test]
        fn test_colliding_root_assignment() {
            assert_matches!(
                check(&[".:42", ".:43"]),
                Err(CompileError::Semantic {
                    source: SemanticError::CollidingAssignments { path },
                    ..
                })
                if path == new_path(".")
            );
        }

        #[test]
        fn test_objects() {
            assert_eq!(
                check(&["foo:42", "bar:43"]).unwrap(),
                Some(r#"{"bar":43,"foo":42}"#.into())
            );

            assert_eq!(
                check(&["0.foo:42", "0.bar:43"]).unwrap(),
                Some(r#"[{"bar":43,"foo":42}]"#.into())
            );

            assert_eq!(
                check(&["a.foo:42", "a.bar:43"]).unwrap(),
                Some(r#"{"a":{"bar":43,"foo":42}}"#.into())
            );

            assert_matches!(
                check(&["foo=x", "foo=y"]),
                Err(CompileError::Semantic {
                    source: SemanticError::CollidingAssignments { path },
                    ..
                })
                if path == new_path("foo")
            );
        }

        #[test]
        fn test_arrays() {
            assert_eq!(
                check(&["0:42", "1:true"]).unwrap(),
                Some(r#"[42,true]"#.into())
            );
            assert_eq!(
                check(&["1.0:42", "1.1:true", "0:{}"]).unwrap(),
                Some(r#"[{},[42,true]]"#.into())
            );

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

            assert_matches!(
                check(&["0:42", "0:43"]),
                Err(CompileError::Semantic {
                    source: SemanticError::CollidingAssignments { path },
                    ..
                })
                if path == new_path("0")
            );
        }

        #[test]
        fn test_inconsistent_structure() {
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
        }
    }
}
