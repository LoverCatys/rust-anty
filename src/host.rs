//! Detection of the REAL host hardware.
//!
//! JS-level spoofing can only rewrite strings. Everything a page can *measure*
//! (WebGL extensions/limits, WebGPU features, rendering output, real parallelism
//! via worker timing) still comes from the physical machine. To stay consistent,
//! spoofed profiles must stay inside the host's hardware "family":
//!   - same GPU vendor (and ideally same architecture), same iGPU/dGPU class;
//!   - no more logical CPU threads than the host really has.

use std::sync::OnceLock;

use crate::hardware::{webgpu_vendor_arch, CpuSpec, GpuKind, GpuSpec};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
}

impl GpuVendor {
    pub fn label(self) -> &'static str {
        match self {
            GpuVendor::Nvidia => "NVIDIA",
            GpuVendor::Amd => "AMD",
            GpuVendor::Intel => "Intel",
        }
    }
}

#[derive(Debug, Clone)]
pub struct HostGpu {
    pub name: String,
    pub vendor: GpuVendor,
    pub integrated: bool,
    pub arch: &'static str,
}

#[derive(Debug, Clone)]
pub struct HostInfo {
    pub logical_cores: u32,
    pub gpu: Option<HostGpu>,
}

static HOST: OnceLock<HostInfo> = OnceLock::new();

pub fn host_info() -> &'static HostInfo {
    HOST.get_or_init(detect)
}

fn detect() -> HostInfo {
    let logical_cores = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(8);
    HostInfo {
        logical_cores,
        gpu: detect_gpu(),
    }
}

/// Classifies an adapter name. Returns None for virtual / remote / emulator adapters.
pub fn classify_gpu_name(name: &str) -> Option<HostGpu> {
    let low = name.to_ascii_lowercase();
    const VIRTUAL_MARKERS: &[&str] = &[
        "virtual", "basic display", "basic render", "parsec", "mumu", "remote",
        "citrix", "vmware", "virtualbox", "hyper-v", "spacedesk", "meta", "idd",
    ];
    if VIRTUAL_MARKERS.iter().any(|m| low.contains(m)) {
        return None;
    }

    let (vendor, integrated) = if low.contains("nvidia") || low.contains("geforce") || low.contains("quadro") {
        (GpuVendor::Nvidia, false)
    } else if low.contains("amd") || low.contains("radeon") {
        // "Radeon(TM) Graphics", "Vega 8/11", "680M/780M" are APU iGPUs; "RX 5xx/6xxx/7xxx" are discrete.
        let igpu = low.contains("vega 8")
            || low.contains("vega 11")
            || (!low.contains(" rx ")
                && (low.contains("vega")
                    || low.ends_with("graphics")
                    || low.contains("m graphics")
                    || low.contains("680m")
                    || low.contains("780m")
                    || low.contains("760m")));
        (GpuVendor::Amd, igpu)
    } else if low.contains("intel") {
        // Intel Arc is discrete, everything else (HD/UHD/Iris) is integrated.
        (GpuVendor::Intel, !low.contains("arc"))
    } else {
        return None;
    };

    let vendor_str = match vendor {
        GpuVendor::Nvidia => "nvidia",
        GpuVendor::Amd => "amd",
        GpuVendor::Intel => "intel",
    };
    let (_, arch) = webgpu_vendor_arch(vendor_str, name);
    Some(HostGpu {
        name: name.trim().to_string(),
        vendor,
        integrated,
        arch,
    })
}

#[cfg(windows)]
fn detect_gpu() -> Option<HostGpu> {
    use std::process::{Command, Stdio};
    let out = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Get-CimInstance Win32_VideoController | ForEach-Object { $_.Name }",
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let gpus: Vec<HostGpu> = text.lines().filter_map(classify_gpu_name).collect();
    // Chrome renders on the discrete GPU when the monitor is attached to it (desktop),
    // so prefer a discrete adapter if one exists.
    gpus.iter()
        .find(|g| !g.integrated)
        .cloned()
        .or_else(|| gpus.into_iter().next())
}

#[cfg(not(windows))]
fn detect_gpu() -> Option<HostGpu> {
    None
}

pub fn spec_vendor(gpu: &GpuSpec) -> GpuVendor {
    vendor_from_strings(gpu.vendor, gpu.name).unwrap_or(GpuVendor::Nvidia)
}

pub fn vendor_from_strings(vendor: &str, name: &str) -> Option<GpuVendor> {
    let s = format!("{vendor} {name}").to_ascii_lowercase();
    if s.contains("nvidia") || s.contains("geforce") {
        Some(GpuVendor::Nvidia)
    } else if s.contains("amd") || s.contains("radeon") {
        Some(GpuVendor::Amd)
    } else if s.contains("intel") {
        Some(GpuVendor::Intel)
    } else {
        None
    }
}

/// How well a spoofed GPU matches the real one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GpuMatch {
    /// Different vendor: WebGL extensions / limits / render output betray the real GPU.
    WrongVendor,
    /// Same vendor but iGPU vs dGPU or different architecture: minor measurable differences.
    SameVendor,
    /// Same vendor, class and architecture: indistinguishable for practical purposes.
    SameFamily,
    /// Host GPU unknown: cannot judge.
    Unknown,
}

pub fn gpu_match(gpu: &GpuSpec) -> GpuMatch {
    let Some(host) = &host_info().gpu else {
        return GpuMatch::Unknown;
    };
    if spec_vendor(gpu) != host.vendor {
        return GpuMatch::WrongVendor;
    }
    let spec_integrated = gpu.kind != GpuKind::Discrete;
    let (_, spec_arch) = webgpu_vendor_arch(gpu.vendor, gpu.name);
    if spec_integrated == host.integrated && spec_arch == host.arch {
        GpuMatch::SameFamily
    } else {
        GpuMatch::SameVendor
    }
}

pub fn profile_gpu_match(vendor: &str, name: &str) -> GpuMatch {
    let Some(host) = &host_info().gpu else {
        return GpuMatch::Unknown;
    };
    match vendor_from_strings(vendor, name) {
        Some(v) if v == host.vendor => {
            let (_, arch) = webgpu_vendor_arch(vendor, name);
            if arch == host.arch {
                GpuMatch::SameFamily
            } else {
                GpuMatch::SameVendor
            }
        }
        _ => GpuMatch::WrongVendor,
    }
}

/// A spoofed CPU must not claim more threads than really exist:
/// a worker-saturation timing test reveals the true parallelism ceiling.
pub fn cpu_threads_ok(threads: u32) -> bool {
    threads <= host_info().logical_cores
}

pub fn cpu_ok(cpu: &CpuSpec) -> bool {
    cpu_threads_ok(cpu.threads)
}
