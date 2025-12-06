cask "oneclient" do
    version "0.1.0"
    sha256 ""

    url "https://oneclient.polyfrost.org/versions/#{version}/macos/OneClient_#{version}_universal.dmg"
    name "OneClient"
    desc "Next-generation open source Minecraft launcher"
    homepage "https://polyfrost.org"

    livecheck do
        url "https://polyfrost.org/projects/oneclient"
        regex("/OneClient[._-]v(\d+(?:\.\d+)+)[._-]universal\.dmg/i")
    end

    auto_updated true
    depends_on macos: ">= :high_sierra"

    app "OneClient.app"

    uninstall quit: "OneClient"

    zap trash: [
        "~/Library/Application Support/OneClient",
        "~/Library/Caches/OneClient",
        "~/Library/Saved Application State/OneClient.savedState",
        "~/Library/WebKit/OneClient",
    ]
end
