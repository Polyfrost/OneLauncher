#[macro_export]
macro_rules! collect_events {
	() => {{
		use onelauncher::api::proxy::*;
		tauri_specta::collect_events![
			ClusterPayload,
			IngressPayload,
			MessagePayload,
			ProcessPayload,
			InternetPayload,
			OfflinePayload,
		]
	}};
}
