{ pkgs ? import <nixpkgs> {} }:
let
  nativeBuildInputs = [
    pkgs.pkg-config
    pkgs.cargo
    pkgs.cargo-flamegraph
    pkgs.rustc
    pkgs.rustfmt
    pkgs.lld_20
  ];
  buildInputs = [
    pkgs.openssl
    pkgs.alsa-lib
    pkgs.libxkbcommon
    pkgs.libGL
    pkgs.wayland
  ];
in
pkgs.mkShell {
  buildInputs = buildInputs;
  nativeBuildInputs = nativeBuildInputs;

  RUST_BACKTRACE = 1;
  TMPDIR = "/tmp";
  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath buildInputs;
}
