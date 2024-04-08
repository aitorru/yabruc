# Usage
To use this tool, just install it with cargo.
```bash
cargo install --git https://github.com/aitorru/yabruc
```

The cli usage is the same as [bru](https://docs.usebruno.com/bru-cli/overview).
You can run it pointing to a folder or a file.

```bash
yabruc run collection/
```
```
✅ Scanned folder in 580.301469ms
✅ Parsed collection/Example PUT.bru in 51.238µs
✅ Parsed collection/Example Empty.bru in 21.591µs
✅ Parsed collection/Example GET.bru in 44.956µs
✅ Parsed collection/Example POST.bru in 25.9µs 
```

```bash
yabruc run example.bru
```
```
✅ Scanned file in 67.559µs
✅ Parsed example.bru in 55.766µs
```
## How to contribute
If you want to contribute to the development of this crate, you will need to download devenv. You can do this by following these steps:
1. Clone the repository:
```bash
git clone https://github.com/aitorru/yabruc
```
2. Download [devenv](https://devenv.sh/getting-started/)
3. Set up the development environment:
```bash
devenv shell
```
::: tip
For help with testing, devenv processes can help.

It will spin up a nginx service with an example server.

```
───────┬─────────────────────────────────────────────────
       │ File: devenv.nix
───────┼─────────────────────────────────────────────────
  15   │   # https://devenv.sh/processes/
  16   │   # processes.ping.exec = "ping example.com";
  17   │   services.nginx = {
  18   │     enable = true;
  19   │     httpConfig = ''
  20   │       server {
  21   │         listen 1234;
  22   │         location / {
  23   │           return 200 "Hello, world!";
  24   │         }
  25   │       }
  26   │     '';
  27   │   };
───────┴─────────────────────────────────────────────────
```

To create the server, just type:

```bash
devenv up
```

For more documentation [read this](https://devenv.sh/processes/)
:::