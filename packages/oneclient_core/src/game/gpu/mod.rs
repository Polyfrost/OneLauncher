#[cfg(any(target_os = "linux", test))]
mod linux;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gpu {
    pub id: String,
    pub name: String,
}

#[cfg(windows)]
const WINDOWS_PREFERENCES: [(&str, &str, u8); 2] = [
    ("integrated", "Integrated GPU", 1),
    ("dedicated", "Dedicated GPU", 2),
];

pub fn detect() -> Vec<Gpu> {
    cfg_select! {
        target_os = "linux" => linux::detect()
            .into_iter()
            .map(|card| Gpu {
                id: card.pci_address,
                name: card.name,
            })
            .collect(),
        windows => WINDOWS_PREFERENCES
            .iter()
            .map(|(id, name, _)| Gpu {
                id: (*id).to_string(),
                name: (*name).to_string(),
            })
            .collect(),
        _ => Vec::new(),
    }
}

#[allow(unused_variables)]
pub async fn select(command: &mut tokio::process::Command, java_path: &str, id: &str) {
    #[cfg(target_os = "linux")]
    {
        let cards = tokio::task::spawn_blocking(linux::detect)
            .await
            .unwrap_or_default();
        let Some(card) = cards.into_iter().find(|card| card.pci_address == id) else {
            tracing::warn!(id, "the selected GPU is gone; leaving the renderer alone");
            return;
        };

        for (key, value) in linux::offload_env(&card) {
            tracing::debug!(
                key,
                value,
                gpu = card.name,
                "rendering the game on the selected GPU"
            );
            command.env(key, value);
        }
    }

    #[cfg(windows)]
    {
        let Some((_, _, preference)) = WINDOWS_PREFERENCES.iter().find(|(known, ..)| *known == id)
        else {
            tracing::warn!(id, "unknown GPU preference; leaving the renderer alone");
            return;
        };

        oneclient_java::set_gpu_preference(std::path::Path::new(java_path), *preference).await;
    }
}
