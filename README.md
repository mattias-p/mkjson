# mkjson – Command-Line JSON Composer

`mkjson` accepts specifications in a shell friendly syntax, and constructs and prints the
corresponding JSON value.

It was created to aid exploratory testing of softwares that accept JSON for input, but it
could also be useful for production use cases.

The range of successful outputs is exactly the set of valid JSON texts (per [RFC 8259])
with minimized whitespace and sorted object keys.




[RFC 8259]: https://www.rfc-editor.org/rfc/rfc8259
