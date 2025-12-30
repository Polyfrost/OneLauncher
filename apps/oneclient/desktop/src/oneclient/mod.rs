pub mod bundles;
pub mod clusters;

pub async fn initialize_oneclient() {
	if let Err(err) = clusters::init_clusters().await {
		tracing::error!("failed to initialize clusters: {err}");
	}

	if let Err(err) = onelauncher_core::api::cluster::sync_clusters().await {
		tracing::error!("failed to sync clusters: {err}");
	}

	tokio::spawn(async {
		bundles::BundlesManager::get().await;
	});
}
