# serde_json_path_to_error

[![License](https://img.shields.io/crates/l/serde_json_path_to_error.svg)](https://crates.io/crates/serde_json_path_to_error)
[![Latest version](https://img.shields.io/crates/v/serde_json_path_to_error.svg)](https://crates.io/crates/serde_json_path_to_error)
[![Latest Docs](https://docs.rs/serde_json_path_to_error/badge.svg)](https://docs.rs/serde_json_path_to_error/)
[![downloads-badge](https://img.shields.io/crates/d/serde_json_path_to_error.svg)](https://crates.io/crates/serde_json_path_to_error)

[API docs](https://docs.rs/serde_path_to_error/)

A drop in replacement for [serde_json] with errors enriched by [serde_path_to_error].

This is usually a better default since it makes it easier to debug when serialization or deserialization fails.
Paths are particularly helpful when your schema is large or when it's difficult to see the raw data that causes an error.

This crate exposes the same items as [serde_json], just with different error types.
For more detailed documentation see [serde_json].

## Migrating from [serde_json]

To enrich your errors simply replace your dependency on [serde_json] with one on serde_json_path_to_error.

```diff
- serde_json = "1.0"
+ serde_json = { package = "serde_json_path_to_error", version = "0.1" }
```

Alternatively, you can add serde_json_path_to_error as a regular dependancy...

```text
# cargo add serde_json_path_to_error
```

..and rename the crate in your crate root to get the same API as [serde_json].

```rust
extern crate serde_json_path_to_error as serde_json;
```

In most cases, your project should continue to compile after migrating.
Your errors will now be enriched with additional context showing the path to serialization and deserialization failures.

```rust
// the rename trick shown above
extern crate serde_json_path_to_error as serde_json;

# use std::collections::BTreeMap as Map;
# use serde::Deserialize;
#[derive(Deserialize)]
struct Package {
    name: String,
    dependencies: Map<String, Dependency>,
}

#[derive(Deserialize)]
struct Dependency {
    version: String,
}

fn main() {
    let j = r#"{
        "name": "demo",
        "dependencies": {
            "serde": {
                "version": 1
            }
        }
    }"#;

    // Uses the enriched version from [serde_json_path_to_error] but with the exact same API
    // you've come to expect from [serde_json]
    let result: Result<Package, _> = serde_json::from_str(j);

    match result {
        Ok(_) => panic!("expected a type error"),
        Err(err) => {
            // You get the error including the path as a default
            assert_eq!(
              err.to_string(),
              "dependencies.serde.version: invalid type: integer `1`, expected a string at line 5 column 28",
            );
            // You can get just the path
            assert_eq!(
              err.path().to_string(),
              "dependencies.serde.version",
            );
            // Or just the original serde_json error
            assert_eq!(
              err.into_inner().to_string(),
              "invalid type: integer `1`, expected a string at line 5 column 28",
            );
        }
    }
}
```

## Caveats

There are still a small number of items that don't return enriched errors.
I'd be interested in accepting PRs that wrap these items.

- [serde_json::de::Deserializer] [#6](https://github.com/eopb/serde_json_path_to_error/issues/6)
- [serde_json::de::StreamDeserializer] [#5](https://github.com/eopb/serde_json_path_to_error/issues/5)
- [serde_json::ser::Serializer] [#4](https://github.com/eopb/serde_json_path_to_error/issues/4)
- [serde_json::value::Serializer] [#3](https://github.com/eopb/serde_json_path_to_error/issues/3)

[serde_json]: https://docs.rs/serde_json/latest/serde_json/
[serde_path_to_error]: https://docs.rs/serde_json/latest/serde_path_to_error/
