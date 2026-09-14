---
# The next page is the parser guide, a static page that the VitePress router can not open
next: false
---

# How it works

yabruc reads a Bruno collection in three layers:

1. **Loader** (`src/collection.rs`): finds the collection root (`opencollection.yml` or `bruno.json`), walks the folders skipping `environments/`, `node_modules`, `.git` and the `ignore` list, and sorts folders and requests like `bru run`.
2. **Syntax** (`src/parser/bru.rs`): turns a `.bru` file into a list of blocks (dictionaries, text, arrays and inline values). YAML files are read with `serde_yaml_ng`.
3. **Mapping** (`src/parser/bru2struct.rs`, `src/parser/yml2struct.rs`): gives meaning to those blocks and builds the same `Request` for both formats.

::: tip Interactive guide
The [parser guide](./parser-guide/index.html){target="_self"} has a playground that shows every decision of the parser step by step, the rules of the syntax and how each block is mapped to a request (in Spanish).
:::
