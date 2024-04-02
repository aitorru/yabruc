{ pkgs, ... }:

{

  # https://devenv.sh/packages/
  packages = [ pkgs.openssl ];

  # https://devenv.sh/languages/
  # languages.nix.enable = true;
  languages.rust.enable = true;

  # https://devenv.sh/pre-commit-hooks/
  # pre-commit.hooks.shellcheck.enable = true;

  # https://devenv.sh/processes/
  # processes.ping.exec = "ping example.com";

  # See full reference at https://devenv.sh/reference/options/
}
