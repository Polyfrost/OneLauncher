use std::path::Path;

const VENDOR_NVIDIA: u16 = 0x10de;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gpu {
    /// The sysfs address, `0000:01:00.0`
    pub pci_address: String,
    pub vendor_id: u16,
    /// The card the firmware booted with
    /// `None` on the platforms that never publish the attribute
    pub boot_vga: Option<bool>,
    /// `device/class`; display controllers are `0x03xxxx`
    pub class: Option<u32>,
    /// The kernel module bound to the card, `nvidia` or `nouveau` for the same silicon
    pub driver: Option<String>,
    pub has_render_node: bool,
}

impl Gpu {
    /// The proprietary driver renders through `/dev/nvidia*`, so it is a target
    /// whether or not `nvidia-drm` is loaded to publish a render node
    fn nvidia_proprietary(&self) -> bool {
        self.driver.as_deref() == Some("nvidia")
    }

    /// A server's BMC display adapter is a display controller with no render node, and naming it in `DRI_PRIME` drops the game to software rendering
    fn can_render(&self) -> bool {
        (self.has_render_node || self.nvidia_proprietary())
            && self.class.is_none_or(|class| class >> 16 == 0x03)
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

    if target.nvidia_proprietary() {
        return vec![
            ("__NV_PRIME_RENDER_OFFLOAD", "1".to_string()),
            ("__VK_LAYER_NV_optimus", "NVIDIA_only".to_string()),
            ("__GLX_VENDOR_LIBRARY_NAME", "nvidia".to_string()),
        ];
    }

    let prime = pci_tag(&target.pci_address).unwrap_or_else(|| "1".to_string());

    vec![("DRI_PRIME", prime)]
}

#[cfg(target_os = "linux")]
pub fn detect() -> Vec<Gpu> {
    read_pci_devices(Path::new("/sys/bus/pci/devices"))
}

fn read_pci_devices(root: &Path) -> Vec<Gpu> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };

    let mut gpus = Vec::new();

    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(address) = name.to_str() else {
            continue;
        };

        let device = entry.path();
        let class = read_class(&device.join("class"));

        if class.is_none_or(|class| class >> 16 != 0x03) {
            continue;
        }

        let Some(vendor_id) = read_hex(&device.join("vendor")) else {
            continue;
        };

        gpus.push(Gpu {
            pci_address: address.to_string(),
            vendor_id,
            boot_vga: read_flag(&device.join("boot_vga")),
            class,
            driver: read_link_name(&device.join("driver")),
            has_render_node: has_render_node(&device.join("drm")),
        });
    }

    gpus.sort_by(|a, b| a.pci_address.cmp(&b.pci_address));
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

fn read_class(path: &Path) -> Option<u32> {
    let raw = std::fs::read_to_string(path).ok()?;
    let raw = raw.trim();
    u32::from_str_radix(raw.strip_prefix("0x").unwrap_or(raw), 16).ok()
}

fn read_link_name(path: &Path) -> Option<String> {
    let link = std::fs::read_link(path).ok()?;
    Some(link.file_name()?.to_str()?.to_string())
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

    fn gpu(address: &str, vendor_id: u16, boot_vga: Option<bool>) -> Gpu {
        Gpu {
            pci_address: address.to_string(),
            vendor_id,
            boot_vga,
            class: Some(0x03_0000),
            driver: Some(match vendor_id {
                NVIDIA => "nvidia",
                AMD => "amdgpu",
                INTEL => "i915",
                _ => "unknown",
            }
            .to_string()),
            has_render_node: true,
        }
    }

    fn nvidia_offload() -> Vec<(&'static str, String)> {
        vec![
            ("__NV_PRIME_RENDER_OFFLOAD", "1".to_string()),
            ("__VK_LAYER_NV_optimus", "NVIDIA_only".to_string()),
            ("__GLX_VENDOR_LIBRARY_NAME", "nvidia".to_string()),
        ]
    }

    fn dri_prime(card: &str) -> Vec<(&'static str, String)> {
        vec![("DRI_PRIME", card.to_string())]
    }

    /// The whole truth table `offload_env` decides over: vendor, which card the
    /// firmware booted, which module is bound, and whether there is a render node.
    /// Getting a row wrong is silent — the game lands on the wrong card or on
    /// llvmpipe and only ever gets reported as "it runs slow"
    #[test]
    fn offload_env_names_the_right_card_for_every_pairing() {
        let igpu = || gpu("0000:00:02.0", INTEL, Some(true));
        let dgpu = || gpu("0000:01:00.0", NVIDIA, Some(false));
        let nothing = Vec::new();

        let cases: Vec<(&str, Vec<Gpu>, Vec<(&'static str, String)>)> = vec![
            ("no cards at all", vec![], nothing.clone()),
            ("a lone amd card", vec![gpu("0000:01:00.0", AMD, Some(true))], nothing.clone()),
            ("a lone nvidia card", vec![gpu("0000:01:00.0", NVIDIA, Some(true))], nothing.clone()),
            ("a lone intel card", vec![igpu()], nothing.clone()),
            (
                "an all-amd hybrid never names nvidia",
                vec![gpu("0000:00:02.0", AMD, Some(true)), gpu("0000:01:00.0", AMD, Some(false))],
                dri_prime("pci-0000_01_00_0"),
            ),
            (
                "an intel + amd hybrid never names nvidia",
                vec![igpu(), gpu("0000:01:00.0", AMD, Some(false))],
                dri_prime("pci-0000_01_00_0"),
            ),
            (
                // `DRI_PRIME` alongside these names a card the proprietary driver
                // owns and steers any Mesa path still in play onto nouveau
                "an optimus laptop gets the nvidia variables and no DRI_PRIME",
                vec![igpu(), dgpu()],
                nvidia_offload(),
            ),
            (
                // `__GLX_VENDOR_LIBRARY_NAME=nvidia` would ask glvnd for a vendor
                // library that is not installed, and the game lands on llvmpipe
                "the same laptop on nouveau gets DRI_PRIME instead",
                vec![igpu(), Gpu { driver: Some("nouveau".to_string()), ..dgpu() }],
                dri_prime("pci-0000_01_00_0"),
            ),
            (
                // No `nvidia-drm`, so nothing publishes `renderD*` — the proprietary
                // driver renders through `/dev/nvidia*` all the same
                "an nvidia card without a render node is still a target",
                vec![igpu(), Gpu { has_render_node: false, ..dgpu() }],
                nvidia_offload(),
            ),
            (
                "an unbound card is never the target",
                vec![igpu(), Gpu { driver: None, has_render_node: false, ..dgpu() }],
                nothing.clone(),
            ),
            (
                // Workstation: iGPU on the display, the BMC's ASPEED at a lower
                // address than the real dGPU, and no render node on the BMC
                "a bmc display adapter is skipped for the real card behind it",
                vec![
                    igpu(),
                    Gpu { has_render_node: false, ..gpu("0000:03:00.0", ASPEED, Some(false)) },
                    gpu("0000:c1:00.0", AMD, Some(false)),
                ],
                dri_prime("pci-0000_c1_00_0"),
            ),
            (
                "a card that is not a display controller is never the target",
                vec![igpu(), Gpu { class: Some(0x12_0000), ..gpu("0000:01:00.0", AMD, Some(false)) }],
                nothing.clone(),
            ),
            (
                // Desktop with the monitor on the NVIDIA card and the iGPU still
                // on. `DRI_PRIME` would move Mesa onto the integrated one
                "nothing happens when the discrete card already drives the display",
                vec![gpu("0000:01:00.0", NVIDIA, Some(true)), gpu("0000:00:02.0", INTEL, Some(false))],
                nothing.clone(),
            ),
            (
                "nothing happens when no card claims to be the boot gpu",
                vec![gpu("0000:00:02.0", INTEL, None), gpu("0000:01:00.0", NVIDIA, None)],
                nothing.clone(),
            ),
            (
                "a malformed pci address falls back to the ordinal",
                vec![gpu("0000:00:02.0", AMD, Some(true)), gpu("not-an-address", AMD, Some(false))],
                dri_prime("1"),
            ),
        ];

        for (name, gpus, expected) in cases {
            assert_eq!(offload_env(&gpus), expected, "{name}");
        }
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

    fn write_card(bus: &Path, address: &str, vendor: &str, boot_vga: &str, driver: &str) {
        let device = bus.join(address);
        // sysfs spells these with a trailing newline
        write(&device.join("vendor"), &format!("{vendor}\n"));
        write(&device.join("boot_vga"), &format!("{boot_vga}\n"));
        write(&device.join("class"), "0x030000\n");

        let target = bus.join("drivers").join(driver);
        std::fs::create_dir_all(&target).unwrap();
        std::os::unix::fs::symlink(&target, device.join("driver")).unwrap();
    }

    #[test]
    fn reads_vendor_boot_flag_driver_and_render_node_out_of_a_sysfs_tree() {
        let scratch = polyio::testing::ScratchDir::new("gpu-sysfs");
        let bus = scratch.join("devices");

        write_card(&bus, "0000:00:02.0", "0x8086", "1", "i915");
        write(
            &bus.join("0000:00:02.0")
                .join("drm")
                .join("renderD128")
                .join("dev"),
            "226:128\n",
        );
        write_card(&bus, "0000:01:00.0", "0x10de", "0", "nvidia");

        let gpus = read_pci_devices(&bus);

        assert_eq!(gpus.len(), 2);
        assert_eq!(gpus[0].pci_address, "0000:00:02.0");
        assert_eq!(gpus[0].vendor_id, INTEL);
        assert_eq!(gpus[0].boot_vga, Some(true));
        assert!(gpus[0].has_render_node);
        assert_eq!(gpus[1].vendor_id, NVIDIA);
        assert_eq!(gpus[1].boot_vga, Some(false));
        assert_eq!(gpus[1].class, Some(0x03_0000));
        assert_eq!(gpus[1].driver.as_deref(), Some("nvidia"));
        assert!(!gpus[1].has_render_node, "no nvidia-drm in this tree");

        // and the whole point: this pair earns the NVIDIA variables
        assert_eq!(offload_env(&gpus).len(), 3);
    }

    #[test]
    fn skips_everything_on_the_bus_that_is_not_a_display_controller() {
        let scratch = polyio::testing::ScratchDir::new("gpu-bus");
        let bus = scratch.join("devices");

        write_card(&bus, "0000:01:00.0", "0x1002", "1", "amdgpu");
        // the audio function of that same card
        let audio = bus.join("0000:01:00.1");
        write(&audio.join("vendor"), "0x1002\n");
        write(&audio.join("class"), "0x040300\n");
        // a network card
        let net = bus.join("0000:02:00.0");
        write(&net.join("vendor"), "0x8086\n");
        write(&net.join("class"), "0x020000\n");
        // present but unreadable as a device
        std::fs::create_dir_all(bus.join("0000:03:00.0")).unwrap();

        let gpus = read_pci_devices(&bus);

        assert_eq!(gpus.len(), 1, "one card, counted once");
        assert_eq!(gpus[0].vendor_id, AMD);
    }

    #[test]
    fn a_missing_pci_tree_is_not_an_error() {
        let scratch = polyio::testing::ScratchDir::new("gpu-missing");

        assert!(read_pci_devices(&scratch.join("nope")).is_empty());
    }
}
