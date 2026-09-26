{
  description = "Rust dev environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    serpentine = {
      url = "github:Serpent-Tools/serpentine/v1.0.1";
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
      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          packages = [
            (pkgs.fenix.latest.withComponents [
              "cargo"
              "clippy"
              "rustc"

              "rust-analyzer"
              "rustfmt"

              "rust-src"
            ])
            pkgs.serpentine
            pkgs.just

            pkgs.wasm-bindgen-cli_0_2_100
            pkgs.binaryen
          ];
        };
      });
    };
}
