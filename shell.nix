{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell {
  buildInputs = [
      pkgs.pkg-config
      pkgs.cmake
      pkgs.clang
      pkgs.ninja

      pkgs.libiconv
      pkgs.shaderc

      pkgs.darwin.apple_sdk.frameworks.Metal
      pkgs.darwin.apple_sdk.frameworks.AppKit
      pkgs.darwin.apple_sdk.frameworks.Foundation
  ];

  nativeBuildInputs = [ ];
}