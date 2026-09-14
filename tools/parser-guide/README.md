# Parser guide

Interactive page to understand how yabruc reads `.bru` files: a map of the modules, a playground
that shows every decision of the parser step by step, the rules of the syntax and how blocks are
mapped to a `Request`.

```bash
devenv shell parser-guide        # http://localhost:8080
devenv shell parser-guide 9000   # another port
devenv shell parser-guide 8080 100.110.181.122   # listen on another address, like the Tailscale IP
```

By default nginx only listens on `127.0.0.1`. To open it from another machine pass the address of an
interface, or `0.0.0.0` for all of them, and make sure the firewall allows the port.

It also works opening `index.html` directly in a browser.

`parser.js` is a line by line port of `src/parser/bru.rs` and `src/parser/bru2struct.rs`. When
the Rust parser changes, update it and run the differential test, which compares both parsers with
every `.bru` file of the repository, hand written edge cases and thousands of random mutations:

```bash
devenv shell parser-guide-check  # must end with mismatches=0
```
