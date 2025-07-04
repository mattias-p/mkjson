# Design Rationale

`mkjson` and `mkjsonrpc` are designed to make JSON construction simple and robust —
especially in environments like shell scripts or CI pipelines.

---

## The Problem

Writing JSON by hand on the command line is frustrating.
Quoting is brittle and escaping is error-prone.
Even simple structures are easy to mess up, and small mistakes—like a missing comma or an
extra quote—are hard to spot.
It works, but it’s clumsy and easy to get wrong.

---

## The Solution

This toolkit introduces a **directive-based syntax** to define JSON structures as a flat
list of paths and values. This syntax:

 * Is compact and composable
 * Maps naturally to CLI arguments
 * Avoids brittle shell quoting and escaping
 * Supports nesting and arrays

For example:

```sh
mkjson \
  name=Alice \
  age:30 \
  contact.email=alice@example.com \
  | jq .
```

Produces:

```json
{
  "name": "Alice",
  "age": 30,
  "contact": {
    "email": "alice@example.com"
  }
}
```

---

## Why Not Use `jo`?

[`jo`] and `mkjson` solve the same basic problem, but in slightly different ways.

The most obvious difference is that `mkjson` has native support for nesting, while `jo`
relies on command substitution.
This means you typically need fewer quotes and parentheses with `mkjson`.

Another difference is in input handling.
`jo` is more permissive, which can lead to silent errors.
`mkjson` is stricter and catches more mistakes, but expects more precision.

`jo` works well for flat or simple structures, but becomes less practical for constructing
deeply nested structures, like JSON-RPC requests with nested params.

`mkjson` is designed for users who want exact control over the output and are willing to
be precise to get it.

---

## What About JSON-RPC?

`mkjsonrpc` builds on the same directive syntax, wrapping it in a valid [JSON-RPC 2.0]
request structure.
This makes it easier to script against JSON-RPC APIs without manually composing full
payloads.

For example:

```bash
mkjsonrpc subtract \
  minuend:42 \
  subtrahend:23 \
  | jq .
```

Produces:

```json
{
  "jsonrpc": "2.0",
  "method": "subtract",
  "params": {
    "minuend": 42,
    "subtrahend": 23
  },
  "id": 1
}
```

---

## Use Cases

The primary use case for this toolkit is exploratory testing of software that consumes
JSON.
But it also happens to be well-suited for:

 * Shell scripts and cron jobs
 * CI/CD pipelines
 * Lightweight service integrations
 * Developer tooling and API prototyping

---

## Philosophy

The design philosophy of these tools can be broken down into a list of principles.
Whenever two principles collide, the earlier principle should take precedence.

 1. Be correct (folllow specifications)
 2. Be precise (no type coersion, reject invalid inputs)
 3  Be predictable (behave deterministically, document behaviors)
 4. Be focused (do one thing and do it well, refuse invalid outputs)
 5. Be ergonomic (avoid special characters in syntax)
 6. Be boring (look for precedent before inventing something new)

---

## See Also

 * [Directive Syntax Reference]
 * [jo]
 * [JSON-RPC 2.0]
 * [mkjson CLI Guide]
 * [mkjsonrpc CLI Guide]




[Directive Syntax Reference]: ./directive-syntax.md
[jo]:                         https://github.com/jpmens/jo
[JSON-RPC 2.0]:               https://www.jsonrpc.org/specification
[mkjson CLI Guide]:           ./mkjson.md
[mkjsonrpc CLI Guide]:        ./mkjsonrpc.md
