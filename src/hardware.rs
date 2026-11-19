use rand::seq::SliceRandom;
use rand::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuBrand {
    Intel,
    Amd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwareTier {
    Entry,
    Mid,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuKind {
    IntelIgpu,
    AmdIgpu,
    Discrete,
}

#[derive(Debug, Clone)]
pub struct CpuSpec {
    pub name: &'static str,
    pub threads: u32,
    pub ram: u32,
    pub brand: CpuBrand,
    pub has_igpu: bool,
    pub tier: HardwareTier,
}

#[derive(Debug, Clone)]
pub struct GpuSpec {
    pub name: &'static str,
    pub vendor: &'static str,
    pub renderer: &'static str,
    pub kind: GpuKind,
    pub tier: HardwareTier,
}

/// Returns a valid Web API `navigator.deviceMemory` value (capped at 8 per W3C spec).
pub fn web_device_memory(ram_gb: u32) -> u32 {
    match ram_gb {
        0..=2 => 2,
        3..=5 => 4,
        _ => 8,
    }
}

pub const CPU_POOL: &[CpuSpec] = &[
    CpuSpec { name: "Intel Core i3-10100", threads: 8, ram: 8, brand: CpuBrand::Intel, has_igpu: true, tier: HardwareTier::Entry },
    CpuSpec { name: "Intel Core i3-12100", threads: 8, ram: 16, brand: CpuBrand::Intel, has_igpu: true, tier: HardwareTier::Entry },
    CpuSpec { name: "Intel Core i5-10400F", threads: 12, ram: 16, brand: CpuBrand::Intel, has_igpu: false, tier: HardwareTier::Mid },
    CpuSpec { name: "Intel Core i5-11400", threads: 12, ram: 16, brand: CpuBrand::Intel, has_igpu: true, tier: HardwareTier::Mid },
    CpuSpec { name: "Intel Core i5-1135G7", threads: 8, ram: 8, brand: CpuBrand::Intel, has_igpu: true, tier: HardwareTier::Entry },
    CpuSpec { name: "Intel Core i7-1165G7", threads: 8, ram: 16, brand: CpuBrand::Intel, has_igpu: true, tier: HardwareTier::Entry },
    CpuSpec { name: "Intel Core i5-8250U", threads: 8, ram: 8, brand: CpuBrand::Intel, has_igpu: true, tier: HardwareTier::Entry },
    CpuSpec { name: "Intel Core i7-8550U", threads: 8, ram: 8, brand: CpuBrand::Intel, has_igpu: true, tier: HardwareTier::Entry },
    CpuSpec { name: "Intel Core i3-1115G4", threads: 4, ram: 8, brand: CpuBrand::Intel, has_igpu: true, tier: HardwareTier::Entry },
    CpuSpec { name: "AMD Ryzen 3 3200G", threads: 4, ram: 8, brand: CpuBrand::Amd, has_igpu: true, tier: HardwareTier::Entry },
    CpuSpec { name: "AMD Ryzen 3 4300G", threads: 8, ram: 8, brand: CpuBrand::Amd, has_igpu: true, tier: HardwareTier::Entry },
    CpuSpec { name: "AMD Ryzen 5 3500X", threads: 6, ram: 16, brand: CpuBrand::Amd, has_igpu: false, tier: HardwareTier::Mid },
    CpuSpec { name: "AMD Ryzen 5 2600", threads: 12, ram: 16, brand: CpuBrand::Amd, has_igpu: false, tier: HardwareTier::Mid },
    CpuSpec { name: "AMD Ryzen 5 1600", threads: 12, ram: 16, brand: CpuBrand::Amd, has_igpu: false, tier: HardwareTier::Entry },
    CpuSpec { name: "AMD Ryzen 5 3600", threads: 12, ram: 16, brand: CpuBrand::Amd, has_igpu: false, tier: HardwareTier::Mid },
    CpuSpec { name: "AMD Ryzen 5 5600X", threads: 12, ram: 16, brand: CpuBrand::Amd, has_igpu: false, tier: HardwareTier::Mid },
    CpuSpec { name: "AMD Ryzen 5 5600G", threads: 12, ram: 16, brand: CpuBrand::Amd, has_igpu: true, tier: HardwareTier::Mid },
    CpuSpec { name: "AMD Ryzen 7 3700X", threads: 16, ram: 16, brand: CpuBrand::Amd, has_igpu: false, tier: HardwareTier::Mid },
    CpuSpec { name: "AMD Ryzen 7 5700X", threads: 16, ram: 32, brand: CpuBrand::Amd, has_igpu: false, tier: HardwareTier::Mid },
    CpuSpec { name: "AMD Ryzen 7 5800X", threads: 16, ram: 32, brand: CpuBrand::Amd, has_igpu: false, tier: HardwareTier::High },
    CpuSpec { name: "Intel Core i5-12400F", threads: 12, ram: 16, brand: CpuBrand::Intel, has_igpu: false, tier: HardwareTier::Mid },
    CpuSpec { name: "Intel Core i5-12600K", threads: 16, ram: 32, brand: CpuBrand::Intel, has_igpu: true, tier: HardwareTier::Mid },
    CpuSpec { name: "Intel Core i5-13400F", threads: 16, ram: 16, brand: CpuBrand::Intel, has_igpu: false, tier: HardwareTier::Mid },
    CpuSpec { name: "Intel Core i5-13600K", threads: 20, ram: 32, brand: CpuBrand::Intel, has_igpu: true, tier: HardwareTier::High },
    CpuSpec { name: "Intel Core i5-14400F", threads: 16, ram: 32, brand: CpuBrand::Intel, has_igpu: false, tier: HardwareTier::Mid },
    CpuSpec { name: "Intel Core i5-14600K", threads: 20, ram: 32, brand: CpuBrand::Intel, has_igpu: true, tier: HardwareTier::High },
    CpuSpec { name: "Intel Core i7-10700K", threads: 16, ram: 16, brand: CpuBrand::Intel, has_igpu: true, tier: HardwareTier::Mid },
    CpuSpec { name: "Intel Core i7-11700K", threads: 16, ram: 32, brand: CpuBrand::Intel, has_igpu: true, tier: HardwareTier::Mid },
    CpuSpec { name: "AMD Ryzen 7 7700X", threads: 16, ram: 32, brand: CpuBrand::Amd, has_igpu: true, tier: HardwareTier::High },
    CpuSpec { name: "AMD Ryzen 7 7800X3D", threads: 16, ram: 32, brand: CpuBrand::Amd, has_igpu: true, tier: HardwareTier::High },
    CpuSpec { name: "AMD Ryzen 9 5900X", threads: 24, ram: 32, brand: CpuBrand::Amd, has_igpu: false, tier: HardwareTier::High },
    CpuSpec { name: "AMD Ryzen 9 5950X", threads: 32, ram: 64, brand: CpuBrand::Amd, has_igpu: false, tier: HardwareTier::High },
    CpuSpec { name: "AMD Ryzen 9 7900X", threads: 24, ram: 32, brand: CpuBrand::Amd, has_igpu: true, tier: HardwareTier::High },
    CpuSpec { name: "AMD Ryzen 9 7950X", threads: 32, ram: 64, brand: CpuBrand::Amd, has_igpu: true, tier: HardwareTier::High },
    CpuSpec { name: "Intel Core i7-12700K", threads: 20, ram: 32, brand: CpuBrand::Intel, has_igpu: true, tier: HardwareTier::High },
    CpuSpec { name: "Intel Core i7-13700K", threads: 24, ram: 32, brand: CpuBrand::Intel, has_igpu: true, tier: HardwareTier::High },
    CpuSpec { name: "Intel Core i7-14700K", threads: 28, ram: 32, brand: CpuBrand::Intel, has_igpu: true, tier: HardwareTier::High },
    CpuSpec { name: "Intel Core i9-12900K", threads: 24, ram: 32, brand: CpuBrand::Intel, has_igpu: true, tier: HardwareTier::High },
    CpuSpec { name: "Intel Core i9-13900K", threads: 32, ram: 64, brand: CpuBrand::Intel, has_igpu: true, tier: HardwareTier::High },
    CpuSpec { name: "Intel Core i9-14900K", threads: 32, ram: 64, brand: CpuBrand::Intel, has_igpu: true, tier: HardwareTier::High },
];

pub const GPU_POOL: &[GpuSpec] = &[
    GpuSpec { name: "Intel HD Graphics 630", vendor: "Google Inc. (Intel)", renderer: "ANGLE (Intel, Intel(R) HD Graphics 630 (0x00005912) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::IntelIgpu, tier: HardwareTier::Entry },
    GpuSpec { name: "Intel UHD Graphics 620", vendor: "Google Inc. (Intel)", renderer: "ANGLE (Intel, Intel(R) UHD Graphics 620 (0x00005917) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::IntelIgpu, tier: HardwareTier::Entry },
    GpuSpec { name: "Intel UHD Graphics 730", vendor: "Google Inc. (Intel)", renderer: "ANGLE (Intel, Intel(R) UHD Graphics 730 (0x00004692) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::IntelIgpu, tier: HardwareTier::Entry },
    GpuSpec { name: "Intel UHD Graphics 770", vendor: "Google Inc. (Intel)", renderer: "ANGLE (Intel, Intel(R) UHD Graphics 770 (0x00004680) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::IntelIgpu, tier: HardwareTier::Mid },
    GpuSpec { name: "Intel Iris Xe Graphics", vendor: "Google Inc. (Intel)", renderer: "ANGLE (Intel, Intel(R) Iris(R) Xe Graphics (0x00009A49) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::IntelIgpu, tier: HardwareTier::Entry },
    GpuSpec { name: "AMD Radeon Vega 8", vendor: "Google Inc. (AMD)", renderer: "ANGLE (AMD, AMD Radeon(TM) Vega 8 Graphics (0x000015D8) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::AmdIgpu, tier: HardwareTier::Entry },
    GpuSpec { name: "AMD Radeon RX Vega 11", vendor: "Google Inc. (AMD)", renderer: "ANGLE (AMD, AMD Radeon(TM) RX Vega 11 Graphics (0x000015DD) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::AmdIgpu, tier: HardwareTier::Entry },
    GpuSpec { name: "AMD Radeon 680M", vendor: "Google Inc. (AMD)", renderer: "ANGLE (AMD, AMD Radeon 680M (0x00001681) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::AmdIgpu, tier: HardwareTier::Entry },
    GpuSpec { name: "AMD Radeon 780M", vendor: "Google Inc. (AMD)", renderer: "ANGLE (AMD, AMD Radeon 780M (0x000015BF) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::AmdIgpu, tier: HardwareTier::Mid },
    GpuSpec { name: "NVIDIA GeForce GTX 1050 Ti", vendor: "Google Inc. (NVIDIA)", renderer: "ANGLE (NVIDIA, NVIDIA GeForce GTX 1050 Ti (0x00001C82) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::Entry },
    GpuSpec { name: "NVIDIA GeForce GTX 1060 6GB", vendor: "Google Inc. (NVIDIA)", renderer: "ANGLE (NVIDIA, NVIDIA GeForce GTX 1060 6GB (0x00001C03) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::Entry },
    GpuSpec { name: "NVIDIA GeForce GTX 1650", vendor: "Google Inc. (NVIDIA)", renderer: "ANGLE (NVIDIA, NVIDIA GeForce GTX 1650 (0x00001F82) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::Entry },
    GpuSpec { name: "NVIDIA GeForce GTX 1660 Super", vendor: "Google Inc. (NVIDIA)", renderer: "ANGLE (NVIDIA, NVIDIA GeForce GTX 1660 SUPER (0x000021C4) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::Mid },
    GpuSpec { name: "NVIDIA GeForce GTX 1660 Ti", vendor: "Google Inc. (NVIDIA)", renderer: "ANGLE (NVIDIA, NVIDIA GeForce GTX 1660 Ti (0x00002182) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::Mid },
    GpuSpec { name: "AMD Radeon RX 570", vendor: "Google Inc. (AMD)", renderer: "ANGLE (AMD, Radeon RX 570 Series (0x000067DF) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::Entry },
    GpuSpec { name: "AMD Radeon RX 580", vendor: "Google Inc. (AMD)", renderer: "ANGLE (AMD, Radeon RX 580 Series (0x000067DF) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::Entry },
    GpuSpec { name: "AMD Radeon RX 6500 XT", vendor: "Google Inc. (AMD)", renderer: "ANGLE (AMD, AMD Radeon RX 6500 XT (0x0000743F) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::Entry },
    GpuSpec { name: "NVIDIA GeForce RTX 2060", vendor: "Google Inc. (NVIDIA)", renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 2060 (0x00001F08) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::Mid },
    GpuSpec { name: "NVIDIA GeForce RTX 2060 Super", vendor: "Google Inc. (NVIDIA)", renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 2060 SUPER (0x00001F06) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::Mid },
    GpuSpec { name: "NVIDIA GeForce RTX 2070", vendor: "Google Inc. (NVIDIA)", renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 2070 (0x00001F02) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::Mid },
    GpuSpec { name: "NVIDIA GeForce RTX 2070 Super", vendor: "Google Inc. (NVIDIA)", renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 2070 SUPER (0x00001E84) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::Mid },
    GpuSpec { name: "NVIDIA GeForce RTX 2080 Super", vendor: "Google Inc. (NVIDIA)", renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 2080 SUPER (0x00001E81) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::Mid },
    GpuSpec { name: "NVIDIA GeForce RTX 3050", vendor: "Google Inc. (NVIDIA)", renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 3050 (0x00002507) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::Mid },
    GpuSpec { name: "NVIDIA GeForce RTX 3060", vendor: "Google Inc. (NVIDIA)", renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 (0x00002504) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::Mid },
    GpuSpec { name: "NVIDIA GeForce RTX 3060 Ti", vendor: "Google Inc. (NVIDIA)", renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 Ti (0x00002489) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::Mid },
    GpuSpec { name: "NVIDIA GeForce RTX 3070", vendor: "Google Inc. (NVIDIA)", renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 3070 (0x00002484) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::High },
    GpuSpec { name: "NVIDIA GeForce RTX 3070 Ti", vendor: "Google Inc. (NVIDIA)", renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 3070 Ti (0x00002482) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::High },
    GpuSpec { name: "NVIDIA GeForce RTX 4060", vendor: "Google Inc. (NVIDIA)", renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 4060 (0x00002882) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::Mid },
    GpuSpec { name: "NVIDIA GeForce RTX 4060 Ti", vendor: "Google Inc. (NVIDIA)", renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 4060 Ti (0x00002803) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::High },
    GpuSpec { name: "AMD Radeon RX 6600", vendor: "Google Inc. (AMD)", renderer: "ANGLE (AMD, AMD Radeon RX 6600 (0x000073FF) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::Mid },
    GpuSpec { name: "AMD Radeon RX 6600 XT", vendor: "Google Inc. (AMD)", renderer: "ANGLE (AMD, AMD Radeon RX 6600 XT (0x000073FF) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::Mid },
    GpuSpec { name: "AMD Radeon RX 6700 XT", vendor: "Google Inc. (AMD)", renderer: "ANGLE (AMD, AMD Radeon RX 6700 XT (0x000073DF) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::High },
    GpuSpec { name: "AMD Radeon RX 7600", vendor: "Google Inc. (AMD)", renderer: "ANGLE (AMD, AMD Radeon RX 7600 (0x00007480) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::Mid },
    GpuSpec { name: "NVIDIA GeForce RTX 3080", vendor: "Google Inc. (NVIDIA)", renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 3080 (0x00002206) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::High },
    GpuSpec { name: "NVIDIA GeForce RTX 3080 Ti", vendor: "Google Inc. (NVIDIA)", renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 3080 Ti (0x00002208) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::High },
    GpuSpec { name: "NVIDIA GeForce RTX 3090", vendor: "Google Inc. (NVIDIA)", renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 3090 (0x00002204) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::High },
    GpuSpec { name: "NVIDIA GeForce RTX 4070", vendor: "Google Inc. (NVIDIA)", renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 4070 (0x00002786) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::High },
    GpuSpec { name: "NVIDIA GeForce RTX 4070 Ti", vendor: "Google Inc. (NVIDIA)", renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 4070 Ti (0x00002782) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::High },
    GpuSpec { name: "NVIDIA GeForce RTX 4080", vendor: "Google Inc. (NVIDIA)", renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 4080 (0x00002704) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::High },
    GpuSpec { name: "NVIDIA GeForce RTX 4090", vendor: "Google Inc. (NVIDIA)", renderer: "ANGLE (NVIDIA, NVIDIA GeForce RTX 4090 (0x00002684) Direct3D11 vs_5_0 ps_5_0, D3D11)", kind: GpuKind::Discrete, tier: HardwareTier::High },
];

pub const FONT_CATALOG: &[&str] = &[
    "Arial", "Calibri", "Cambria", "Comic Sans MS", "Consolas", "Courier New",
    "Georgia", "Impact", "Lucida Console", "Lucida Sans Unicode", "Segoe UI",
    "Tahoma", "Times New Roman", "Trebuchet MS", "Verdana", "Franklin Gothic Medium",
    "Palatino Linotype", "Candara", "Constantia", "Corbel", "Century Gothic",
    "Gill Sans MT", "Garamond", "MS Gothic", "MS PGothic", "SimSun", "Malgun Gothic",
    "Yu Gothic", "Sitka Text", "Ebrima", "Gadugi", "Leelawadee UI", "Nirmala UI",
];

pub fn is_compatible(cpu: &CpuSpec, gpu: &GpuSpec) -> bool {
    let is_laptop_u_or_g = cpu.name.ends_with('U')
        || cpu.name.contains("G7")
        || cpu.name.contains("G4");

    match gpu.kind {
        GpuKind::IntelIgpu => cpu.brand == CpuBrand::Intel && cpu.has_igpu,
        GpuKind::AmdIgpu => cpu.brand == CpuBrand::Amd && cpu.has_igpu,
        GpuKind::Discrete => {
            if is_laptop_u_or_g {
                // Ultrabook U/G-series CPUs rarely pair with desktop high-tier GPUs
                gpu.tier == HardwareTier::Entry
            } else {
                // Match within 1 tier distance so we don't pair i3-10100 with RTX 4090
                match (cpu.tier, gpu.tier) {
                    (HardwareTier::Entry, HardwareTier::High) => false,
                    (HardwareTier::High, HardwareTier::Entry) => false,
                    _ => true,
                }
            }
        }
    }
}

/// Picks a random CPU+GPU pair that stays inside the REAL host's hardware family:
/// - GPU: same vendor as the host (prefer same architecture + iGPU/dGPU class),
///   so WebGL extensions, limits and rendering output stay consistent with the claimed GPU;
/// - CPU: no more threads than the host has (worker timing would reveal the real ceiling),
///   preferring at least half of the host's threads to stay realistic.
pub fn pick_random_hardware_pair() -> (&'static CpuSpec, &'static GpuSpec) {
    use crate::host::{cpu_ok, gpu_match, host_info, GpuMatch};

    let mut rng = rand::thread_rng();
    let host = host_info();

    // 1. GPU candidates, best consistency tier that has enough variety
    let family: Vec<&'static GpuSpec> =
        GPU_POOL.iter().filter(|g| gpu_match(g) == GpuMatch::SameFamily).collect();
    let same_vendor: Vec<&'static GpuSpec> = GPU_POOL
        .iter()
        .filter(|g| matches!(gpu_match(g), GpuMatch::SameFamily | GpuMatch::SameVendor))
        .collect();
    let gpu_candidates: Vec<&'static GpuSpec> = if family.len() >= 3 {
        family
    } else if !same_vendor.is_empty() {
        same_vendor
    } else {
        GPU_POOL.iter().collect()
    };

    // 2. Try GPUs until one has compatible CPUs within the host thread budget
    let min_threads = (host.logical_cores / 2).max(4);
    let mut shuffled = gpu_candidates.clone();
    shuffled.shuffle(&mut rng);
    for gpu in shuffled {
        let preferred: Vec<&'static CpuSpec> = CPU_POOL
            .iter()
            .filter(|c| is_compatible(c, gpu) && cpu_ok(c) && c.threads >= min_threads)
            .collect();
        let fallback: Vec<&'static CpuSpec> = CPU_POOL
            .iter()
            .filter(|c| is_compatible(c, gpu) && cpu_ok(c))
            .collect();
        let pool = if !preferred.is_empty() { preferred } else { fallback };
        if let Some(cpu) = pool.choose(&mut rng) {
            return (cpu, gpu);
        }
    }

    // 3. Last resort (exotic host): legacy behaviour
    let cpu = CPU_POOL.choose(&mut rng).expect("CPU_POOL not empty");
    let gpu = GPU_POOL
        .iter()
        .filter(|g| is_compatible(cpu, g))
        .collect::<Vec<_>>()
        .choose(&mut rng)
        .copied()
        .unwrap_or(&GPU_POOL[23]);
    (cpu, gpu)
}

pub fn generate_random_fonts() -> Vec<String> {
    let mut rng = rand::thread_rng();
    // Always include essential Windows core fonts so websites render naturally, plus random extras
    let core = ["Arial", "Segoe UI", "Times New Roman", "Courier New", "Verdana", "Tahoma", "Calibri"];
    let mut selected: Vec<String> = core.iter().map(|s| (*s).to_string()).collect();

    let extras: Vec<&str> = FONT_CATALOG
        .iter()
        .copied()
        .filter(|f| !core.contains(f))
        .collect();

    let extra_count = rng.gen_range(7..=16).min(extras.len());
    for f in extras.choose_multiple(&mut rng, extra_count) {
        selected.push((*f).to_string());
    }
    selected
}

pub fn pick_random_resolution_for_cpu(cpu: &CpuSpec) -> (u32, u32) {
    let is_laptop = cpu.name.ends_with('U')
        || cpu.name.contains("G7")
        || cpu.name.contains("G4");
    let mut rng = rand::thread_rng();
    if is_laptop {
        let mobile_res = [(1920, 1080), (1920, 1080), (1366, 768)];
        *mobile_res.choose(&mut rng).unwrap_or(&(1920, 1080))
    } else {
        // Desktop systems overwhelmingly run 1920x1080 (prevents outerWidth > screen.width on 1080p monitors)
        let desktop_res = [(1920, 1080), (1920, 1080), (1920, 1080), (2560, 1440)];
        *desktop_res.choose(&mut rng).unwrap_or(&(1920, 1080))
    }
}

/// Maps a GPU profile to exact Chromium Dawn WebGPU `GPUAdapterInfo` `(vendor, architecture)` strings.
pub fn webgpu_vendor_arch(gpu_vendor: &str, gpu_name: &str) -> (&'static str, &'static str) {
    let v_low = gpu_vendor.to_ascii_lowercase();
    let n_low = gpu_name.to_ascii_lowercase();

    if v_low.contains("intel") || n_low.contains("intel") {
        if n_low.contains("730") || n_low.contains("770") || n_low.contains("iris xe") {
            ("intel", "gen-12lp")
        } else {
            ("intel", "gen-9")
        }
    } else if v_low.contains("amd") || n_low.contains("radeon") {
        if n_low.contains("7600") || n_low.contains("780m") {
            ("amd", "rdna-3")
        } else if n_low.contains("6500")
            || n_low.contains("6600")
            || n_low.contains("6700")
            || n_low.contains("680m")
        {
            ("amd", "rdna-2")
        } else if n_low.contains("vega") {
            ("amd", "gcn-5")
        } else {
            ("amd", "gcn-4")
        }
    } else if n_low.contains("rtx 40") {
        ("nvidia", "ada-lovelace")
    } else if n_low.contains("rtx 30") {
        ("nvidia", "ampere")
    } else if n_low.contains("rtx 20") || n_low.contains("gtx 16") {
        ("nvidia", "turing")
    } else {
        ("nvidia", "pascal")
    }
}
