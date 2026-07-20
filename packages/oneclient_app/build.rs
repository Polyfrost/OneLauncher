fn main() {
    // Embeds the app icon into the Windows executable.
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("icons/icon.ico");
        res.compile()
            .expect("failed to embed Windows icon resource");
    }
}
