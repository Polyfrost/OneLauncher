use crate::store::{IngressProcessType, IngressProcessor};

pub async fn check_ingress_feeds() -> crate::Result<bool> {
	IngressProcessor::finished(IngressProcessType::IngressFeed).await
}
