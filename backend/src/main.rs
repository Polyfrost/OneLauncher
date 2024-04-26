use actix_web::{web::Data, App, HttpServer};
use log::info;

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
	dotenvy::dotenv().ok();
	env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

	info!(
		"starting polyfrost api on {}",
		dotenvy::var("BIND_ADDR").unwrap()
	);

	HttpServer::new(|| {
		let mut headers = reqwest::header::HeaderMap::new();
		let header = reqwest::header::HeaderValue::from_str(&format!(
			"onelauncher/{} (polyfrost.org)",
			env!("CARGO_PKG_VERSION")
		))
		.unwrap();
		headers.insert(reqwest::header::USER_AGENT, header);

		App::new()
			.app_data(Data::new(state::AppState {
				public_maven_url: dotenvy::var("PUBLIC_MAVEN_URL")
					.unwrap_or("https://repo.polyfrost.org".to_string()),
				internal_maven_url: dotenvy::var("INTERNAL_MAVEN_URL")
					.unwrap_or("http://localhost:8000".to_string()),
				http_client: reqwest::Client::builder()
					.tcp_keepalive(Some(std::time::Duration::from_secs(15)))
					.default_headers(headers)
					.build()
					.expect("failed to build reqwest client!"),
			}))
			.service(index)
			.service(oneconfig)
	})
	.bind(dotenvy::var("BIND_ADDR").unwrap())?
	.run()
	.await
}
