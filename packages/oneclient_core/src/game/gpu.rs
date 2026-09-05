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
    /// `device/class`; display controllers are `0x03xxxx`
    pub class: Option<u32>,
    pub has_render_node: bool,
}

impl Gpu {
    /// A server's BMC display adapter is a display controller with no render node, and naming it in `DRI_PRIME` drops the game to software rendering
    fn can_render(&self) -> bool {
        self.has_render_node && self.class.is_none_or(|class| class >> 16 == 0x03)
    }
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

    let Some(target) = gpus
        .iter()
        .find(|gpu| gpu.boot_vga != Some(true) && gpu.can_render())
    else {
        return Vec::new();
    };

    // `prime-run` sets only the NVIDIA variables; `DRI_PRIME` alongside them names a
    // card the proprietary driver owns and steers any Mesa path still in play onto nouveau
    if target.vendor_id == VENDOR_NVIDIA {
        return vec![
            ("__NV_PRIME_RENDER_OFFLOAD", "1".to_string()),
            ("__VK_LAYER_NV_optimus", "NVIDIA_only".to_string()),
            ("__GLX_VENDOR_LIBRARY_NAME", "nvidia".to_string()),
        ];
    }

    let prime = target
        .pci_address
        .as_deref()
        .and_then(pci_tag)
        .unwrap_or_else(|| "1".to_string());

    vec![("DRI_PRIME", prime)]
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
            class: read_class(&device.join("class")),
            has_render_node: has_render_node(&device.join("drm")),
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

/// sysfs spells the PCI class as `0x030000`
fn read_class(path: &Path) -> Option<u32> {
    let raw = std::fs::read_to_string(path).ok()?;
    let raw = raw.trim();
    u32::from_str_radix(raw.strip_prefix("0x").unwrap_or(raw), 16).ok()
}

/// The `drm` directory on the device holds its own `cardN` plus a `renderD*` whenever the card can be rendered on
fn has_render_node(drm: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(drm) else {
        return false;
    };

    entries.flatten().any(|entry| {
        entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with("renderD"))
    })
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
    const ASPEED: u16 = 0x1a03;

    fn gpu(index: u32, vendor_id: u16, boot_vga: Option<bool>) -> Gpu {
        Gpu {
            index,
            vendor_id,
            boot_vga,
            pci_address: None,
            class: Some(0x03_0000),
            has_render_node: true,
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
                ("__NV_PRIME_RENDER_OFFLOAD", "1".to_string()),
                ("__VK_LAYER_NV_optimus", "NVIDIA_only".to_string()),
                ("__GLX_VENDOR_LIBRARY_NAME", "nvidia".to_string()),
            ]
        );
        assert!(
            !env.iter().any(|(key, _)| *key == "DRI_PRIME"),
            "the proprietary driver owns the card DRI_PRIME would name"
        );
    }

    #[test]
    fn a_bmc_display_adapter_is_never_the_offload_target() {
        // Workstation: iGPU on the display, the BMC's ASPEED at a lower card index
        // than the real dGPU, and no render node on the BMC
        let mut bmc = gpu(1, ASPEED, Some(false));
        bmc.has_render_node = false;

        let mut dgpu = gpu(2, AMD, Some(false));
        dgpu.pci_address = Some("0000:c1:00.0".to_string());

        let env = offload_env(&[gpu(0, INTEL, Some(true)), bmc, dgpu]);

        assert_eq!(env, vec![("DRI_PRIME", "pci-0000_c1_00_0".to_string())]);
    }

    #[test]
    fn a_card_that_is_not_a_display_controller_is_never_the_offload_target() {
        let mut compute = gpu(1, AMD, Some(false));
        compute.class = Some(0x12_0000);

        assert!(offload_env(&[gpu(0, INTEL, Some(true)), compute]).is_empty());
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
        let mut target = gpu(1, AMD, Some(false));
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
        write(&drm.join("card0").join("device").join("class"), "0x030000\n");
        write(
            &drm.join("card0")
                .join("device")
                .join("drm")
                .join("renderD128")
                .join("dev"),
            "226:128\n",
        );
        write(&drm.join("card1").join("device").join("vendor"), "0x10de\n");
        write(&drm.join("card1").join("device").join("boot_vga"), "0\n");
        write(&drm.join("card1").join("device").join("class"), "0x030000\n");
        write(
            &drm.join("card1")
                .join("device")
                .join("drm")
                .join("renderD129")
                .join("dev"),
            "226:129\n",
        );

        let gpus = read_drm_devices(&drm);

        assert_eq!(gpus.len(), 2);
        assert_eq!(gpus[0].index, 0);
        assert_eq!(gpus[0].vendor_id, INTEL);
        assert_eq!(gpus[0].boot_vga, Some(true));
        assert_eq!(gpus[1].vendor_id, NVIDIA);
        assert_eq!(gpus[1].boot_vga, Some(false));
        assert_eq!(gpus[1].class, Some(0x03_0000));
        assert!(gpus[1].has_render_node);

        // and the whole point: this pair earns the NVIDIA variables
        assert_eq!(offload_env(&gpus).len(), 3);
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
