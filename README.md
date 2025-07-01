# JSON Composition Tools

This repository contains command-line tools for composing JSON and JSON-RPC messages.

## Tools

### [`mkjson`](./docs/mkjson.md)
Compose arbitrary JSON structures using a path-based directive syntax.

### [`mkjsonrpc`](./docs/mkjsonrpc.md)
Compose JSON-RPC 2.0 requests using the same directive syntax to build `params`.

## Shared Syntax

Both tools use the same directive-based syntax for constructing JSON data.
See [Directive Syntax](./docs/directive-syntax.md) for full details.
