# Design Rationale

This document explains the motivations, design principles, and key trade-offs behind
`mkjson` and `mkjsonrpc`.

---

## The Problem

Writing JSON by hand on the command line is error-prone and awkward.
Quoting is brittle and escaping is error-prone.
Even simple structures are easy to mess up.
This leads to brittle scripts and hard-to-maintain tooling.
The friction becomes even more painful when you're iterating rapidly — tweaking
parameters, exploring inputs, or scripting interactions with JSON-speaking APIs.

---

## Prior Art

### JSON construction

Several tools already exist to help with JSON construction in shell environments.
The most notable example is [`jo`], which provides a compact command-line syntax for
building JSON objects.

`jo` works well for simple, flat structures.
However, it tends to break down in more complex use cases:

 * It relies on command substitution to express nested data.
 * It becomes verbose and fragile when constructing deeply nested or recursive structures.
 * Its permissiveness can lead to silent errors or unexpected behavior if inputs are
   malformed.
 * It may limit the precision of numbers, change their formatting (e.g., decimal vs
   scientific notation), or coerce large numbers to strings.

Other alternatives include inline heredocs and `jq -n`.
These tools all informed the design of `mkjson`, but none offered the balance of
strictness and conciseness we were looking for — especially for iterative ad-hoc API
interaction during exploratory testing.

### Path Syntax

There are several established languages for querying JSON ([jq], [JMESPath], [RFC 9353] –
JSONPath).
All of these languages have similar notations for deep member access, e.g.,
`.items[0].price`.
They all differ in prefix before the first object key (`.`, the empty string or `$.`) and
how they refer to the root node (`.`, `@` or `$`).

---

## Design Goals and Constraints

The design of `mkjson` and `mkjsonrpc` was guided by a desire to make JSON construction:

 * Reliable — inputs are validated early and fail loudly on errors.
 * Precise — numeric and structural fidelity are preserved across environments.
 * Composable — the syntax should fit naturally into Unix pipelines and CLI workflows.
 * Unambiguous — there should be no guessing about how inputs will be interpreted.
 * Minimalist — avoid unnecessary syntax, quoting, or structural ceremony.
 * Safe — output is always valid JSON; malformed inputs are rejected early.

Constraints included:

 * No runtime dependencies beyond a standard Unix shell environment.
 * Interoperable with tools like jq or curl, without re or scripting overhead.quiring wrapper scripts.
 * Operable entirely from the command line, without auxiliary files.

---

## Directive-Based Structure

At the heart of `mkjson` is a flat directive syntax: each argument encodes a path and
value assignment.

For example, this command:

```sh
mkjson items.0.article=pen items.0.quantity:5 | jq .
```

Produces the following JSON:

```json
{
  "items": [
    {
      "article": "pen",
      "quantity": 5
    }
  ]
}
```

This approach offers several advantages:

 * **Flat and readable**: avoids deeply nested quoting or complex command substitution.
 * **Predictable parsing**: each argument is interpreted deterministically with no
   heuristics.
 * **Shell-native**: the syntax avoids characters that require shell escaping, making it
   easy to use inline or in scripts.
 * **Composable**: works well in pipelines with tools like `jq` or `curl`.

Although inspired by query languages like JSONPath, this syntax is used for constructing
structures, and adapted for command line safety.

---

## JSON-RPC Construction

`mkjsonrpc` extends the same directive-based syntax as `mkjson`, but wraps it in a
[JSON-RPC 2.0] request envelope.
This makes it possible to construct complete, well-formed JSON-RPC requests entirely from
the command line, without relying on inline heredocs, wrapper scripts, or template files.

For example:

```sh
mkjsonrpc get_weather location.city=Stockholm location.unit=metric | jq .
```

Produces:

```json
{
  "id": 1,
  "jsonrpc": "2.0",
  "method": "get_weather",
  "params": {
    "location": {
      "city": "Stockholm",
      "unit": "metric"
    }
  }
}
```

---

## Error Handling and Strictness

One of the most deliberate design choices was strict input validation.

Where some tools prioritize leniency or convenience, `mkjson` and `mkjsonrpc` are designed
to:

 * **Fail fast**: Misformatted paths, unsupported types, ambiguous values, or inputs that
   would result in ill-formed JSON are rejected early.
 * **Avoid silent coercion**: String values that look like numbers aren't parsed as such
   when using the typed assignment operator (`:`).

This strictness can feel harsh during casual use, but it pays dividends in reliability —
especially when troubleshooting and in scripts where silent failure can be costly.

---

## Use Cases

The primary audience for these tools is developers working in environments where JSON must
be assembled quickly, reliably, and often temporarily.

Typical use cases include:

 * Building test payloads when exploring APIs
 * Scripting JSON-RPC requests in shell-based tools
 * Generating structured data in CI pipelines
 * Replacing fragile heredocs or inline JSON in shell scripts
 * Creating repeatable CLI test harnesses

---

## Summary

`mkjson` and `mkjsonrpc` were created to fill a gap: a precise, predictable,
shell-friendly way to construct JSON and JSON-RPC payloads. Their design reflects a strong
bias toward reliability, correctness, and simplicity — particularly in CLI and automation
contexts where brittle tooling can quickly become a liability.

By prioritizing deterministic behavior, explicit typing, and ergonomic syntax, the tools
aim to reduce friction and help developers focus on what they’re building — not how to
format it.

---

## See Also

 * [Directive Syntax Reference]
 * [JMESPath]
 * [jo]
 * [jq]
 * [JSON-RPC 2.0]
 * [mkjson CLI Guide]
 * [mkjsonrpc CLI Guide]
 * [RFC 9353] – JSONPath standard




[Directive Syntax Reference]: ./directive-syntax.md
[JMESPath]:                   https://jmespath.org/
[jo]:                         https://github.com/jpmens/jo
[jq]:                         https://jqlang.org/
[JSON-RPC 2.0]:               https://www.jsonrpc.org/specification
[mkjson CLI Guide]:           ./mkjson.md
[mkjsonrpc CLI Guide]:        ./mkjsonrpc.md
[RFC 9353]:                   https://www.rfc-editor.org/rfc/rfc9535.html
