use std::{ops::Deref, sync::Arc};

use anyhow::anyhow;
use tokio::sync::OnceCell;

use crate::{api::proxy::LauncherProxy, LauncherResult};

static PROXY_STATE: OnceCell<Arc<ProxyState>> = OnceCell::const_new();

pub struct ProxyState {
	inner: Box<dyn LauncherProxy>,
}

impl Deref for ProxyState {
	type Target = Box<dyn LauncherProxy>;

	fn deref(&self) -> &Self::Target {
		&self.inner
	}
}

impl ProxyState {
	#[tracing::instrument(level = "debug")]
	pub async fn initialize(proxy: impl LauncherProxy + 'static) -> LauncherResult<Arc<Self>> {
		PROXY_STATE
			.get_or_try_init(|| async {
				Ok(Arc::new(Self {
					inner: Box::new(proxy),
				}))
			})
			.await
			.cloned()
	}

	pub fn get() -> LauncherResult<Arc<Self>> {
		Ok(PROXY_STATE
			.get()
			.ok_or_else(|| anyhow!("proxy state not initialized"))
			.cloned()?)
	}
}