# mkjson – Command-Line JSON Composer

`mkjson` aids exploratory testing of software that accept JSON input.
It is designed with precision, correctness and CLI ergonomics in mind.

The range of successful outputs is exactly the set of valid JSON texts ([RFC 8259]) with
no unnecessary whitespace and sorted keys.

## Overview

`mkjson` takes a set of directives as input.
Each directive defines a value assignment at a specific path within the JSON structure.

A directive is an instruction to construct part of a JSON value.
Directives are composed together to form the final JSON output.

If all directives are compatible—i.e., there are no structural conflicts or undefined
array indices—the resulting structure is serialized as valid JSON.

### Terminology

 * A *directive* is an instruction to make sure that a given location exists within a json
   tree, and to include a given value at that location.
 * A *path* describes a sequence of steps into a json tree.
   Paths have two syntactic forms: non-empty paths and the root path.
 * A *segment* describes a single step into a json tree.
   Segments have three syntactic forms: bare keys, quoted keys and array indices.


## Syntax

**Directives** come in two forms:

 * a **JSON directive** is a concatenation of a path, a `:`, and a restricted JSON value
 * a **string directive** is a concatenation of a path, a `=`, and a raw UTF-8 string

The restricted JSON value in the JSON directive definition accepts a subset of valid JSON
values ([RFC 8259]).
Specifically, it accepts the literal names (i.e., `null`, `true,` and `false`), any number
or string, and finally the empty object (`{}`) and the empty array (`[]`).

The raw UTF-8 string in the string directive definition accepts any UTF-8 string.
It is automatically converted to a JSON string.
Any `"`, `\` or control (0x00–0x1f and 0x7f) characters are escaped in the JSON string.
Two-character sequences are used where possible, and six-character sequences are used as a
fallback.
Character sequences that happen to look like escape sequences in the raw string, are not
preserved as escape sequences in the JSON string.

Note the difference between the two forms of directives:

| Directive     | Output              | Notes                        |
|---------------|---------------------|------------------------------|
| `foo:42`      | `{"foo":42}`        | JSON number                  |
| `foo=42`      | `{"foo":"42"}`      | String literal `42`          |
| `foo:true`    | `{"foo":true}`      | Boolean                      |
| `foo=true`    | `{"foo":"true"}`    | String `"true"`              |
| `foo:"\n"`    | `{"foo":"\n"}`      | Escaped line feed            |
| `foo="\n"`    | `{"foo":"\"\\n\""}` | Escaped quotes and backslash |


## Path syntax

A **path** is a dot-separated sequence of segments that identify a location with the JSON
structure.
Use `.` to refer to the root path.

Example:

 * `mkjson .:42` → `42`

### Path segments

There are three types of path segments:

 * bare keys (e.g., `café`)
 * quoted keys (e.g., `"foo.bar"`)
 * array indices (e.g., `0`)

#### Bare keys

A bare key is an unquoted identifier used as an object key.

It must be a valid identifier according to [XID rules][UAX #31].
E.g., it cannot contain non-letter symbols.

Examples:

```sh
mkjson foo:42          → {"foo":42}
mkjson café:42         → {"café":42}
mkjson なまえ:42       → {"なまえ":42}
```

Invalid examples:

```sh
mkjson 'foo bar:42'    ✖ Error: space not allowed
mkjson foo-bar:42      ✖ Error: dash not allowed
```

#### Quoted keys

A quoted key is any valid JSON string ([RFC 8259]) used as an object key.

Use this form for keys that do not conform to [XID rules][UAX #31], or if you need a form
that is compatible with any key.

Follows standard JSON string syntax, including escape sequences.

Examples:

```sh
mkjson '"foo bar":42'             → {"foo bar":42}
mkjson '"foo.bar":42'             → {"foo.bar":42}
mkjson '"foo:bar":42'             → {"foo:bar":42}
mkjson '"key with \u2600":1'      → {"key with \u2600":1}
mkjson '""=value'                 → {"":"value"}   # empty string key
```


#### Array indices

An array index is an unsigned integer used as an array index.

 * Must consist of digits only.
 * Must not have leading zeros.
 * Must be contiguous—if index 2 is specified, indices 0 and 1 must also be specified.

Examples:

```sh
mkjson 0:42               → [42]
mkjson 0.0:42             → [[42]]
mkjson foo.1:42 foo.0:43  → {"foo":[43,42]}
```

Invalid examples:

```sh
mkjson 01:42              ✖ Error: leading zero not allowed
mkjson 1:42               ✖ Error: index 0 missing (arrays must be complete)
```


## Directive composition

When multiple directives are provided, they are merged into a single JSON output.
This happens by recursively constructing objects and array according to the directive
paths.
Inconsistent or incomplete compositions result in validation errors.

Example:

 * `mkjson foo.bar:42 foo.baz=hello` → `{"foo":{"bar":42,"baz":"hello"}}`

Invalid examples:

 * `mkjson foo:42 0:43`                   ✖ Root node cannot be both object and array
 * `mkjson foo:42 foo:43`                 ✖ Duplicate assignments to the same path
 * `mkjson '"J":42'` '"\u004a":43'        ✖ Duplicate assignments to the same path
 * `mkjson '"J".b:42'` '"\u004a".b:43'    ✖ Key cannot be both `"J"` and `"\u004a"` in the output
 * `mkjson '"\u004a":42'` '"\u004A".b:43' ✖ Key cannot be both `"\u004a"` and `"\u004A"` in the output
 * `mkjson 1:42                           ✖ Array index 0 is undefined


## Examples

 * `mkjson foo:42` → `{"foo":42}`
 * `mkjson foo.bar=hello` → `{"foo":{"bar":"hello"}}`
 * `mkjson 0:42 1:true` → `[42,true]`
 * `mkjson '.:[]'` → `[]`


## Compatibility

 * The output conforms to [RFC 8259] \(the JSON specification).
 * The output is compact JSON: no superfluous whitespace or indentation.
 * Object keys are sorted in Unicode codepoint order.


## Error Handling

Errors may result from:

 * Syntax issues (malformed paths, invalid characters)
 * Invalid JSON values (in quoted keys and restricted JSON values).
 * Structural conflicts (e.g., assigning both an object and an array at the same path)
 * Incomplete array definitions (e.g., assigning to index 1 but not to 0)


## Formal Grammar

The following ABNF ([RFC 5234]) grammar defines the exact syntax accepted for directives.

```abnf
directive        = json-directive / string-directive
json-directive   = path colon json-value
string-directive = path equal-sign raw-string
path             = root-path / non-empty-path
root-path        = period
non-empty-path   = segment *( period segment )
segment          = bare-key / quoted-key / array-index
bare-key         = xid-start *xid-continue
quoted-key       = json-string
array-index      = json-int
raw-string       = *( %x00-10ffff )
json-value       = ( json-null /
                     json-true /
                     json-false /
                     json-number /
                     json-string /
                     empty-object /
                     empty-array )
empty-array      = %x5b.5d      ; {}
empty-object     = %x7b.7d      ; []
period           = %x2e         ; .
colon            = %x3a         ; :
equal-sign       = %x3d         ; =
; json-false     = false as defined in RFC 8259
; json-int       = int as defined in RFC 8259
; json-null      = null as defined in RFC 8259
; json-number    = number as defined in RFC 8259
; json-string    = string as defined in RFC 8259
; json-true      = true as defined in RFC 8259
; xid-continue   = XID_Continue as defined in UAX #31
; xid-start      = XID_Start as defined in UAX #31
```


## Character encoding gotchas

### Shell quoting

Use `'single quotes'` to avoid shell interpretation `"` or `\` in command line arguments.
Escape `'` with backslash inside single quoted command line arguments to avoid shell
interpretation.

### JSON quoting

Escape `"` and `\` with backslash inside JSON strings to avoid JSON interpretation.
Escape control characters using either the two or six character escape notations (e.g.,
`\n` or `\u001f`).

### Raw-string quoting

Note that when raw string values are converted to JSON strings, any `"` and `\` characters
are encoded as `\"` and `\\` in the JSON string representation.
E.g, if a raw string contains `\t`, that is interpreted as a `\` and a `t`, and not as an
escaped tab character.

### Invalid UTF-8 encodings

JSON ([RFC 8259]) allows strings to represent UTF-8 encodings of codepoints that are
reserved for UTF-16-only codepoints.
Starting up, `mkjson` binary UTF-8 decodes its arguments.
If an argument contains a UTF-16-only codepoint, the UTF-8 decoding fails, making it
impossible to produce JSON containing such UTF-8 encodings.
There is currently no solution for this.

### Null characters

POSIX does not support the null character in command line arguments.
There is currently no way to include null characters in the JSON output when invoking
`mkjson` from the command line.
As a partial work around, escaped null characters (`\u0000`) can be included.
If you are calling into `mkjson` programmatically, null characters are fully supported.



[RFC 8259]: https://www.rfc-editor.org/rfc/rfc8259
[RFC 5234]: https://www.rfc-editor.org/rfc/rfc5234
[UAX #31]: https://www.unicode.org/reports/tr31/
