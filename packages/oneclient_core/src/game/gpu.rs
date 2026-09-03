use std::path::Path;

const VENDOR_NVIDIA: u16 = 0x10de;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gpu {
    pub index: u32,
    pub vendor_id: u16,
    /// The card the firmware booted with
	/// `None` on the non-PCI platforms that never publish the attribute
    pub boot_vga: Option<bool>,
    pub pci_address: Option<String>,
}

/// Empty whenever offload does not apply
pub fn offload_env(gpus: &[Gpu]) -> Vec<(&'static str, String)> {
    if gpus.len() < 2 {
        return Vec::new();
    }

    let Some(boot) = gpus.iter().find(|gpu| gpu.boot_vga == Some(true)) else {
        return Vec::new();
    };

    if boot.vendor_id == VENDOR_NVIDIA {
        return Vec::new();
    }

    let Some(target) = gpus.iter().find(|gpu| gpu.boot_vga != Some(true)) else {
        return Vec::new();
    };

    let prime = target
        .pci_address
        .as_deref()
        .and_then(pci_tag)
        .unwrap_or_else(|| "1".to_string());

    let mut env = vec![("DRI_PRIME", prime)];

    // Only once the card we picked is actually NVIDIA's
    if target.vendor_id == VENDOR_NVIDIA {
        env.push(("__NV_PRIME_RENDER_OFFLOAD", "1".to_string()));
        env.push(("__VK_LAYER_NV_optimus", "NVIDIA_only".to_string()));
        env.push(("__GLX_VENDOR_LIBRARY_NAME", "nvidia".to_string()));
    }

    env
}

#[cfg(target_os = "linux")]
pub fn detect() -> Vec<Gpu> {
    read_drm_devices(Path::new("/sys/class/drm"))
}

fn read_drm_devices(root: &Path) -> Vec<Gpu> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };

    let mut gpus = Vec::new();

    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };

        // `card0` is a device; `card0-HDMI-A-1` is a connector hanging off it
        let Some(index) = name.strip_prefix("card").and_then(|n| n.parse::<u32>().ok()) else {
            continue;
        };

        let device = entry.path().join("device");
        let Some(vendor_id) = read_hex(&device.join("vendor")) else {
            continue;
        };

        gpus.push(Gpu {
            index,
            vendor_id,
            boot_vga: read_flag(&device.join("boot_vga")),
            pci_address: std::fs::read_link(&device)
                .ok()
                .and_then(|link| link.file_name()?.to_str().map(str::to_string)),
        });
    }

    gpus.sort_by_key(|gpu| gpu.index);
    gpus
}

fn read_hex(path: &Path) -> Option<u16> {
    let raw = std::fs::read_to_string(path).ok()?;
    let raw = raw.trim();
    u16::from_str_radix(raw.strip_prefix("0x").unwrap_or(raw), 16).ok()
}

fn read_flag(path: &Path) -> Option<bool> {
    Some(std::fs::read_to_string(path).ok()?.trim() == "1")
}

/// Mesa builds its tag as `pci-%04x_%02x_%02x_%1u`, which is the sysfs `0000:01:00.0` spelling with the separators swapped
fn pci_tag(address: &str) -> Option<String> {
    let (domain, rest) = address.split_once(':')?;
    let (bus, rest) = rest.split_once(':')?;
    let (device, function) = rest.split_once('.')?;

    let widths = [(domain, 4), (bus, 2), (device, 2), (function, 1)];
    if widths
        .iter()
        .any(|(part, width)| part.len() != *width || !part.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return None;
    }

    Some(format!("pci-{domain}_{bus}_{device}_{function}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const NVIDIA: u16 = 0x10de;
    const AMD: u16 = 0x1002;
    const INTEL: u16 = 0x8086;

    fn gpu(index: u32, vendor_id: u16, boot_vga: Option<bool>) -> Gpu {
        Gpu {
            index,
            vendor_id,
            boot_vga,
            pci_address: None,
        }
    }

    #[test]
    fn a_lone_card_is_never_offloaded_from() {
        for vendor in [AMD, NVIDIA, INTEL] {
            assert!(offload_env(&[gpu(0, vendor, Some(true))]).is_empty());
        }
        assert!(offload_env(&[]).is_empty());
    }

    #[test]
    fn an_all_amd_hybrid_never_names_nvidia() {
        let env = offload_env(&[gpu(0, AMD, Some(true)), gpu(1, AMD, Some(false))]);

        assert_eq!(env, vec![("DRI_PRIME", "1".to_string())]);
    }

    #[test]
    fn an_intel_amd_hybrid_never_names_nvidia() {
        let env = offload_env(&[gpu(0, INTEL, Some(true)), gpu(1, AMD, Some(false))]);

        assert_eq!(env, vec![("DRI_PRIME", "1".to_string())]);
    }

    #[test]
    fn an_optimus_laptop_gets_the_nvidia_variables() {
        let env = offload_env(&[gpu(0, INTEL, Some(true)), gpu(1, NVIDIA, Some(false))]);

        assert_eq!(
            env,
            vec![
                ("DRI_PRIME", "1".to_string()),
                ("__NV_PRIME_RENDER_OFFLOAD", "1".to_string()),
                ("__VK_LAYER_NV_optimus", "NVIDIA_only".to_string()),
                ("__GLX_VENDOR_LIBRARY_NAME", "nvidia".to_string()),
            ]
        );
    }

    #[test]
    fn nothing_happens_when_the_discrete_card_already_drives_the_display() {
        // Desktop with the monitor on the NVIDIA card and the iGPU still on.
        // `DRI_PRIME` would move Mesa onto the integrated one
        let env = offload_env(&[gpu(0, NVIDIA, Some(true)), gpu(1, INTEL, Some(false))]);

        assert!(env.is_empty());
    }

    #[test]
    fn nothing_happens_when_no_card_claims_to_be_the_boot_gpu() {
        let env = offload_env(&[gpu(0, INTEL, None), gpu(1, NVIDIA, None)]);

        assert!(env.is_empty());
    }

    #[test]
    fn the_target_card_is_named_by_pci_address_when_sysfs_offers_one() {
        let mut target = gpu(1, NVIDIA, Some(false));
        target.pci_address = Some("0000:01:00.0".to_string());

        let env = offload_env(&[gpu(0, INTEL, Some(true)), target]);

        assert_eq!(env[0], ("DRI_PRIME", "pci-0000_01_00_0".to_string()));
    }

    #[test]
    fn a_malformed_pci_address_falls_back_to_the_ordinal() {
        let mut target = gpu(1, AMD, Some(false));
        target.pci_address = Some("not-an-address".to_string());

        let env = offload_env(&[gpu(0, AMD, Some(true)), target]);

        assert_eq!(env[0], ("DRI_PRIME", "1".to_string()));
    }

    #[test]
    fn pci_tags_match_the_shape_mesa_builds() {
        assert_eq!(pci_tag("0000:01:00.0").as_deref(), Some("pci-0000_01_00_0"));
        assert_eq!(pci_tag("10000:af:1f.7").as_deref(), None, "domain is 4 wide");
        assert_eq!(pci_tag("0000:1:00.0"), None, "bus is 2 wide");
        assert_eq!(pci_tag("0000:01:00"), None, "no function");
        assert_eq!(pci_tag("0000:0g:00.0"), None, "not hex");
    }

    fn write(path: &Path, contents: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    #[test]
    fn reads_vendor_boot_flag_and_address_out_of_a_sysfs_tree() {
        let scratch = polyio::testing::ScratchDir::new("gpu-sysfs");
        let drm = scratch.join("drm");

        // sysfs spells these with a trailing newline
        write(&drm.join("card0").join("device").join("vendor"), "0x8086\n");
        write(&drm.join("card0").join("device").join("boot_vga"), "1\n");
        write(&drm.join("card1").join("device").join("vendor"), "0x10de\n");
        write(&drm.join("card1").join("device").join("boot_vga"), "0\n");

        let gpus = read_drm_devices(&drm);

        assert_eq!(gpus.len(), 2);
        assert_eq!(gpus[0].index, 0);
        assert_eq!(gpus[0].vendor_id, INTEL);
        assert_eq!(gpus[0].boot_vga, Some(true));
        assert_eq!(gpus[1].vendor_id, NVIDIA);
        assert_eq!(gpus[1].boot_vga, Some(false));

        // and the whole point: this pair earns the NVIDIA variables
        assert_eq!(offload_env(&gpus).len(), 4);
    }

    #[test]
    fn skips_connectors_and_anything_without_a_vendor() {
        let scratch = polyio::testing::ScratchDir::new("gpu-connectors");
        let drm = scratch.join("drm");

        write(&drm.join("card0").join("device").join("vendor"), "0x1002\n");
        // a connector, not a device
        write(
            &drm.join("card0-HDMI-A-1").join("device").join("vendor"),
            "0x1002\n",
        );
        // renderD128 sits alongside the cards and is the same device again
        write(&drm.join("renderD128").join("device").join("vendor"), "0x1002\n");
        // present but unreadable as a device
        std::fs::create_dir_all(drm.join("card9")).unwrap();

        let gpus = read_drm_devices(&drm);

        assert_eq!(gpus.len(), 1, "one card, counted once");
        assert_eq!(gpus[0].vendor_id, AMD);
    }

    #[test]
    fn a_missing_drm_tree_is_not_an_error() {
        let scratch = polyio::testing::ScratchDir::new("gpu-missing");

        assert!(read_drm_devices(&scratch.join("nope")).is_empty());
    }
}
