{
  description = "OneClient (Polyfrost OneLauncher) pour NixOS";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
        config.allowUnfree = true;
      };

      oneclient = pkgs.callPackage ./nix/package.nix { };
    in
    {
      packages.${system} = {
        inherit oneclient;
        default = oneclient;
      };

      apps.${system}.default = {
        type = "app";
        program = "${oneclient}/bin/oneclient";
      };

      overlays.default = final: prev: {
        oneclient = final.callPackage ./nix/package.nix { };
      };
    };
}
