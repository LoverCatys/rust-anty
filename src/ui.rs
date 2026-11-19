use std::io::{self, Write};

use crate::hardware::{CpuSpec, GpuSpec};

pub const COLOR_RUST: (u8, u8, u8) = (255, 82, 28);
pub const COLOR_AMBER: (u8, u8, u8) = (255, 162, 42);
pub const COLOR_GOLD: (u8, u8, u8) = (255, 216, 92);
pub const COLOR_CREAM: (u8, u8, u8) = (255, 232, 214);
pub const COLOR_STEEL: (u8, u8, u8) = (156, 174, 188);
pub const COLOR_MUTED: (u8, u8, u8) = (100, 116, 130);
pub const COLOR_ONLINE: (u8, u8, u8) = (74, 222, 128);
pub const COLOR_ERR: (u8, u8, u8) = (248, 113, 113);
pub const COLOR_WARN: (u8, u8, u8) = (251, 191, 36);

pub const RESET: &str = "\x1b[0m";
pub const BOLD: &str = "\x1b[1m";

pub const BANNER_LINES: &[&str] = &[
    "██████╗ ██╗   ██╗███████╗████████╗   █████╗ ███╗   ██╗████████╗██╗   ██╗",
    "██╔══██╗██║   ██║██╔════╝╚══██╔══╝  ██╔══██╗████╗  ██║╚══██╔══╝╚██╗ ██╔╝",
    "██████╔╝██║   ██║███████╗   ██║     ███████║██╔██╗ ██║   ██║    ╚████╔╝ ",
    "██╔══██╗██║   ██║╚════██║   ██║     ██╔══██║██║╚██╗██║   ██║     ╚██╔╝  ",
    "██║  ██║╚██████╔╝███████║   ██║     ██║  ██║██║ ╚████║   ██║      ██║   ",
    "╚═╝  ╚═╝ ╚═════╝ ╚══════╝   ╚═╝     ╚═╝  ╚═╝╚═╝  ╚═══╝   ╚═╝      ╚═╝   ",
];

pub fn init_terminal() {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Console::{
            GetConsoleMode, GetStdHandle, SetConsoleCP, SetConsoleMode, SetConsoleOutputCP,
            ENABLE_VIRTUAL_TERMINAL_PROCESSING, STD_OUTPUT_HANDLE,
        };

        SetConsoleOutputCP(65001);
        SetConsoleCP(65001);

        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        let mut mode: u32 = 0;
        if GetConsoleMode(handle, &mut mode) != 0 {
            let _ = SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
        }
    }

    print!("\x1b]0;RUST ANTY v2.0 - Native Antidetect Engine\x07");
    let _ = io::stdout().flush();
}

pub fn clear_screen() {
    print!("\x1b[2J\x1b[H");
    let _ = io::stdout().flush();
}

pub fn fg(c: (u8, u8, u8)) -> String {
    format!("\x1b[38;2;{};{};{}m", c.0, c.1, c.2)
}

pub fn paint(text: &str, c: (u8, u8, u8)) -> String {
    format!("{}{text}{RESET}", fg(c))
}

pub fn paint_bold(text: &str, c: (u8, u8, u8)) -> String {
    format!("{BOLD}{}{text}{RESET}", fg(c))
}

pub fn c_rust(text: &str) -> String {
    paint(text, COLOR_RUST)
}
pub fn c_amber(text: &str) -> String {
    paint(text, COLOR_AMBER)
}
pub fn c_gold(text: &str) -> String {
    paint(text, COLOR_GOLD)
}
pub fn c_cream(text: &str) -> String {
    paint(text, COLOR_CREAM)
}
pub fn c_steel(text: &str) -> String {
    paint(text, COLOR_STEEL)
}
pub fn c_muted(text: &str) -> String {
    paint(text, COLOR_MUTED)
}
pub fn c_online(text: &str) -> String {
    paint_bold(text, COLOR_ONLINE)
}
pub fn c_err(text: &str) -> String {
    paint_bold(text, COLOR_ERR)
}
pub fn c_warn(text: &str) -> String {
    paint_bold(text, COLOR_WARN)
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round().clamp(0.0, 255.0) as u8
}

fn lerp_color(c1: (u8, u8, u8), c2: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    (
        lerp_u8(c1.0, c2.0, t),
        lerp_u8(c1.1, c2.1, t),
        lerp_u8(c1.2, c2.2, t),
    )
}

pub fn gradient_text(text: &str, c_start: (u8, u8, u8), c_end: (u8, u8, u8)) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    if n == 0 {
        return String::new();
    }
    if n == 1 {
        return paint(text, c_start);
    }

    let mut out = String::with_capacity(n * 20);
    for (i, ch) in chars.into_iter().enumerate() {
        let ratio = i as f32 / (n - 1) as f32;
        let col = lerp_color(c_start, c_end, ratio);
        out.push_str(&fg(col));
        out.push(ch);
    }
    out.push_str(RESET);
    out
}

pub fn render_banner() -> String {
    let total_rows = BANNER_LINES.len();
    let mut out = String::new();
    out.push('\n');

    for (r, line) in BANNER_LINES.iter().enumerate() {
        let row_t = r as f32 / (total_rows.saturating_sub(1).max(1)) as f32;
        let row_start = lerp_color(COLOR_RUST, COLOR_AMBER, row_t * 0.65);
        let row_end = lerp_color(COLOR_AMBER, COLOR_GOLD, row_t);
        out.push_str("  ");
        out.push_str(&gradient_text(line, row_start, row_end));
        out.push('\n');
    }

    out
}

pub fn divider(width: usize) -> String {
    format!("  {}", gradient_text(&"━".repeat(width), COLOR_RUST, COLOR_GOLD))
}

pub fn thin_divider(width: usize) -> String {
    format!("  {}", c_muted(&"─".repeat(width)))
}

pub fn section_header(title: &str) -> String {
    let badge = gradient_text(&format!("> {title}"), COLOR_RUST, COLOR_GOLD);
    format!("\n  {BOLD}{badge}{RESET}")
}

pub fn key_badge(k: &str) -> String {
    format!("{}{}{}{RESET}", fg(COLOR_MUTED), "[", RESET)
        + &paint_bold(k, COLOR_GOLD)
        + &format!("{}]{RESET}", fg(COLOR_MUTED))
}

pub fn prompt(label: &str) -> String {
    print!("\n  {} {}: {}", paint_bold(">", COLOR_RUST), c_cream(label), fg(COLOR_GOLD));
    let _ = io::stdout().flush();
    let mut buf = String::new();
    let _ = io::stdin().read_line(&mut buf);
    print!("{RESET}");
    let _ = io::stdout().flush();
    buf.trim().to_string()
}

pub fn pause(msg: &str) {
    print!("\n  {} {}", c_muted("[Enter]"), c_steel(msg));
    let _ = io::stdout().flush();
    let mut dummy = String::new();
    let _ = io::stdin().read_line(&mut dummy);
}

pub fn format_uptime(secs: u64) -> String {
    let mins = secs / 60;
    let rem = secs % 60;
    if mins >= 60 {
        format!("{}ч {:02}м", mins / 60, mins % 60)
    } else if mins > 0 {
        format!("{}м {:02}с", mins, rem)
    } else {
        format!("{}с", rem)
    }
}

pub fn select_cpu_paginated(pool: &[CpuSpec]) -> &'static CpuSpec {
    let page_size = 10usize;
    let total_pages = (pool.len() + page_size - 1) / page_size;
    let mut page = 0usize;

    loop {
        clear_screen();
        println!(
            "{}",
            section_header(&format!(
                "ВЫБОР ПРОЦЕССОРА  (Стр. {}/{})",
                page + 1,
                total_pages
            ))
        );
        println!("{}", divider(72));

        let start = page * page_size;
        let end = (start + page_size).min(pool.len());
        let host_cores = crate::host::host_info().logical_cores;
        for idx in start..end {
            let item = &pool[idx];
            let igpu_tag = if item.has_igpu { "iGPU" } else { "dGPU only" };
            let warn = if crate::host::cpu_ok(item) {
                String::new()
            } else {
                format!(" {}", c_warn(&format!("(!) потоков больше, чем у вас ({host_cores})")))
            };
            println!(
                "  {} {} {}{}",
                key_badge(&format!("{:2}", idx + 1)),
                c_cream(item.name),
                c_muted(&format!(
                    "({} потоков • {} GB RAM • {})",
                    item.threads, item.ram, igpu_tag
                )),
                warn
            );
        }

        println!("{}", thin_divider(72));
        println!(
            "  {} След. стр.   {} Пред. стр.   {} Случайный выбор",
            key_badge("N"),
            key_badge("P"),
            key_badge("R")
        );

        let choice = prompt("Номер или команда").to_ascii_lowercase();
        match choice.as_str() {
            "n" if page + 1 < total_pages => page += 1,
            "p" if page > 0 => page -= 1,
            "r" => {
                use rand::seq::SliceRandom;
                let mut rng = rand::thread_rng();
                let safe: Vec<&'static CpuSpec> = crate::hardware::CPU_POOL
                    .iter()
                    .filter(|c| crate::host::cpu_ok(c))
                    .collect();
                return safe
                    .choose(&mut rng)
                    .copied()
                    .unwrap_or(&crate::hardware::CPU_POOL[0]);
            }
            _ => {
                if let Ok(num) = choice.parse::<usize>() {
                    if (1..=crate::hardware::CPU_POOL.len()).contains(&num) {
                        return &crate::hardware::CPU_POOL[num - 1];
                    }
                }
            }
        }
    }
}

pub fn select_gpu_paginated(chosen_cpu: &CpuSpec) -> &'static GpuSpec {
    let pool = crate::hardware::GPU_POOL;
    let page_size = 10usize;
    let total_pages = (pool.len() + page_size - 1) / page_size;
    let mut page = 0usize;

    loop {
        clear_screen();
        println!(
            "{}",
            section_header(&format!(
                "ВЫБОР ВИДЕОКАРТЫ  (Стр. {}/{})",
                page + 1,
                total_pages
            ))
        );
        println!(
            "  {} Выбран CPU: {}",
            c_steel("[i]"),
            c_gold(chosen_cpu.name)
        );
        if let Some(hg) = &crate::host::host_info().gpu {
            println!(
                "  {} Ваша видеокарта: {} {}",
                c_steel("[i]"),
                c_gold(&hg.name),
                c_muted("(берите того же вендора, иначе WebGL выдаст подмену)")
            );
        }
        println!("{}", divider(72));

        let start = page * page_size;
        let end = (start + page_size).min(pool.len());
        for idx in start..end {
            let item = &pool[idx];
            let compat = crate::hardware::is_compatible(chosen_cpu, item);
            let compat_hint = match (compat, crate::host::gpu_match(item)) {
                (_, crate::host::GpuMatch::WrongVendor) => {
                    c_err("(!) другой вендор - WebGL палится")
                }
                (false, _) => c_warn("(!) нетипично для выбранного CPU"),
                (true, crate::host::GpuMatch::SameFamily) => {
                    c_online("(+) как ваша")
                }
                (true, _) => c_muted(&format!("({})", item.vendor)),
            };
            println!(
                "  {} {} {}",
                key_badge(&format!("{:2}", idx + 1)),
                c_cream(item.name),
                compat_hint
            );
        }

        println!("{}", thin_divider(72));
        println!(
            "  {} След. стр.   {} Пред. стр.   {} Совместимый рандом",
            key_badge("N"),
            key_badge("P"),
            key_badge("R")
        );

        let choice = prompt("Номер или команда").to_ascii_lowercase();
        match choice.as_str() {
            "n" if page + 1 < total_pages => page += 1,
            "p" if page > 0 => page -= 1,
            "r" => {
                use rand::seq::SliceRandom;
                let mut rng = rand::thread_rng();
                let compat: Vec<&'static GpuSpec> = pool
                    .iter()
                    .filter(|g| {
                        crate::hardware::is_compatible(chosen_cpu, g)
                            && crate::host::gpu_match(g) != crate::host::GpuMatch::WrongVendor
                    })
                    .collect();
                return compat.choose(&mut rng).copied().unwrap_or(&pool[23]);
            }
            _ => {
                if let Ok(num) = choice.parse::<usize>() {
                    if (1..=pool.len()).contains(&num) {
                        return &pool[num - 1];
                    }
                }
            }
        }
    }
}
