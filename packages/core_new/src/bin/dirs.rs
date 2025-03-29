use onelauncher_new::store::Dirs;

#[tokio::main]
async fn main() {
	let dirs = Dirs::get().await.expect("couldn't initialize dirs");

	println!("Base dir: {:?}", dirs.base_dir());
	println!("Launcher logs dir: {:?}", dirs.launcher_logs_dir());
	println!("Metadata dir: {:?}", dirs.metadata_dir());
	println!("DB file: {:?}", dirs.db_file());
}