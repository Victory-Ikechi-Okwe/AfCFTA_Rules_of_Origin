{ pkgs ? (import (fetchTarball "https://github.com/NixOS/nixpkgs/archive/nixos-unstable.tar.gz") {})}:

let
  localPackages = if builtins.pathExists ./local-packages.nix then
    (import ./local-packages.nix) pkgs
  else
    [ ];

  basePackages = with pkgs; [
    glab

    rustc
    cargo
    rustfmt
    clippy
    rust-analyzer

    bats
    jq

    parallel

    sqlite
  ];

  allPackages = basePackages ++ localPackages;
in
pkgs.mkShell {
  buildInputs = allPackages;
  RUST_BACKTRACE = "1";
  RUST_LOG = "debug";
}
