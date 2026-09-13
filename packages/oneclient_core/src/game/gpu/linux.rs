use std::path::{Path, PathBuf};

const INTEL_INTEGRATED: &str = "0000:00:02.0";

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
pub struct Gpu {
    pub pci_address: String,
    pub boot_vga: Option<bool>,
    pub driver: Driver,
    pub render_node: Option<PathBuf>,
    pub discrete: bool,
}

impl Gpu {
    fn nvidia_proprietary(&self) -> bool {
        self.driver == Driver::Nvidia
    }

    fn can_render(&self) -> bool {
        self.render_node.is_some() || self.nvidia_proprietary()
    }

    fn priority(&self) -> u32 {
        u32::from(self.discrete) * 2 + u32::from(self.boot_vga == Some(true))
    }
}

pub fn offload_env(gpus: &[Gpu]) -> Vec<(&'static str, String)> {
    let Some(best) = gpus
        .iter()
        .filter(|gpu| gpu.can_render())
        .max_by_key(|gpu| gpu.priority())
    else {
        return Vec::new();
    };

    if !best.discrete || best.boot_vga == Some(true) {
        return Vec::new();
    }

    let mut env = if best.nvidia_proprietary() {
        vec![
            ("__NV_PRIME_RENDER_OFFLOAD", "1".to_string()),
            ("__VK_LAYER_NV_optimus", "NVIDIA_only".to_string()),
            ("__GLX_VENDOR_LIBRARY_NAME", "nvidia".to_string()),
        ]
    } else {
        let prime = pci_tag(&best.pci_address).unwrap_or_else(|| "1".to_string());
        vec![("DRI_PRIME", prime)]
    };

    if let Some(icd) = best.driver.vulkan_icd() {
        env.push(("VK_LOADER_DRIVERS_SELECT", icd.to_string()));
    }

    env
}

#[cfg(target_os = "linux")]
pub fn detect() -> Vec<Gpu> {
    let mut gpus = read_pci_devices(Path::new("/sys/bus/pci/devices"));

    for gpu in &mut gpus {
        gpu.discrete = probe_discrete(gpu);
    }

    gpus
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

        if read_class(&device.join("class")).is_none_or(|class| class >> 16 != 0x03) {
            continue;
        }

        gpus.push(Gpu {
            pci_address: address.to_string(),
            boot_vga: read_flag(&device.join("boot_vga")),
            driver: Driver::from_name(read_link_name(&device.join("driver")).as_deref()),
            render_node: render_node(&device.join("drm")),
            discrete: false,
        });
    }

    gpus.sort_by(|a, b| a.pci_address.cmp(&b.pci_address));
    gpus
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

fn render_node(drm: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(drm).ok()?;

    entries.flatten().find_map(|entry| {
        let name = entry.file_name();
        name.to_str()?
            .starts_with("renderD")
            .then(|| Path::new("/dev/dri").join(&name))
    })
}

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

fn probe_discrete(gpu: &Gpu) -> bool {
    match gpu.driver {
        Driver::Nvidia => true,
        Driver::AmdGpu => answered(gpu, drm::amdgpu_is_discrete(gpu)),
        Driver::Xe => answered(gpu, drm::xe_is_discrete(gpu)),
        Driver::I915 => gpu.pci_address != INTEL_INTEGRATED,
        Driver::Nouveau | Driver::Radeon | Driver::Other => gpu.boot_vga == Some(false),
    }
}

fn answered(gpu: &Gpu, probe: std::io::Result<bool>) -> bool {
    probe.unwrap_or_else(|error| {
        tracing::warn!(
            card = gpu.pci_address,
            ?error,
            "no discreteness answer; assuming integrated"
        );
        false
    })
}

#[cfg(not(target_os = "linux"))]
mod drm {
    use super::Gpu;

    fn unavailable() -> std::io::Result<bool> {
        Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
    }

    pub fn amdgpu_is_discrete(_gpu: &Gpu) -> std::io::Result<bool> {
        unavailable()
    }

    pub fn xe_is_discrete(_gpu: &Gpu) -> std::io::Result<bool> {
        unavailable()
    }
}

#[cfg(target_os = "linux")]
mod drm {
    use std::fs::File;
    use std::os::fd::AsRawFd;

    use super::Gpu;

    const DRM_IOCTL_BASE: u32 = b'd' as u32;
    const DRM_COMMAND_BASE: u32 = 0x40;

    fn open(gpu: &Gpu) -> std::io::Result<Option<File>> {
        let Some(node) = &gpu.render_node else {
            return Ok(None);
        };

        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(node)
            .map(Some)
    }

    fn check(result: i32) -> std::io::Result<()> {
        if result == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    fn command_write<T>(file: &File, index: u32, data: &T) -> std::io::Result<()> {
        let request = libc::_IOW::<T>(DRM_IOCTL_BASE, DRM_COMMAND_BASE + index);
        check(unsafe { libc::ioctl(file.as_raw_fd(), request, std::ptr::from_ref(data)) })
    }

    fn command_read_write<T>(file: &File, index: u32, data: &mut T) -> std::io::Result<()> {
        let request = libc::_IOWR::<T>(DRM_IOCTL_BASE, DRM_COMMAND_BASE + index);
        check(unsafe { libc::ioctl(file.as_raw_fd(), request, std::ptr::from_mut(data)) })
    }

    pub fn amdgpu_is_discrete(gpu: &Gpu) -> std::io::Result<bool> {
        let Some(file) = open(gpu)? else {
            return Ok(false);
        };

        #[repr(C)]
        struct InfoDevice {
            _before_ids_flags: [u32; 34],
            ids_flags: u64,
        }

        #[repr(C)]
        struct Info {
            return_pointer: u64,
            return_size: u32,
            query: u32,
            _query_args: [u32; 4],
        }

        let mut result = InfoDevice {
            _before_ids_flags: [0; 34],
            ids_flags: 0,
        };

        const AMDGPU_INFO_DEV_INFO: u32 = 0x16;
        let query = Info {
            return_pointer: std::ptr::from_mut(&mut result) as u64,
            return_size: size_of_val(&result) as u32,
            query: AMDGPU_INFO_DEV_INFO,
            _query_args: [0; 4],
        };

        const DRM_AMDGPU_INFO: u32 = 0x05;
        command_write(&file, DRM_AMDGPU_INFO, &query)?;

        const AMDGPU_IDS_FLAGS_FUSION: u64 = 0x1;
        Ok(result.ids_flags & AMDGPU_IDS_FLAGS_FUSION == 0)
    }

    pub fn xe_is_discrete(gpu: &Gpu) -> std::io::Result<bool> {
        let Some(file) = open(gpu)? else {
            return Ok(false);
        };

        #[repr(C)]
        struct DeviceQuery {
            extensions: u64,
            query: u32,
            size: u32,
            data: u64,
            _reserved: [u64; 2],
        }

        const DRM_XE_DEVICE_QUERY: u32 = 0x00;
        const DRM_XE_DEVICE_QUERY_CONFIG: u32 = 0x2;

        let mut query = DeviceQuery {
            extensions: 0,
            query: DRM_XE_DEVICE_QUERY_CONFIG,
            size: 0,
            data: 0,
            _reserved: [0; 2],
        };

        command_read_write(&file, DRM_XE_DEVICE_QUERY, &mut query)?;

        const INFO_OFFSET: usize = 1;
        const CONFIG_FLAGS: usize = 1;
        let words = (query.size as usize).div_ceil(size_of::<u64>());

        if words <= INFO_OFFSET + CONFIG_FLAGS {
            tracing::warn!(size = query.size, "xe config too short to hold its flags");
            return Ok(false);
        }

        let mut data = vec![0_u64; words];
        query.data = data.as_mut_ptr() as u64;

        command_read_write(&file, DRM_XE_DEVICE_QUERY, &mut query)?;

        const DRM_XE_QUERY_CONFIG_FLAG_HAS_VRAM: u64 = 1 << 0;
        Ok(data[INFO_OFFSET + CONFIG_FLAGS] & DRM_XE_QUERY_CONFIG_FLAG_HAS_VRAM != 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AMD_INTEGRATED: &str = "0000:0c:00.0";

    fn gpu(address: &str, driver: Driver, boot_vga: Option<bool>) -> Gpu {
        let discrete = match driver {
            Driver::Nvidia => true,
            Driver::I915 => address != INTEL_INTEGRATED,
            _ => boot_vga == Some(false),
        };

        Gpu {
            pci_address: address.to_string(),
            boot_vga,
            driver,
            render_node: Some(PathBuf::from("/dev/dri/renderD128")),
            discrete,
        }
    }

    fn nvidia_offload() -> Vec<(&'static str, String)> {
        vec![
            ("__NV_PRIME_RENDER_OFFLOAD", "1".to_string()),
            ("__VK_LAYER_NV_optimus", "NVIDIA_only".to_string()),
            ("__GLX_VENDOR_LIBRARY_NAME", "nvidia".to_string()),
            ("VK_LOADER_DRIVERS_SELECT", "*nvidia*".to_string()),
        ]
    }

    fn dri_prime(card: &str, icd: &str) -> Vec<(&'static str, String)> {
        vec![
            ("DRI_PRIME", card.to_string()),
            ("VK_LOADER_DRIVERS_SELECT", icd.to_string()),
        ]
    }

    #[test]
    fn offload_env_names_the_right_card_for_every_pairing() {
        let igpu = || gpu(INTEL_INTEGRATED, Driver::I915, Some(true));
        let dgpu = || gpu("0000:01:00.0", Driver::Nvidia, Some(false));

        type Case = (&'static str, Vec<Gpu>, Vec<(&'static str, String)>);
        let amd = "*radeon*,*amd*";

        let cases: Vec<Case> = vec![
            ("no cards at all", vec![], vec![]),
            (
                "a lone amd card",
                vec![gpu("0000:01:00.0", Driver::AmdGpu, Some(true))],
                vec![],
            ),
            (
                "a lone nvidia card",
                vec![gpu("0000:01:00.0", Driver::Nvidia, Some(true))],
                vec![],
            ),
            ("a lone intel card", vec![igpu()], vec![]),
            (
                "an all-amd hybrid never names nvidia",
                vec![
                    gpu(AMD_INTEGRATED, Driver::AmdGpu, Some(true)),
                    gpu("0000:01:00.0", Driver::AmdGpu, Some(false)),
                ],
                dri_prime("pci-0000_01_00_0", amd),
            ),
            (
                "an intel + amd hybrid never names nvidia",
                vec![igpu(), gpu("0000:01:00.0", Driver::AmdGpu, Some(false))],
                dri_prime("pci-0000_01_00_0", amd),
            ),
            (
                "an optimus laptop gets the nvidia variables and no DRI_PRIME",
                vec![igpu(), dgpu()],
                nvidia_offload(),
            ),
            (
                "the same laptop on nouveau gets DRI_PRIME and no ICD filter",
                vec![igpu(), Gpu { driver: Driver::Nouveau, ..dgpu() }],
                vec![("DRI_PRIME", "pci-0000_01_00_0".to_string())],
            ),
            (
                "an nvidia card without a render node is still a target",
                vec![igpu(), Gpu { render_node: None, ..dgpu() }],
                nvidia_offload(),
            ),
            (
                "an unbound card is never the target",
                vec![igpu(), Gpu { driver: Driver::Other, render_node: None, ..dgpu() }],
                vec![],
            ),
            (
                "a bmc display adapter is skipped for the real card behind it",
                vec![
                    igpu(),
                    Gpu { render_node: None, ..gpu("0000:03:00.0", Driver::Other, Some(false)) },
                    gpu("0000:c1:00.0", Driver::AmdGpu, Some(false)),
                ],
                dri_prime("pci-0000_c1_00_0", amd),
            ),
            (
                "the discrete card wins over a renderable card at a lower address",
                vec![
                    igpu(),
                    Gpu { discrete: false, ..gpu("0000:03:00.0", Driver::AmdGpu, Some(false)) },
                    gpu("0000:c1:00.0", Driver::AmdGpu, Some(false)),
                ],
                dri_prime("pci-0000_c1_00_0", amd),
            ),
            (
                "nothing happens when the discrete card already drives the display",
                vec![
                    gpu("0000:01:00.0", Driver::Nvidia, Some(true)),
                    gpu(INTEL_INTEGRATED, Driver::I915, Some(false)),
                ],
                vec![],
            ),
            (
                "the driver still finds the discrete card when nothing publishes boot_vga",
                vec![
                    gpu(INTEL_INTEGRATED, Driver::I915, None),
                    gpu("0000:01:00.0", Driver::Nvidia, None),
                ],
                nvidia_offload(),
            ),
            (
                "a malformed pci address falls back to the ordinal",
                vec![
                    gpu(AMD_INTEGRATED, Driver::AmdGpu, Some(true)),
                    gpu("not-an-address", Driver::AmdGpu, Some(false)),
                ],
                dri_prime("1", amd),
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

    #[test]
    fn a_driver_that_will_not_answer_is_never_called_discrete() {
        let mut apu = gpu(AMD_INTEGRATED, Driver::AmdGpu, Some(false));
        apu.discrete = probe_discrete(&apu);
        assert!(!apu.discrete);

        let mut dgpu = gpu("0000:03:00.0", Driver::AmdGpu, Some(true));
        dgpu.discrete = probe_discrete(&dgpu);
        assert!(!dgpu.discrete);

        assert!(
            offload_env(&[apu, dgpu]).is_empty(),
            "knowing nothing, leave the renderer alone"
        );
    }

    #[test]
    fn the_cards_we_can_place_without_asking_the_driver() {
        assert!(probe_discrete(&gpu("0000:01:00.0", Driver::Nvidia, Some(false))));
        assert!(!probe_discrete(&gpu(INTEL_INTEGRATED, Driver::I915, Some(true))));
        assert!(
            probe_discrete(&gpu("0000:03:00.0", Driver::I915, Some(false))),
            "an arc board does not sit at the integrated address"
        );
        assert!(
            probe_discrete(&gpu("0000:01:00.0", Driver::Nouveau, Some(false))),
            "no query, so boot_vga is all we have"
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

    fn write_card(bus: &Path, address: &str, boot_vga: &str, driver: &str) {
        let device = bus.join(address);
        write(&device.join("boot_vga"), &format!("{boot_vga}\n"));
        write(&device.join("class"), "0x030000\n");

        let target = bus.join("drivers").join(driver);
        std::fs::create_dir_all(&target).unwrap();
        std::os::unix::fs::symlink(&target, device.join("driver")).unwrap();
    }

    #[test]
    fn reads_boot_flag_driver_and_render_node_out_of_a_sysfs_tree() {
        let scratch = polyio::testing::ScratchDir::new("gpu-sysfs");
        let bus = scratch.join("devices");

        write_card(&bus, INTEL_INTEGRATED, "1", "i915");
        write(
            &bus.join(INTEL_INTEGRATED)
                .join("drm")
                .join("renderD128")
                .join("dev"),
            "226:128\n",
        );
        write_card(&bus, "0000:01:00.0", "0", "nvidia");

        let gpus = read_pci_devices(&bus);

        assert_eq!(gpus.len(), 2);
        assert_eq!(gpus[0].pci_address, INTEL_INTEGRATED);
        assert_eq!(gpus[0].boot_vga, Some(true));
        assert_eq!(gpus[0].driver, Driver::I915);
        assert_eq!(
            gpus[0].render_node,
            Some(PathBuf::from("/dev/dri/renderD128")),
            "the node the ioctl probes open"
        );
        assert_eq!(gpus[1].boot_vga, Some(false));
        assert_eq!(gpus[1].driver, Driver::Nvidia);
        assert_eq!(gpus[1].render_node, None, "no nvidia-drm in this tree");

        assert!(gpus.iter().all(|gpu| !gpu.discrete));
    }

    #[test]
    fn skips_everything_on_the_bus_that_is_not_a_display_controller() {
        let scratch = polyio::testing::ScratchDir::new("gpu-bus");
        let bus = scratch.join("devices");

        write_card(&bus, "0000:01:00.0", "1", "amdgpu");
        write(&bus.join("0000:01:00.1").join("class"), "0x040300\n");
        write(&bus.join("0000:02:00.0").join("class"), "0x020000\n");
        std::fs::create_dir_all(bus.join("0000:03:00.0")).unwrap();

        let gpus = read_pci_devices(&bus);

        assert_eq!(gpus.len(), 1, "one card, counted once");
        assert_eq!(gpus[0].driver, Driver::AmdGpu);
    }

    #[test]
    fn a_missing_pci_tree_is_not_an_error() {
        let scratch = polyio::testing::ScratchDir::new("gpu-missing");

        assert!(read_pci_devices(&scratch.join("nope")).is_empty());
    }
}
