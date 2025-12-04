mod events;
pub use events::*;

mod commands;
pub use commands::*;

mod folders;
pub use folders::*;

mod debug;
pub use debug::*;

pub trait TauRPCLauncherExt {
	fn use_launcher_api(self) -> Self;
}

impl<R: tauri::Runtime> TauRPCLauncherExt for taurpc::Router<R> {
	fn use_launcher_api(self) -> Self {
		self.merge(TauriLauncherApiImpl.into_handler())
			.merge(TauriLauncherEventApiImpl.into_handler())
			.merge(TauriLauncherFoldersApiImpl.into_handler())
			.merge(TauriLauncherDebugApiImpl.into_handler())
	}
}
