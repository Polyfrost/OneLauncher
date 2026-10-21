use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Driver {
    AmdGpu,
    Radeon,
    Nouveau,
    Nvidia,
    I915,
    Xe,
    Other,
}

impl Driver {
    fn from_name(name: Option<&str>) -> Self {
        match name {
            Some("amdgpu") => Self::AmdGpu,
            Some("radeon") => Self::Radeon,
            Some("nouveau") => Self::Nouveau,
            Some("nvidia") => Self::Nvidia,
            Some("i915") => Self::I915,
            Some("xe") => Self::Xe,
            Some(other) => {
                tracing::warn!(driver = other, "unknown graphics driver");
                Self::Other
            }
            None => Self::Other,
        }
    }

    fn vulkan_icd(self) -> Option<&'static str> {
        match self {
            Self::AmdGpu | Self::Radeon => Some("*radeon*,*amd*"),
            Self::Nvidia => Some("*nvidia*"),
            Self::I915 | Self::Xe => Some("*intel*"),
            Self::Nouveau | Self::Other => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub pci_address: String,
    pub name: String,
    pub driver: Driver,
}

pub fn offload_env(card: &Card) -> Vec<(&'static str, String)> {
    let mut env = if card.driver == Driver::Nvidia {
        vec![
            ("__NV_PRIME_RENDER_OFFLOAD", "1".to_string()),
            ("__VK_LAYER_NV_optimus", "NVIDIA_only".to_string()),
            ("__GLX_VENDOR_LIBRARY_NAME", "nvidia".to_string()),
        ]
    } else {
        let prime = format!("pci-{}", card.pci_address.replace([':', '.'], "_"));
        vec![("DRI_PRIME", prime)]
    };

    if let Some(icd) = card.driver.vulkan_icd() {
        env.push(("VK_LOADER_DRIVERS_SELECT", icd.to_string()));
    }

    env
}

#[cfg(target_os = "linux")]
pub fn detect() -> Vec<Card> {
    let ids = [
        "/usr/share/hwdata/pci.ids",
        "/usr/share/misc/pci.ids",
        "/usr/share/pci.ids",
    ]
    .iter()
    .find_map(|path| std::fs::read_to_string(path).ok())
    .unwrap_or_default();

    read_pci_devices(Path::new("/sys/bus/pci/devices"), &ids)
}

fn read_pci_devices(root: &Path, ids: &str) -> Vec<Card> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };

    let mut cards = Vec::new();

    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(address) = name.to_str() else {
            continue;
        };

        let device = entry.path();

        if read_hex(&device.join("class")).is_none_or(|class| class >> 16 != 0x03) {
            continue;
        }

        let driver = Driver::from_name(read_link_name(&device.join("driver")).as_deref());
        if driver != Driver::Nvidia && !has_render_node(&device.join("drm")) {
            continue;
        }

        let vendor = read_hex(&device.join("vendor")).unwrap_or(0);
        let model = read_hex(&device.join("device")).unwrap_or(0);

        cards.push(Card {
            pci_address: address.to_string(),
            name: display_name(ids, vendor, model, address),
            driver,
        });
    }

    cards.sort_by(|a, b| a.pci_address.cmp(&b.pci_address));
    cards
}

fn read_hex(path: &Path) -> Option<u32> {
    let raw = std::fs::read_to_string(path).ok()?;
    let raw = raw.trim();
    u32::from_str_radix(raw.strip_prefix("0x").unwrap_or(raw), 16).ok()
}

fn read_link_name(path: &Path) -> Option<String> {
    let link = std::fs::read_link(path).ok()?;
    Some(link.file_name()?.to_str()?.to_string())
}

fn has_render_node(drm: &Path) -> bool {
    std::fs::read_dir(drm).is_ok_and(|entries| {
        entries
            .flatten()
            .any(|entry| entry.file_name().to_string_lossy().starts_with("renderD"))
    })
}

fn display_name(ids: &str, vendor: u32, model: u32, address: &str) -> String {
    let brand = match vendor {
        0x10de => "NVIDIA",
        0x1002 => "AMD",
        0x8086 => "Intel",
        _ => "",
    };

    let model = pci_ids_model(ids, vendor, model).map_or_else(
        || format!("GPU {address}"),
        |name| {
            name.rsplit_once('[')
                .and_then(|(_, marketing)| marketing.strip_suffix(']'))
                .unwrap_or(name)
                .to_string()
        },
    );

    if brand.is_empty() {
        model
    } else {
        format!("{brand} {model}")
    }
}

fn pci_ids_model(ids: &str, vendor: u32, model: u32) -> Option<&str> {
    let vendor = format!("{vendor:04x}  ");
    let model = format!("\t{model:04x}  ");

    ids.lines()
        .skip_while(|line| !line.starts_with(&vendor))
        .skip(1)
        .take_while(|line| line.starts_with('\t') || line.starts_with('#'))
        .find_map(|line| line.strip_prefix(&model))
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDS: &str = "\
# comment
10de  NVIDIA Corporation
\t25a2  GA107M [GeForce RTX 3050 Mobile]
\t\t1043 1e1f  GA107M [GeForce RTX 3050 Mobile]
\t1234  Plain Name
1002  Advanced Micro Devices, Inc. [AMD/ATI]
\t25a2  Not The Nvidia Card
8086  Intel Corporation
";

    fn card(address: &str, driver: Driver) -> Card {
        Card {
            pci_address: address.to_string(),
            name: String::new(),
            driver,
        }
    }

    #[test]
    fn offload_env_names_the_chosen_card() {
        assert_eq!(
            offload_env(&card("0000:01:00.0", Driver::Nvidia)),
            vec![
                ("__NV_PRIME_RENDER_OFFLOAD", "1".to_string()),
                ("__VK_LAYER_NV_optimus", "NVIDIA_only".to_string()),
                ("__GLX_VENDOR_LIBRARY_NAME", "nvidia".to_string()),
                ("VK_LOADER_DRIVERS_SELECT", "*nvidia*".to_string()),
            ],
        );
        assert_eq!(
            offload_env(&card("0000:c1:00.0", Driver::AmdGpu)),
            vec![
                ("DRI_PRIME", "pci-0000_c1_00_0".to_string()),
                ("VK_LOADER_DRIVERS_SELECT", "*radeon*,*amd*".to_string()),
            ],
        );
        assert_eq!(
            offload_env(&card("0000:01:00.0", Driver::Nouveau)),
            vec![("DRI_PRIME", "pci-0000_01_00_0".to_string())],
            "nouveau gets no ICD filter",
        );
    }

    #[test]
    fn an_unknown_module_name_is_not_fatal() {
        assert_eq!(Driver::from_name(Some("asahi")), Driver::Other);
        assert_eq!(Driver::from_name(None), Driver::Other);
    }

    fn write(path: &Path, contents: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    fn write_card(bus: &Path, address: &str, vendor: &str, model: &str, driver: &str) {
        let device = bus.join(address);
        write(&device.join("class"), "0x030000\n");
        write(&device.join("vendor"), &format!("0x{vendor}\n"));
        write(&device.join("device"), &format!("0x{model}\n"));

        let target = bus.join("drivers").join(driver);
        std::fs::create_dir_all(&target).unwrap();
        std::os::unix::fs::symlink(&target, device.join("driver")).unwrap();
    }

    #[test]
    fn lists_the_cards_that_can_render_out_of_a_sysfs_tree() {
        let scratch = polyio::testing::ScratchDir::new("gpu-sysfs");
        let bus = scratch.join("devices");

        write_card(&bus, "0000:00:02.0", "8086", "9a49", "i915");
        write(
            &bus.join("0000:00:02.0")
                .join("drm")
                .join("renderD128")
                .join("dev"),
            "226:128\n",
        );
        write_card(&bus, "0000:01:00.0", "10de", "25a2", "nvidia");
        write_card(&bus, "0000:03:00.0", "1a03", "2000", "ast");
        write(&bus.join("0000:01:00.1").join("class"), "0x040300\n");
        write(&bus.join("0000:02:00.0").join("class"), "0x020000\n");
        std::fs::create_dir_all(bus.join("0000:04:00.0")).unwrap();

        let gpus = read_pci_devices(&bus, IDS);

        assert_eq!(
            gpus.len(),
            2,
            "no bmc adapter, audio, nic or classless device"
        );
        assert_eq!(gpus[0].pci_address, "0000:00:02.0");
        assert_eq!(gpus[0].driver, Driver::I915);
        assert_eq!(gpus[1].driver, Driver::Nvidia, "needs no render node");
        assert_eq!(gpus[1].name, "NVIDIA GeForce RTX 3050 Mobile");
    }

    #[test]
    fn a_missing_pci_tree_is_not_an_error() {
        let scratch = polyio::testing::ScratchDir::new("gpu-missing");

        assert!(read_pci_devices(&scratch.join("nope"), "").is_empty());
    }
}
