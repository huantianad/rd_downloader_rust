{
  description = "rd_downloader";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";

    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";

    naersk.url = "github:nmattia/naersk";
    naersk.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, fenix, flake-utils, naersk, nixpkgs }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages."${system}";
        fenix-packages = fenix.packages.${system};
        naersk-lib = naersk.lib.${system}.override {
          inherit (fenix-packages.minimal) cargo rustc;
        };

        build-deps = with pkgs; [
          openssl_3
          pkg-config
        ];
      in
      rec {
        defaultPackage = naersk-lib.buildPackage {
          src = ./.;
          buildInputs = build-deps;
        };

        devShell = pkgs.mkShell {
          nativeBuildInputs = [
            (fenix-packages.complete.withComponents [
              "cargo"
              "clippy"
              "rust-src"
              "rustc"
              "rustfmt"
            ])
            fenix-packages.rust-analyzer
          ] ++ build-deps;
        };
      }
    );
}
