# mkjson – Command-Line JSON Composer

`mkjson` aids exploratory testing of software that accept JSON input.
It is designed with precision, correctness and CLI ergonomics in mind.

The range of successful outputs is exactly the set of valid JSON texts (per [RFC 8259])
with minimized whitespace and sorted keys.

## Overview

`mkjson` takes a set of directives as input.
Each directive defines a value assignment at a specific path within the JSON structure.

You can think of a directive as an instruction for constructing part of a JSON value.
Directives are composed together to form the final JSON output.

If all directives are compatible—i.e., there are no structural conflicts or undefined
array indices—the resulting structure is serialized as valid JSON.

## Syntax

Each **directive** consists of:

 * a path
 * an assignment operator
 * a value

### Assignment operators

There are two **assignment operators**:

 * `:`, the **JSON assignment** operator, assigns a value parsed as JSON.
 * `=`, the **string assignment** operator, assigns a value interpreted as a raw string.

Examples:

 * `mkjson foo:42` → `{"foo":42}`
 * `mkjson foo=42` → `{"foo":"42"}`


## Path syntax

A **path** is a dot-separated sequence of segments that identify a location with the JSON
structure.
Use `.` to refer to the root path.

Example:

 * `mkjson .:42` → `42`

### Path segments

A **segment** is either:

 * a **bare key** (e.g., `foo`; `mkjson foo:42` → `{"foo":42}`)
 * a **quoted key** (e.g., `"foo bar"`; `mkjson '"foo bar":42'` → `{"foo bar":42}`)
 * an **array index** (e.g., `0`; `mkjson 0:42` → `[42]`)

Keys that contain spaces, punctuation, or special characters must be quoted.
Array indices must be unsigned integers without leading zeros.


## Directive composition

When multiple directives are provided, they are merged into a single JSON output.
This happens by recursively constructing objects and array according to the directive
paths.
Conflicts—such as duplicate assignments to the same path or incompatible structures—will
result in validation errors.

Example:

 * `mkjson foo.bar:42 foo.baz=hello` → `{"foo":{"bar":42,"baz":"hello"}}`


## Examples

 * `mkjson foo:42` → `{"foo":42}`
 * `mkjson foo.bar=hello` → `{"foo":{"bar":"hello"}}`
 * `mkjson 0:42 1:true` → `[42,true]`
 * `mkjson '.:[]'` → `[]`


## Compatibility

 * The output conforms to [RFC 8259] \(the JSON specification).
 * Object keys are sorted.
 * Whitespace is minimized.


## Error handling

Errors may result from:

 * Syntax issues (malformed paths, invalid characters)
 * Invalid JSON values (when using quoted keys and the JSON assignment operator).
 * Structural conflicts (e.g., assigning both an object and an array at the same path)
 * Incomplete array definitions (e.g., assigning to index 1 but not to 0)










[RFC 8259]: https://www.rfc-editor.org/rfc/rfc8259
