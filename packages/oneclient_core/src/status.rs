use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::sync::{Notify, watch};

use crate::LauncherState;

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
        }
    })
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

pub fn start() {
    let conn = connectivity();
    if conn.started.swap(true, Ordering::SeqCst) {
        return;
    }

    tokio::spawn(async move {
        loop {
            if let Ok(state) = LauncherState::get() {
                let status = check_service_status(&state).await;
                let _ = conn.tx.send(status);
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

pub async fn check_service_status(state: &LauncherState) -> ServiceStatus {
    let client = state.services.requester.http();

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
