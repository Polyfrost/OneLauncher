use onelauncher_core::{api::{credentials, proxy::ProxyDynamic}, error::LauncherResult, initialize_core, store::CoreOptions, utils::io::IOError};
use tokio::io::AsyncBufReadExt;

#[tokio::main]
pub async fn main() -> LauncherResult<()> {
	initialize_core(CoreOptions::default(), ProxyDynamic::new()).await?;

	let flow = credentials::begin().await?;
	println!("{flow:#?}");

	let stdin = tokio::io::stdin();
	let mut reader = tokio::io::BufReader::new(stdin);
	let mut code = String::new();

	println!("Please enter the code you received:");
	reader.read_line(&mut code).await.map_err(IOError::from)?;
	println!("Please wait");
	code = code.trim().to_string();

	let creds = credentials::finish(&code, flow).await?;

	println!("Credentials: {creds:#?}");

	Ok(())
}