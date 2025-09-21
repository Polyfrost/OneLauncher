pub mod bundles;
pub mod clusters;

pub async fn initialize_oneclient() {
	clusters::init_clusters().await;
}
