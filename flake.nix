{
  description = "OneLauncher: Next-generation open source Minecraft launcher";

  inputs.nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";

  inputs.crane.url = "github:ipetkov/crane";
  inputs.crane.inputs.nixpkgs.follows = "nixpkgs";

  inputs.fenix.url = "github:nix-community/fenix";
  inputs.fenix.inputs.nixpkgs.follows = "nixpkgs";

  inputs.utils.url = "github:numtide/flake-utils";
  inputs.utils.inputs.nixpkgs.follows = "nixpkgs";

  inputs.rust.url = "github:numtide/rust-overlay";
  inputs.rust.inputs.nixpkgs.follows = "nixpkgs";

  inputs.advisory.url = "github:rustsec/advisory-db";
  inputs.advisory.flake = false;

  outputs = { self, nixpkgs, crane, fenix, utils, rust, advisory, ... }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        inherit (pkgs) lib;
        
        craneLib = crane.lib.${system};
        cargoSrc = craneLib.cleanCargoSource (craneLib.path ./.);
        arguments = {
          inherit cargoSrc;
          strictDeps = true;

        };
      in {
        
      }
    );
}
