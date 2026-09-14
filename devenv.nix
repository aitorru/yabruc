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
