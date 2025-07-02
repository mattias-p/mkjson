use mkjson::composer::compose;
use regex::Regex;
use std::sync::LazyLock;

const POSITIVE_INDICATOR: &str = "→";
const NEGATIVE_INDICATOR: &str = "✖";

static POSITIVE_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^mkjson\s+(.+?)\s+→\s+(.*)$"#).unwrap());
static NEGATIVE_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^mkjson\s+(.+?)\s+✖"#).unwrap());

struct Example {
    line_no: usize,
    args: Vec<Vec<u8>>,
    expected: Option<String>,
}

fn extract_examples(filename: &str) -> Vec<Example> {
    use std::fs::read_to_string;

    let readme = read_to_string(filename).expect("Failed to read file");

    let mut examples = vec![];

    for (i, line) in readme.lines().enumerate() {
        let line_no = i + 1;
        if line.contains(POSITIVE_INDICATOR) && !line.contains(NEGATIVE_INDICATOR) {
            if let Some(caps) = POSITIVE_PATTERN.captures(line) {
                let input_raw = caps.get(1).unwrap().as_str();
                let output_expected = caps.get(2).unwrap().as_str();

                // split CLI-style string into args
                let args = shell_words::split(input_raw)
                    .map_err(|e| format!("{} line {}: error {}", filename, line_no, e))
                    .unwrap()
                    .into_iter()
                    .map(|s| s.bytes().collect())
                    .collect();

                examples.push(Example {
                    line_no,
                    args,
                    expected: Some(output_expected.to_string()),
                });
            } else {
                panic!(
                    "Line {} looks like a positive example but didn’t match",
                    line_no
                );
            }
        } else if line.contains(NEGATIVE_INDICATOR) && !line.contains(POSITIVE_INDICATOR) {
            if let Some(caps) = NEGATIVE_PATTERN.captures(line) {
                let input_raw = caps.get(1).unwrap().as_str();

                // split CLI-style string into args
                let args = shell_words::split(input_raw)
                    .map_err(|e| format!("{} line {}: error {}", filename, line_no, e))
                    .unwrap()
                    .into_iter()
                    .map(|s| s.bytes().collect())
                    .collect();

                examples.push(Example {
                    line_no,
                    args,
                    expected: None,
                });
            } else {
                panic!(
                    "Line {} looks like a negative example but didn’t match",
                    line_no
                );
            }
        } else if line.contains(NEGATIVE_INDICATOR) && line.contains(POSITIVE_INDICATOR) {
            panic!(
                "Line {} looks like a both a positive and negative example",
                line_no
            );
        }
    }

    examples
}

fn check_examples(filename: &str) {
    let examples = extract_examples(filename);

    assert!(!examples.is_empty(), "No examples found in README.md.");
    println!("✅ Found {} examples in {}", examples.len(), filename);

    for example in examples {
        let output = compose(example.args.into_iter());
        if let Some(expected) = example.expected {
            assert_eq!(
                output
                    .map_err(|e| format!("{} line {}: {}", filename, example.line_no, e))
                    .unwrap()
                    .ok_or_else(|| format!("{} line {}", filename, example.line_no))
                    .unwrap()
                    .to_string(),
                expected,
                "{} line {}",
                filename,
                example.line_no
            );
        } else {
            assert!(output.is_err());
        }
    }
}

#[test]
fn test_directive_syntax_examples() {
    check_examples("docs/directive-syntax.md");
}

#[test]
fn test_directive_mkjson_examples() {
    check_examples("docs/mkjson.md");
}

#[test]
#[ignore]
fn test_directive_mkjsonrpc_examples() {
    check_examples("docs/mkjsonrpc.md");
}
