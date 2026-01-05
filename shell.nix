let
  rust-overlay = builtins.fetchTarball "https://github.com/oxalica/rust-overlay/archive/master.tar.gz";
  pkgs = import <nixpkgs> {
    overlays = [
      (import rust-overlay)
      (_: prev: {
        my-rust-toolchain = prev.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
      })
    ];
  };
in
pkgs.mkShell {
  strictDeps = true;
  nativeBuildInputs = with pkgs; [
    pkg-config
    gobject-introspection
    rustup
    cargo-tauri
    nodejs
    pnpm
    my-rust-toolchain
  ];

  buildInputs = with pkgs; [
    at-spi2-atk
    atkmm
    cairo
    gdk-pixbuf
    glib
    gtk3
    harfbuzz
    librsvg
    libsoup_3
    pango
    webkitgtk_4_1
    openssl
  ];
}
