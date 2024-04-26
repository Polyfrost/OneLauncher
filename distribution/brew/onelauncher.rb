cask "onelauncher" do
    version "0.1.0"
    sha256 ""

    url "https://launcher.polyfrost.org/versions/#{version}/macos/OneLauncher_#{version}_universal.dmg"
    name "OneLauncher"
    desc "Next-generation open source Minecraft launcher"
    homepage "https://polyfrost.org"

    livecheck do
        url "https://polyfrost.org/launcher"
        regex("/OneLauncher[._-]v(\d+(?:\.\d+)+)[._-]universal\.dmg/i")
    end

    auto_updated true
    depends_on macos: ">= :high_sierra"

    app "OneLauncher.app"

    uninstall quit: "org.polyfrost.launcher"

    zap trash: [
        "~/Library/Application Support/org.polyfrost.launcher",
        "~/Library/Caches/org.polyfrost.launcher",
        "~/Library/Saved Application State/org.polyfrost.launcher.savedState",
        "~/Library/WebKit/org.polyfrost.launcher",
    ]
end
