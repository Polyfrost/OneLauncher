use std::sync::{OnceLock, RwLock};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::sync::{Notify, watch};

use crate::service::RequestClient;

const CONNECTIVITY_URL: &str = "https://www.gstatic.com/generate_204";
const MC_AUTH_URL: &str = "https://api.minecraftservices.com/";
const POLYFROST_STATUS_URL: &str = "https://status.polyfrost.org/index.json";

const PROBE_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServiceStatus {
    pub online: bool,
    pub mc_auth_up: bool,
    pub polyfrost_up: bool,
}

impl Default for ServiceStatus {
    fn default() -> Self {
        Self {
            online: true,
            mc_auth_up: true,
            polyfrost_up: true,
        }
    }
}

struct Connectivity {
    tx: watch::Sender<ServiceStatus>,
    rx: watch::Receiver<ServiceStatus>,
    notify: Notify,
    started: AtomicBool,
    /// While set probe results are never published so the forced status
    /// survives a recheck
    forced: RwLock<Option<ServiceStatus>>,
}

fn connectivity() -> &'static Connectivity {
    static CONNECTIVITY: OnceLock<Connectivity> = OnceLock::new();
    CONNECTIVITY.get_or_init(|| {
        let (tx, rx) = watch::channel(ServiceStatus::default());
        Connectivity {
            tx,
            rx,
            notify: Notify::new(),
            started: AtomicBool::new(false),
            forced: RwLock::new(None),
        }
    })
}

/// Pins the reported status to `status` or releases the pin with `None` and
/// re-probes
pub fn force(status: Option<ServiceStatus>) {
    let conn = connectivity();
    *conn.forced.write().unwrap_or_else(|e| e.into_inner()) = status;

    match status {
        Some(status) => {
            let _ = conn.tx.send(status);
        }
        None => request_recheck(),
    }
}

#[must_use]
pub fn forced() -> Option<ServiceStatus> {
    *connectivity()
        .forced
        .read()
        .unwrap_or_else(|e| e.into_inner())
}

pub fn subscribe() -> watch::Receiver<ServiceStatus> {
    connectivity().rx.clone()
}

pub fn current() -> ServiceStatus {
    *connectivity().rx.borrow()
}

pub fn request_recheck() {
    connectivity().notify.notify_one();
}

pub fn note_request_result(success: bool) {
    let online = connectivity().rx.borrow().online;
    if !success || !online {
        request_recheck();
    }
}

/// Idempotent later calls are ignored so the client handed in on the first
/// call is used for the process lifetime
pub fn start(client: RequestClient) {
    let conn = connectivity();
    if conn.started.swap(true, Ordering::SeqCst) {
        return;
    }

    tokio::spawn(async move {
        loop {
            let probed = check_service_status(&client).await;
            if forced().is_none() {
                let _ = conn.tx.send(probed);
            }
            conn.notify.notified().await;
        }
    });
}

async fn reachable(client: &reqwest::Client, url: &str) -> bool {
    client
        .get(url)
        .timeout(PROBE_TIMEOUT)
        .send()
        .await
        .is_ok()
}

pub async fn check_service_status(requester: &RequestClient) -> ServiceStatus {
    let client = requester.http();

    if !reachable(client, CONNECTIVITY_URL).await {
        return ServiceStatus {
            online: false,
            mc_auth_up: false,
            polyfrost_up: false,
        };
    }

    let mc_auth_up = reachable(client, MC_AUTH_URL).await;

    let polyfrost_up = match client
        .get(POLYFROST_STATUS_URL)
        .timeout(PROBE_TIMEOUT)
        .send()
        .await
    {
        Ok(resp) => resp
            .json::<serde_json::Value>()
            .await
            .ok()
            .and_then(|json| {
                json.get("data")?
                    .get("attributes")?
                    .get("aggregate_state")?
                    .as_str()
                    .map(|s| s == "operational")
            })
            .unwrap_or(true),
        Err(_) => false,
    };

    ServiceStatus {
        online: true,
        mc_auth_up,
        polyfrost_up,
    }
}
