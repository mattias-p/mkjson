# mkjsonrpc – JSON-RPC Request Builder

`mkjsonrpc` constructs JSON-RPC 2.0 requests using the same path-based directive syntax as
[`mkjson`](./mkjson.md).

It is designed for use in testing and scripting scenarios, enabling quick composition of
valid JSON-RPC request objects directly from the shell.

---

## 🚀 Overview

`mkjsonrpc` builds a complete JSON-RPC 2.0 request object by:

- Setting the `jsonrpc` field to `"2.0"`
- Setting the required `method` field
- Constructing the `params` field from path-based directives (as in `mkjson`)
- Optionally including an `id` field

---

## Usage

```sh
mkjsonrpc --method <METHOD> [DIRECTIVES]...
```

### Options

| Option               | Description                              |
|----------------------|------------------------------------------|
| `-m`, `--method`     | Required. Method name for the request.   |
| `-i`, `--id`         | Optional. Sets the `"id"` field.         |
| `-h`, `--help`       | Show help message.                       |
| `-V`, `--version`    | Show version information.                |

---

## Example

```sh
mkjsonrpc -m subtract x:42 y:23
```

Output:

```json
{
  "jsonrpc": "2.0",
  "method": "subtract",
  "params": {
    "x": 42,
    "y": 23
  }
}
```

With an ID:

```sh
mkjsonrpc -m subtract -i 1 x:42 y:23
```

```json
{
  "jsonrpc": "2.0",
  "method": "subtract",
  "params": {
    "x": 42,
    "y": 23
  },
  "id": 1
}
```

---

## Directive Syntax

`mkjsonrpc` uses the exact same directive and path syntax as [`mkjson`](./mkjson.md).

See [Directive Syntax](./directive-syntax.md) for:

- Path structure and quoting
- JSON vs. string directive rules
- Array and object nesting
- Escaping, character encoding, and grammar

---

## Output

- Follows the [JSON-RPC 2.0 Specification](https://www.jsonrpc.org/specification)
- Fields always included: `jsonrpc`, `method`, `params`
- Optional field: `id` (when provided)

---

## Error Cases

- Missing `--method` option
- Invalid or conflicting directives (e.g., arrays with missing indices)
- Duplicate path assignments
- Mixing incompatible JSON types in `params`

See the [Directive Syntax](./directive-syntax.md#composition) section for examples.

---

## See Also

- [mkjson](./mkjson.md) – base tool for composing raw JSON
- [Directive Syntax](./directive-syntax.md) – path and value composition rules
