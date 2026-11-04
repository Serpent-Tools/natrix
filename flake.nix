{
  description = "Rust dev environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    serpentine = {
      url = "github:Serpent-Tools/serpentine/v1.0.4";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      nixpkgs,
      fenix,
      serpentine,
      ...
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems =
        function:
        nixpkgs.lib.genAttrs systems (
          system:
          function (
            import nixpkgs {
              inherit system;
              overlays = [
                fenix.overlays.default
                serpentine.overlays.default
              ];
            }
          )
        );
    in
    {
      devShells = forAllSystems (
        pkgs:
        let
          wasm-bindgen-cli = pkgs.buildWasmBindgenCli rec {
            src = pkgs.fetchCrate {
              pname = "wasm-bindgen-cli";
              version = "0.2.130";
              hash = "sha256-9Og1O/6/VC3sc5hBjoIWBErmLW2UHGMsTQO+6+m/dng=";
            };
            cargoDeps = pkgs.rustPlatform.fetchCargoVendor {
              inherit src;
              inherit (src) pname version;
              hash = "sha256-j81sUf1m5yXU4FJfGj7GV+hdqVCV8mRmNb0QIsMCjqw=";
            };
          };
        in
        {
          default = pkgs.mkShell {
            packages = [
              (pkgs.fenix.combine [
                (pkgs.fenix.latest.withComponents [
                  "cargo"
                  "clippy"
                  "rustc"

                  "rust-analyzer"
                  "rustfmt"

                  "rust-src"
                ])
                pkgs.fenix.targets.wasm32-unknown-unknown.latest.rust-std
              ])
              pkgs.serpentine
              pkgs.just

              wasm-bindgen-cli
              pkgs.binaryen
              pkgs.chromedriver
            ];
          };
        }
      );
    };
}
