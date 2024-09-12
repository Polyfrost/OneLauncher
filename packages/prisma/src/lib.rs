#![recursion_limit = "256"]
#[allow(warnings, unused)]
#[rustfmt::skip]
pub mod prisma;


pub async fn test_db() -> std::sync::Arc<prisma::PrismaClient> {
	std::sync::Arc::new(
		prisma::PrismaClient::_builder()
			.with_url(format!("file:/tmp/test-db-{}", uuid::Uuid::new_v4()))
			.build()
			.await
			.unwrap(),
	)
}
