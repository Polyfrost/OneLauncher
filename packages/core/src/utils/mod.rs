pub mod crypto;
pub mod http;
pub mod icon;
pub mod io;
pub mod minecraft;
pub mod os_ext;
pub mod pagination;

#[async_trait::async_trait]
pub trait DatabaseModelExt {
	async fn path(&self) -> crate::LauncherResult<std::path::PathBuf>;
}
