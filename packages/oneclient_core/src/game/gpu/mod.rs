#[cfg(any(target_os = "linux", test))]
mod linux;

use tokio::process::Command;

#[allow(unused_variables)]
pub async fn prefer_discrete(command: &mut Command, java_path: &str) {
    #[cfg(target_os = "linux")]
    {
        let gpus = linux::detect();
        let env = linux::offload_env(&gpus);

        if env.is_empty() {
            tracing::info!(
                gpus = gpus.len(),
                "discrete GPU was requested but nothing here is a valid offload target; \
                 leaving the renderer alone"
            );
            return;
        }

        for (key, value) in env {
            tracing::debug!(key, value, "offloading the game to the discrete GPU");
            command.env(key, value);
        }
    }

    #[cfg(windows)]
    oneclient_java::prefer_dedicated_gpu(std::path::Path::new(java_path)).await;
}
