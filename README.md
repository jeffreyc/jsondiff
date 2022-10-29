# jsondiff

jsondiff is a utility for comparing two [JSON](https://www.json.org/) files. It
ignores whitespace and ordering of unordered values, and generates
[JSON Patch](https://jsonpatch.com/)
([RFC 6902](https://datatracker.ietf.org/doc/html/rfc6902/)) output describing
the differences between the two files.

## Usage

```shell
% jsondiff old.json old.json
Comparing old.json and old.json
No differences were detected.

% jsondiff old.json new.json
Comparing old.json and new.json
[
  {"op":"remove","path":"/removed_value"},
  {"op":"add","path":"/added_value","value":"This is the value that was added."},
  {"op":"replace","path":"/changed_value","value":"This is the value that was changed."}
]
```

## License

jsondiff is dual-licensed under the [Apache License, v2.0](LICENSE-APACHE.md)
or the [MIT License](LICENSE-MIT.md).
