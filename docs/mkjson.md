# mkjson – Command-Line JSON Composer

`mkjson` aids exploratory testing of software that accepts JSON input.
It is designed with precision, correctness, and CLI ergonomics in mind.

The range of successful outputs is exactly the set of valid JSON texts ([RFC 8259]) with
no unnecessary whitespace and sorted keys.

---

## Overview

`mkjson` takes a set of directives as input.
Each directive defines a value assignment at a specific path within the JSON structure.

Directives are composed together to form the final JSON output.
If all directives are compatible—i.e., there are no structural conflicts or undefined
array indices—the result is serialized as valid JSON.

---

## Usage

```sh
mkjson [DIRECTIVE]...
```

### Arguments

- `[DIRECTIVE]...`  — One or more path-based directives (e.g., `foo:42`, `bar.baz=hello`)

### Options

| Option             | Description             |
|--------------------|-------------------------|
| `-h`, `--help`     | Show help message       |
| `-V`, `--version`  | Show version information|

---

## Examples

```sh
mkjson foo:42                     → {"foo":42}
mkjson foo.bar=hello              → {"foo":{"bar":"hello"}}
mkjson foo=42 foo=true            ✖ Invalid: conflicting assignments
mkjson 0=x 1=y                    → ["x","y"]
mkjson .:{}                       → {}
mkjson '"foo.bar":42'             → {"foo.bar":42}
mkjson '"":true'                  → {"":true}
```

---

## Features

- Accepts both raw strings and structured JSON values
- Builds nested objects and arrays via path composition
- Produces compact, sorted JSON
- Validates structural consistency
- Emits helpful errors for conflicting assignments

---

## Error Cases

The following inputs will cause errors:

```sh
mkjson foo:1 0:2       ✖ Invalid: Mixing object and array at the root
mkjson foo:1 foo:2     ✖ Invalid: Assigning to the same path twice
mkjson 1:true          ✖ Invalid: Skipping array indices
```

See [Directive Syntax](./directive-syntax.md#composition) for more.

---

## Output Format

- Conforms to [RFC 8259]
- Minimal: no extra whitespace
- Unicode object keys are sorted by codepoint

---

## See Also

- [Directive Syntax](./directive-syntax.md) – full reference for paths and directives
- [mkjsonrpc](./mkjsonrpc.md) – build JSON-RPC requests using the same syntax

[RFC 8259]: https://www.rfc-editor.org/rfc/rfc8259
