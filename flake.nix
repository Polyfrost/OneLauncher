{
  description = "OneClient is a Minecraft client featuring fully 100% open-source components, offering many packaged and pre-configured mods in one click. OneLauncher is a WIP Minecraft launcher giving power-users the greatest customization whilst featuring a clean UI.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
  }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
        };

        skiaSrc = pkgs.fetchFromGitHub {
          owner = "rust-skia";
          repo = "skia";
          rev = "m152-0.100.0";
          hash = "sha256-h1N5drad9FPGsdI1lzFWa5q2JDyAPuZ6w3ieCV6NtWs=";
        };

        skiaExternals = pkgs.linkFarm "skia-externals" (
          pkgs.lib.mapAttrsToList (name: value: {
            inherit name;
            path = pkgs.fetchgit value;
          }) (pkgs.lib.importJSON ./skia-externals.json)
        );

        skiaSource = pkgs.runCommand "skia-source" { } ''
          cp -R ${skiaSrc} $out
          chmod -R +w $out
          ln -s ${skiaExternals} $out/third_party/externals
        '';

        wrapperRuntimeLibs = with pkgs; [
          wayland
          libxkbcommon
          libX11
          libXcursor
          libXext
          libXi
          libXrandr
          libXfixes
          libXxf86vm
          libGL
          vulkan-loader
        ];

        desktopItem = pkgs.makeDesktopItem {
          name = "oneclient";
          desktopName = "OneClient";
          exec = "oneclient_app";
          icon = "oneclient";
          categories = [ "Game" ];
        };
      in
      {
        packages.default = (pkgs.rustPlatform.buildRustPackage.override {
          stdenv = pkgs.clangStdenv;
        }) {
          pname = "oneclient";
          version = "2.2.3";

          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = with pkgs; [
            pkg-config
            python3
            ninja
            gn
            cmake
            rustPlatform.bindgenHook
            makeWrapper
          ];

          buildInputs = with pkgs; [
            freetype
            fontconfig
            libGL
            libxkbcommon
            wayland
            libX11
            libXcursor
            libXext
            libXi
            libXrandr
            libXfixes
            libXxf86vm
          ];

          env = {
            SKIA_SOURCE_DIR = skiaSource;
            SKIA_GN_COMMAND = "${pkgs.gn}/bin/gn";
            SKIA_NINJA_COMMAND = "${pkgs.ninja}/bin/ninja";
          };

          buildPhase = ''
            runHook preBuild

            cargo build --release --locked -p oneclient_app

            runHook postBuild
          '';

          installPhase = ''
            runHook preInstall

            mkdir -p "$out/bin" "$out/share/applications"
            cp target/release/oneclient_app "$out/bin/oneclient_app"
            wrapProgram "$out/bin/oneclient_app" \
              --prefix LD_LIBRARY_PATH : "${pkgs.lib.makeLibraryPath wrapperRuntimeLibs}"
            cp ${desktopItem}/share/applications/*.desktop "$out/share/applications/"

            runHook postInstall
          '';

          meta = {
            description = "OneClient is a Minecraft client featuring fully 100% open-source components.";
            homepage = "https://polyfrost.org/projects/oneclient";
            license = pkgs.lib.licenses.gpl3Only;
            platforms = [ "x86_64-linux" ];
            mainProgram = "oneclient_app";
          };
        };
      });
}
