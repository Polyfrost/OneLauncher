use onelauncher_core::store::Dirs;

#[tokio::main]
async fn main() {
	let dirs = Dirs::get().await.expect("couldn't initialize dirs");

	println!("         Base dir : {}", dirs.base_dir().display());
	println!("Launcher logs dir : {}", dirs.launcher_logs_dir().display());
	println!("     Metadata dir : {}", dirs.metadata_dir().display());
	println!("          DB file : {}", dirs.db_file().display());
}