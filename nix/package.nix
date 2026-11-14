{ lib
, appimageTools
, fetchurl
, makeDesktopItem
, makeWrapper
}:

let
  pname = "oneclient";
  version = "2.10.2";

  src = fetchurl {
    url = "https://github.com/Polyfrost/OneLauncher/releases/download/oneclient-${version}/OneClient_${version}_linux_x86_64.AppImage";
    hash = "sha256-fKuHG61AnHaNqDmG2G94Q546aWiNcsaE4bkdy6EJJXE=";
  };

  extracted = appimageTools.extract { inherit pname version src; };

  desktopItem = makeDesktopItem {
    name = pname;
    desktopName = "OneClient";
    comment = "Minecraft launcher by Polyfrost";
    exec = "${pname} %U";
    icon = pname;
    terminal = false;
    categories = [ "Game" ];
    mimeTypes = [ "x-scheme-handler/oneclient" ];
  };
in
appimageTools.wrapType2 {
  inherit pname version src;

  nativeBuildInputs = [ makeWrapper ];

  extraPkgs = pkgs: with pkgs; [
    libgpg-error
    libgcrypt
    vulkan-loader
    libxkbcommon
    wayland
    jdk8
    jdk17
    jdk21
    libGL
    openal
    libpulseaudio
    libxcursor
    libxrandr
    libxxf86vm
  ];

  extraInstallCommands = ''
    wrapProgram $out/bin/${pname} --unset GIO_EXTRA_MODULES

    install -Dm444 ${desktopItem}/share/applications/*.desktop \
      -t $out/share/applications

    find ${extracted} -maxdepth 2 -name '*.png' | head -n1 | while read -r icon; do
      install -Dm444 "$icon" $out/share/pixmaps/${pname}.png
    done
  '';

  meta = {
    description = "OneClient, Minecraft launcher by Polyfrost";
    homepage = "https://github.com/Polyfrost/OneLauncher";
    sourceProvenance = [ lib.sourceTypes.binaryNativeCode ];
    platforms = [ "x86_64-linux" ];
    mainProgram = pname;
  };
}
