#[derive(rust_embed::RustEmbed)]
#[folder = "./assets/"]
pub struct AppAssets;

impl AppAssets {
    pub fn get_bytes(path: &str) -> Option<bytes::Bytes> {
        AppAssets::get(path).map(|file| match file.data {
            std::borrow::Cow::Borrowed(slice) => bytes::Bytes::copy_from_slice(slice),
            std::borrow::Cow::Owned(vec) => {
                tracing::warn!("Cloning asset data for '{}'", path);
                bytes::Bytes::from(vec)
            }
        })
    }
}
