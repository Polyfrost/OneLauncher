fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("icons/icon.ico");

        let version = env!("CARGO_PKG_VERSION");
        res.set("CompanyName", "RemixClient Contributors");
        res.set("ProductName", "RemixClient");
        res.set("FileDescription", "RemixClient");
        res.set(
            "LegalCopyright",
            "© 2026 RemixClient Contributors - GPL-3.0",
        );
        res.set("OriginalFilename", "oneclient_app.exe");
        res.set("InternalName", "oneclient_app");
        res.set("FileVersion", version);
        res.set("ProductVersion", version);

        res.compile()
            .expect("failed to embed Windows icon resource");
    }
}
