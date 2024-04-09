{ pkgs, ... }:

{

  # https://devenv.sh/packages/
  packages = [ pkgs.openssl pkgs.fzf pkgs.ripgrep pkgs.bat pkgs.bun pkgs.just ];

  # https://devenv.sh/languages/
  # languages.nix.enable = true;
  languages.rust.enable = true;

  # https://devenv.sh/pre-commit-hooks/
  # pre-commit.hooks.shellcheck.enable = true;

  # https://devenv.sh/processes/
  # processes.ping.exec = "ping example.com";
  services.nginx = {
    enable = true;
    httpConfig = ''
      server {
        listen 1234;
        location / {
          return 200 "Hello, world!";
        }
      }
    '';
  };

  enterTest = ''
    cargo run run .
    cargo test
  '';

  # See full reference at https://devenv.sh/reference/options/
}
