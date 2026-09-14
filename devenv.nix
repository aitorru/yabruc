{ pkgs, ... }:

{
  # https://devenv.sh/packages/
  packages = [ pkgs.fzf pkgs.ripgrep pkgs.bat pkgs.bun pkgs.just pkgs.cargo-audit ];

  # https://devenv.sh/languages/
  languages.rust.enable = true;

  # https://devenv.sh/services/
  # Example server used by the bru files in test/yabruc-bruno
  services.nginx = {
    enable = true;
    httpConfig = ''
      server {
        listen 1234;
        location / {
          return 200 "Hello, world!";
        }
        location /error {
          return 500 "Internal server error";
        }
      }
    '';
  };

  # https://devenv.sh/scripts/
  # Serves the interactive guide of the parser: devenv shell parser-guide [port]
  scripts.parser-guide = {
    description = "Serve tools/parser-guide with nginx (default port 8080)";
    exec = ''
      port="''${1:-8080}"
      prefix="$DEVENV_STATE/parser-guide"
      mkdir -p "$prefix/tmp"
      cat > "$prefix/nginx.conf" <<EOF
      daemon off;
      pid $prefix/nginx.pid;
      error_log stderr;
      events {}
      http {
        include ${pkgs.nginx}/conf/mime.types;
        access_log off;
        client_body_temp_path $prefix/tmp/client_body;
        proxy_temp_path $prefix/tmp/proxy;
        fastcgi_temp_path $prefix/tmp/fastcgi;
        uwsgi_temp_path $prefix/tmp/uwsgi;
        scgi_temp_path $prefix/tmp/scgi;
        server {
          listen 127.0.0.1:$port;
          root $DEVENV_ROOT/tools/parser-guide;
          index index.html;
          add_header Cache-Control no-store;
        }
      }
      EOF
      echo "📖 Parser guide at http://localhost:$port (Ctrl+C to stop)"
      exec ${pkgs.nginx}/bin/nginx -p "$prefix" -c "$prefix/nginx.conf"
    '';
  };

  # Checks that tools/parser-guide/parser.js still behaves like the Rust parser
  scripts.parser-guide-check = {
    description = "Compare tools/parser-guide/parser.js with the Rust parser";
    exec = ''
      set -e
      export CARGO_TARGET_DIR="$DEVENV_ROOT/target"
      cargo build --release --quiet --manifest-path "$DEVENV_ROOT/tools/parser-guide/difftest/Cargo.toml"
      bun "$DEVENV_ROOT/tools/parser-guide/difftest.js" "$CARGO_TARGET_DIR/release/difftest"
    '';
  };

  # https://devenv.sh/tests/
  enterTest = ''
    wait_for_port 1234
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test
    cargo run -- run test/yabruc-bruno
  '';

  # See full reference at https://devenv.sh/reference/options/
}
