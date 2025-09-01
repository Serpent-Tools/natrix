{
  description = "Rust dev environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
    nur = {
      url = "github:nix-community/NUR";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      fenix,
      flake-utils,
      nur,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ nur.overlays.default ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            (fenix.packages.${system}.latest.withComponents [
              "cargo"
              "clippy"
              "rustc"

              "rust-analyzer"
              "rustfmt"

              "rust-src"
            ])
            pkgs.nur.repos.dagger.dagger
            just

            wasm-bindgen-cli_0_2_100
            binaryen
          ];
        };
      }
    );
}
