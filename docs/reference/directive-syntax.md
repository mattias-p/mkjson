# Directive Syntax for JSON Construction

This document defines the shared syntax used by tools like [`mkjson`](./mkjson.md) and
[`mkjsonrpc`](./mkjsonrpc.md) to build JSON structures from the command line using concise
path-based directives.

These directives offer a compact and expressive way to describe JSON values, making them
ideal for use in shell scripts, debugging, or manual testing.

---

## Overview

Each directive assigns a value to a specific path within a JSON tree.

Directives come in two forms:

| Type              | Syntax                 |
|-------------------|------------------------|
| JSON directive    | `path:json-value`      |
| String directive  | `path=utf8-string`     |


Examples:

```sh
mkjson foo:42                  → {"foo":42}
mkjson foo=42                  → {"foo":"42"}
```

Directives are composed together to produce a final JSON structure.
Conflicting sets of directives result in errors.
Note that array nodes with unspecified elements are not allowed.

---

## Paths

A **path** identifies a location within a JSON tree using dot-separated **segments**.

Special case:
- `.` (a single period) refers to the root of the JSON value.

### Examples

```sh
mkjson .:42                    → 42
mkjson foo.bar=hello           → {"foo":{"bar":"hello"}}
mkjson 0:42 1:true             → [42,true]
```

---

## Path Segments

A segment is one of:

| Segment Type   | Example        | Notes                              |
|----------------|----------------|------------------------------------|
| **Bare key**   | `foo`          | Must follow Unicode XID rules      |
| **Quoted key** | `"foo.bar"`    | Full JSON string syntax            |
| **Array index**| `0`, `1`       | Zero-based, no gaps allowed        |

### Bare Keys

Follow Unicode XID (identifier) rules. Examples:

```sh
mkjson foo:42                  → {"foo":42}
mkjson café:42                 → {"café":42}
```

Invalid:

```sh
mkjson 'foo bar:42'            ✖ Invalid: space not allowed
mkjson foo-bar:42              ✖ Invalid: dash not allowed
```

### Quoted Keys

Use for keys with special characters or whitespace.

```sh
mkjson '"foo bar":42'          → {"foo bar":42}
mkjson '"key with \u2600":1'   → {"key with \u2600":1}
mkjson '""=value'              → {"":"value"}
```

### Array Indices

Must be contiguous from `0`. Valid:

```sh
mkjson 0:42                    → [42]
mkjson foo.1:42 foo.0:43       → {"foo":[43,42]}
```

Invalid:

```sh
mkjson 1:42                    ✖ Invalid: index 0 missing
mkjson 01:42                   ✖ Invalid: leading zero
```

---

## Directive Types

### JSON Directives

Assign a structured value (literal, number, empty object/array):

```sh
mkjson foo:42                  → {"foo":42}
mkjson foo:true                → {"foo":true}
mkjson 'foo:"\n"'              → {"foo":"\n"}
```

### String Directives

Assign raw UTF-8 strings, auto-escaped as JSON strings:

```sh
mkjson foo=42                  → {"foo":"42"}
mkjson foo=true                → {"foo":"true"}
mkjson 'foo="\n"'              → {"foo":"\"\\n\""}
```

---

## Input–Output Examples

This section demonstrates exactly how directives are interpreted and serialized into JSON.
Use it to explore how string vs. JSON directives affect the output.

```sh
mkjson foo:42                  → {"foo":42}
mkjson foo=42                  → {"foo":"42"}
mkjson flag:true               → {"flag":true}
mkjson flag=true               → {"flag":"true"}
mkjson 'foo:"\n"'              → {"foo":"\n"}
mkjson 'foo="\n"'              → {"foo":"\"\\n\""}
mkjson 0:42 1:true             → [42,true]
mkjson foo.0:1 foo.1=bar       → {"foo":[1,"bar"]}
mkjson .:false                 → false
```

> Try these in a terminal with `mkjson` to observe their exact behavior.

---

## Composition

Multiple directives are recursively merged:

```sh
mkjson foo.bar:42 foo.baz=no   → {"foo":{"bar":42,"baz":"no"}}
```

Invalid compositions:

```sh
mkjson foo:42 0:43             ✖ Invalid: root cannot be both object and array
mkjson foo:42 foo:43           ✖ Invalid: duplicate path
mkjson '"J":42' '"\u004a":43'  ✖ Invalid: equivalent key conflict
```

---

## Escaping and Encoding

### Shell Quoting

- Use `'single quotes'` in the shell to avoid interpretation of `"` or `\`.

### JSON Escaping

- Inside JSON strings, escape `"` and `\` as `\"` and `\\`.

### Raw String Escaping

- A raw string like `\t` becomes `\\t` in the output string, not a tab character.

---

## Limitations

- Null characters (`\u0000`) not supported via CLI on POSIX shells.
- Invalid UTF-8 sequences will raise errors.
- Surrogate pairs for UTF-16-only codepoints cannot be directly passed via CLI, but may be
  constructed programmatically.

---

## ABNF Grammar

```abnf
directive             = json-directive / string-directive
json-directive        = path ":" restricted-json-value
string-directive      = path "=" relaxed-utf8-string
path                  = "." / ( segment *( "." segment ) )
segment               = bare-key / quoted-key / array-index
bare-key              = xid-start *xid-continue
quoted-key            = json-string
array-index           = json-int
relaxed-utf8-string   = *( %x00-10ffff )
restricted-json-value = json-null / json-true / json-false /
                        json-number / json-string /
                        empty-object / empty-array
empty-object          = "{}"
empty-array           = "[]"
```

> N.b.:
> Rules starting with `json-` are defined in [RFC 8259].
> Rules starting wiht `xid-` are defined in [UAX #31].

---

## See Also

- [mkjson – JSON composer](./mkjson.md)
- [mkjsonrpc – JSON-RPC composer](./mkjsonrpc.md)
- [RFC 8259 – JSON standard][RFC 8259]
- [UAX #31 – Unicode Identifier Guidelines][UAX #31]


[RFC 8259]: https://www.rfc-editor.org/rfc/rfc8259
[UAX #31]: https://www.unicode.org/reports/tr31/
