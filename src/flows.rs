use std::fs;
use std::time::Instant;

use crate::browser::{
    export_profile_cookies, import_profile_cookies, launch_profile_session, SessionManager,
};
use crate::cookies::{self, export_to_json, export_to_netscape, parse_cookies};
use crate::hardware::{
    generate_random_fonts, pick_random_hardware_pair, pick_random_resolution_for_cpu,
    web_device_memory, CPU_POOL,
};
use crate::host::{cpu_threads_ok, host_info, profile_gpu_match, GpuMatch};
use crate::models::{
    generate_seed, normalize_timezone, save_db, session_dir_for_profile, validate_profile_name,
    Profile, ProfileDb, ProxyConfig,
};
use crate::proxy::{check_proxy, parse_proxy};
use crate::ui::{
    c_amber, c_cream, c_err, c_gold, c_muted, c_online, c_rust, c_steel, c_warn, clear_screen,
    divider, format_uptime, key_badge, pause, prompt, section_header, select_cpu_paginated,
    select_gpu_paginated, thin_divider,
};

pub enum ProxyFlowResult {
    Selected {
        proxy: ProxyConfig,
        timezone: Option<String>,
        latitude: Option<f64>,
        longitude: Option<f64>,
    },
    Removed,
    Cancelled,
}

fn print_datacenter_hint(proxy_type: &str) {
    if proxy_type == "Датацентр" {
        println!(
            "  {} Датацентр-прокси: чекеры (Pixelscan) определяют его по ASN -> 'Proxy detected'.",
            c_warn("[!]")
        );
        println!(
            "      {}",
            c_muted("Для прохождения антифрода используйте резидентные или мобильные прокси.")
        );
    }
}

pub fn print_hardware_warnings(prof: &Profile) {
    let host = host_info();
    if !cpu_threads_ok(prof.cpu) {
        println!(
            "  {} CPU профиля ({} потоков) > реальных на ПК ({}) - нагрузочный тест Workers выявит расхождение.",
            c_warn("[!]"),
            prof.cpu,
            host.logical_cores
        );
    }
    if profile_gpu_match(&prof.gpu_vendor, &prof.gpu_name) == GpuMatch::WrongVendor {
        if let Some(hg) = &host.gpu {
            println!(
                "  {} GPU профиля ({}) другого вендора, чем ваша ({}) - параметры WebGL выдадут подмену.",
                c_err("[!]"),
                prof.gpu_name,
                hg.name
            );
        }
    }
}

pub async fn configure_proxy_flow(allow_remove: bool) -> ProxyFlowResult {
    loop {
        clear_screen();
        println!("{}", section_header("НАСТРОЙКА ПРОКСИ-СОЕДИНЕНИЯ"));
        println!("{}", divider(72));
        println!("  {} HTTP", key_badge("1"));
        println!("  {} SOCKS5  {}", key_badge("2"), c_muted("(удалённый DNS, без утечек)"));
        println!("  {} SOCKS4 / SOCKS4a", key_badge("3"));
        println!("  {} HTTPS   {}", key_badge("4"), c_muted("(TLS-соединение до прокси)"));
        if allow_remove {
            println!("  {} Без прокси  {}", key_badge("5"), c_muted("(прямое подключение)"));
        } else {
            println!("  {} Пропустить (без прокси)", key_badge("5"));
        }
        println!("  {} Отмена / Назад", key_badge("0"));
        println!("{}", thin_divider(72));

        let choice = prompt("Выберите протокол [0-5]");
        let scheme = match choice.as_str() {
            "1" => "http",
            "2" => "socks5",
            "3" => "socks4",
            "4" => "https",
            "5" => return ProxyFlowResult::Removed,
            "0" | "" => return ProxyFlowResult::Cancelled,
            _ => continue,
        };

        loop {
            println!(
                "\n  {} Протокол: {}",
                c_rust(">"),
                c_gold(&scheme.to_ascii_uppercase())
            );
            println!(
                "  {}",
                c_muted("Поддерживаемые форматы: IP:PORT • IP:PORT:USER:PASS • USER:PASS@HOST:PORT • IP:PORT@USER:PASS")
            );
            println!("  {}", c_muted("Введите '0' для возврата к выбору протокола"));

            let raw = prompt("Строка прокси");
            if raw == "0" || raw.is_empty() {
                break;
            }

            let Some(parsed) = parse_proxy(&raw, scheme) else {
                println!(
                    "\n  {} {}",
                    c_warn("[!]"),
                    c_err("Не удалось распознать формат прокси. Проверьте порт и разделители.")
                );
                continue;
            };

            println!(
                "\n  {} Проверка туннеля {}...",
                c_amber("[*]"),
                c_gold(&parsed.display_short())
            );

            match check_proxy(&parsed, 7).await {
                Ok(info) => {
                    let isp_full = if info.asn.is_empty() {
                        info.isp.clone()
                    } else {
                        format!("{} ({})", info.isp, info.asn)
                    };
                    let tz_label = info
                        .timezone
                        .as_deref()
                        .unwrap_or("не определена (останется текущая)");
                    println!("\n{}", divider(62));
                    println!("  {} {}", c_online("[+]"), c_online("ПРОКСИ АКТИВЕН И ПРОВЕРЕН"));
                    println!("{}", thin_divider(62));
                    println!("  Внешний IP   : {}", c_gold(&info.ip));
                    println!(
                        "  Геолокация   : {}",
                        c_cream(&format!("{}, {}", info.country, info.city))
                    );
                    println!("  Провайдер    : {}", c_steel(&isp_full));
                    println!("  Тип сети     : {}", c_amber(&info.proxy_type));
                    println!("  Часовой пояс : {}", c_online(tz_label));
                    print_datacenter_hint(&info.proxy_type);
                    println!("{}", divider(62));
                    pause("Нажмите Enter, чтобы применить прокси...");
                    return ProxyFlowResult::Selected {
                        proxy: parsed,
                        timezone: info.timezone,
                        latitude: info.latitude,
                        longitude: info.longitude,
                    };
                }
                Err(reason) => {
                    println!(
                        "\n  {} {}",
                        c_err("[-] ОШИБКА ПРОВЕРКИ ПРОКСИ:"),
                        c_cream(&reason)
                    );
                    println!(
                        "\n  {} Ввести другой   {} Повторить тест   {} Сохранить без проверки   {} Отмена",
                        key_badge("1"),
                        key_badge("2"),
                        key_badge("3"),
                        key_badge("0")
                    );
                    match prompt("Действие").as_str() {
                        "2" => {
                            println!("  {} Повторная проверка (таймаут 10с)...", c_amber("[*]"));
                            if let Ok(info) = check_proxy(&parsed, 10).await {
                                println!(
                                    "  {} Успешно! IP: {} ({}, {})",
                                    c_online("[+]"),
                                    c_gold(&info.ip),
                                    info.country,
                                    info.city
                                );
                                print_datacenter_hint(&info.proxy_type);
                                pause("Нажмите Enter для продолжения...");
                                return ProxyFlowResult::Selected {
                                    proxy: parsed,
                                    timezone: info.timezone,
                                    latitude: info.latitude,
                                    longitude: info.longitude,
                                };
                            } else {
                                println!("  {} Повторная проверка также не удалась.", c_err("[!]"));
                            }
                        }
                        "3" => {
                            let manual_tz = prompt(
                                "Часовой пояс прокси (например Europe/London, Enter — оставить текущий)",
                            );
                            let tz_opt = if manual_tz.is_empty() {
                                None
                            } else {
                                Some(normalize_timezone(&manual_tz))
                            };
                            return ProxyFlowResult::Selected {
                                proxy: parsed,
                                timezone: tz_opt,
                                latitude: None,
                                longitude: None,
                            };
                        }
                        "0" => return ProxyFlowResult::Cancelled,
                        _ => continue,
                    }
                }
            }
        }
    }
}

pub async fn create_profile_flow(db: &mut ProfileDb) -> Option<String> {
    clear_screen();
    println!("{}", section_header("СОЗДАНИЕ НОВОГО ПРОФИЛЯ"));
    println!("{}", divider(72));

    let name = prompt("Название профиля (Enter — отмена)");
    if name.is_empty() {
        return None;
    }
    if let Err(msg) = validate_profile_name(&name) {
        println!("\n  {} {}", c_err("[-]"), c_cream(msg));
        pause("Нажмите Enter...");
        return None;
    }
    if db.contains_key(&name) {
        println!(
            "\n  {} {}",
            c_err("[-]"),
            c_cream("Профиль с таким именем уже существует!")
        );
        pause("Нажмите Enter...");
        return None;
    }

    let host = host_info();
    let host_gpu_label = host
        .gpu
        .as_ref()
        .map(|g| format!("{} ({})", g.name, g.vendor.label()))
        .unwrap_or_else(|| "не определена".to_string());
    println!(
        "\n  {} Ваш ПК: {} потоков • GPU: {}",
        c_steel("[i]"),
        c_gold(&host.logical_cores.to_string()),
        c_cream(&host_gpu_label)
    );
    println!("  {}", c_steel("Подбор аппаратного отпечатка (CPU / GPU / RAM):"));
    println!(
        "  {} Умный рандом под ваш ПК  {}",
        key_badge("1"),
        c_muted("(рекомендуется: тот же вендор GPU, потоки <= реальных)")
    );
    println!("  {} Выбрать процессор и видеокарту вручную", key_badge("2"));

    let hw_mode = prompt("Ваш выбор [1/2]");
    let (chosen_cpu, chosen_gpu) = if hw_mode == "2" {
        let cpu = select_cpu_paginated(CPU_POOL);
        let gpu = select_gpu_paginated(cpu);
        (cpu, gpu)
    } else {
        pick_random_hardware_pair()
    };

    let (width, height) = pick_random_resolution_for_cpu(chosen_cpu);
    let fonts = generate_random_fonts();
    let seed = generate_seed();

    let (proxy_opt, timezone, latitude, longitude) = match configure_proxy_flow(false).await {
        ProxyFlowResult::Selected {
            proxy,
            timezone,
            latitude,
            longitude,
        } => {
            let tz = match timezone {
                Some(t) => t,
                None => {
                    let tz_in = prompt("Часовой пояс не определён. Введите вручную [по умолчанию Europe/Kyiv]");
                    normalize_timezone(&tz_in)
                }
            };
            (Some(proxy), tz, latitude, longitude)
        }
        ProxyFlowResult::Removed | ProxyFlowResult::Cancelled => {
            let tz_in = prompt("Часовой пояс [по умолчанию Europe/Kyiv]");
            (None, normalize_timezone(&tz_in), None, None)
        }
    };

    let profile = Profile {
        cpu_name: chosen_cpu.name.to_string(),
        cpu: chosen_cpu.threads,
        ram: chosen_cpu.ram,
        gpu_name: chosen_gpu.name.to_string(),
        gpu_vendor: chosen_gpu.vendor.to_string(),
        gpu_renderer: chosen_gpu.renderer.to_string(),
        width,
        height,
        timezone,
        fonts,
        proxy: proxy_opt,
        noise_seed: seed,
        folder_id: None,
        latitude,
        longitude,
    };

    db.insert(name.clone(), profile.clone());
    if let Err(e) = save_db(db) {
        println!("\n  {} Ошибка сохранения БД: {}", c_err("[-]"), e);
    } else {
        println!(
            "\n  {} Профиль {} создан! ({} / {})",
            c_online("[+]"),
            c_gold(&name),
            c_cream(chosen_cpu.name),
            c_amber(chosen_gpu.name)
        );
        print_hardware_warnings(&profile);

        let cookie_input = prompt("Импортировать cookies сейчас? (путь к файлу / Enter для пропуска)");
        if !cookie_input.is_empty() {
            let p = std::path::Path::new(&cookie_input);
            let content = if p.is_file() {
                fs::read_to_string(p).unwrap_or_else(|_| cookie_input.clone())
            } else {
                cookie_input
            };
            match parse_cookies(&content) {
                Ok(cookies) => {
                    let s_dir = session_dir_for_profile(&name, &profile);
                    if cookies::save_pending_cookies(&s_dir, &cookies).is_ok() {
                        println!(
                            "  {} Загружено {} cookies для первого запуска!",
                            c_online("[+]"),
                            c_gold(&cookies.len().to_string())
                        );
                    }
                }
                Err(e) => {
                    println!("  {} Предупреждение: не удалось распознать cookies: {e}", c_warn("[!]"));
                }
            }
        }
    }
    pause("Нажмите Enter для возврата в меню...");
    Some(name)
}

pub fn select_profile_flow(db: &ProfileDb, sessions: &SessionManager) -> Option<String> {
    clear_screen();
    println!("{}", section_header("СПИСОК ПРОФИЛЕЙ"));
    println!("{}", divider(76));

    if db.is_empty() {
        println!("  {}", c_muted("База профилей пуста. Создайте первый профиль в пункте [1]."));
        pause("Нажмите Enter...");
        return None;
    }

    let names: Vec<String> = db.keys().cloned().collect();
    for (idx, name) in names.iter().enumerate() {
        let prof = &db[name];
        let status = if sessions.is_running(name) {
            c_online("[ONLINE]")
        } else {
            c_muted("[IDLE]  ")
        };
        let hw_warn = if !cpu_threads_ok(prof.cpu)
            || profile_gpu_match(&prof.gpu_vendor, &prof.gpu_name) == GpuMatch::WrongVendor
        {
            format!(" {}", c_warn("[!]"))
        } else {
            String::new()
        };
        let prx = prof
            .proxy
            .as_ref()
            .map(|p| c_amber(&p.display_short()))
            .unwrap_or_else(|| c_muted("Без прокси"));

        println!(
            "  {} {}  {}{}  {} {}  │ {}",
            key_badge(&format!("{:2}", idx + 1)),
            status,
            c_gold(name),
            hw_warn,
            c_muted("-"),
            c_cream(&format!("{} / {}", prof.cpu_name, prof.gpu_name)),
            prx
        );
    }

    println!("{}", thin_divider(76));
    let raw = prompt("Номер профиля для выбора (Enter — назад)");
    if let Ok(n) = raw.parse::<usize>() {
        if (1..=names.len()).contains(&n) {
            return Some(names[n - 1].clone());
        }
    }
    None
}

pub async fn edit_profile_flow(
    db: &mut ProfileDb,
    mut current_name: String,
    sessions: &SessionManager,
) -> Option<String> {
    loop {
        let Some(prof) = db.get(&current_name).cloned() else {
            return None;
        };

        let is_online = sessions.is_running(&current_name);
        let status_badge = if is_online {
            c_online("[ONLINE] (запущен)")
        } else {
            c_muted("[IDLE] Не запущен")
        };
        let prx_str = prof
            .proxy
            .as_ref()
            .map(|p| c_gold(&p.display_short()))
            .unwrap_or_else(|| c_muted("Без прокси"));

        clear_screen();
        println!(
            "{}",
            section_header(&format!("РЕДАКТОР ПРОФИЛЯ: {}", current_name))
        );
        println!("{}", divider(72));
        println!("  Статус       : {status_badge}");
        println!(
            "  Процессор    : {} {}",
            c_cream(&prof.cpu_name),
            c_muted(&format!(
                "({} потоков • {} GB RAM -> Web API {} GB)",
                prof.cpu,
                prof.ram,
                web_device_memory(prof.ram)
            ))
        );
        println!("  Видеокарта   : {}", c_cream(&prof.gpu_name));
        println!(
            "  Экран        : {}",
            c_gold(&format!("{}x{}", prof.width, prof.height))
        );
        println!("  Таймзона     : {}", c_amber(&prof.timezone));
        println!(
            "  Отпечатки    : {}",
            c_muted(&format!("Seed #{:08X} • Шрифты: нативные", prof.noise_seed & 0xFFFF_FFFF))
        );
        println!("  Прокси       : {prx_str}");
        print_hardware_warnings(&prof);
        println!("{}", divider(72));
        println!("  {} Переименовать профиль", key_badge("1"));
        println!("  {} Изменить процессор и видеокарту", key_badge("2"));
        println!("  {} Изменить разрешение экрана", key_badge("3"));
        println!("  {} Настроить / сменить / отключить прокси", key_badge("4"));
        println!("  {} Проверить текущий прокси профиля", key_badge("5"));
        println!("  {} Изменить часовой пояс вручную", key_badge("6"));
        println!("  {} Перегенерировать Seed шума (Canvas/Audio)", key_badge("7"));
        println!("  {} Экспорт Cookies (JSON / Netscape)", key_badge("8"));
        println!("  {} Импорт Cookies (JSON / Netscape)", key_badge("9"));
        println!("  {} {}", key_badge("D"), c_err("Удалить профиль навсегда"));
        println!("  {} Вернуться в главное меню", key_badge("0"));
        println!("{}", thin_divider(72));

        match prompt("Ваш выбор").as_str() {
            "1" => {
                if is_online {
                    println!(
                        "\n  {} Закройте активную сессию профиля перед переименованием!",
                        c_warn("[!]")
                    );
                    pause("Нажмите Enter...");
                    continue;
                }
                let new_name = prompt("Новое название профиля");
                if new_name.is_empty() || new_name == current_name {
                    continue;
                }
                if let Err(msg) = validate_profile_name(&new_name) {
                    println!("\n  {} {}", c_err("[-]"), c_cream(msg));
                    pause("Нажмите Enter...");
                    continue;
                }
                if db.contains_key(&new_name) {
                    println!("\n  {} Профиль с таким именем уже существует!", c_err("[-]"));
                    pause("Нажмите Enter...");
                    continue;
                }

                let old_dir = session_dir_for_profile(&current_name, &prof);
                let updated_prof = prof.clone();
                if updated_prof.folder_id.is_none() && old_dir.exists() {
                    let target_probe = Profile {
                        folder_id: None,
                        ..updated_prof.clone()
                    };
                    let new_dir = session_dir_for_profile(&new_name, &target_probe);
                    if let Err(e) = fs::rename(&old_dir, &new_dir) {
                        println!(
                            "\n  {} Не удалось переименовать папку сессии: {e}",
                            c_err("[-]")
                        );
                        pause("Нажмите Enter...");
                        continue;
                    }
                }

                db.remove(&current_name);
                db.insert(new_name.clone(), updated_prof);
                let _ = save_db(db);
                current_name = new_name;
                println!("\n  {} Имя профиля обновлено!", c_online("[+]"));
                pause("Нажмите Enter...");
            }
            "2" => {
                println!(
                    "\n  {} Умный рандом под ваш ПК   {} Выбрать вручную",
                    key_badge("1"),
                    key_badge("2")
                );
                let (c, g) = if prompt("Выбор [1/2]") == "2" {
                    let cpu = select_cpu_paginated(CPU_POOL);
                    let gpu = select_gpu_paginated(cpu);
                    (cpu, gpu)
                } else {
                    pick_random_hardware_pair()
                };
                if let Some(p) = db.get_mut(&current_name) {
                    p.cpu_name = c.name.to_string();
                    p.cpu = c.threads;
                    p.ram = c.ram;
                    p.gpu_name = g.name.to_string();
                    p.gpu_vendor = g.vendor.to_string();
                    p.gpu_renderer = g.renderer.to_string();
                }
                let _ = save_db(db);
                println!(
                    "\n  {} Железо обновлено: {} / {}",
                    c_online("[+]"),
                    c_cream(c.name),
                    c_amber(g.name)
                );
                pause("Нажмите Enter...");
            }
            "3" => {
                println!("\n  {} 1920x1080 (FullHD)", key_badge("1"));
                println!("  {} 1366x768  (Ноутбук)", key_badge("2"));
                println!("  {} 1600x900  (HD+)", key_badge("3"));
                println!("  {} 2560x1440 (2K QHD)", key_badge("4"));
                println!("  {} Ввести вручную", key_badge("5"));

                let res = match prompt("Выбор [1-5]").as_str() {
                    "1" => Some((1920, 1080)),
                    "2" => Some((1366, 768)),
                    "3" => Some((1600, 900)),
                    "4" => Some((2560, 1440)),
                    "5" => {
                        let w = prompt("Ширина (800-7680)").parse::<u32>().ok();
                        let h = prompt("Высота (600-4320)").parse::<u32>().ok();
                        match (w, h) {
                            (Some(w), Some(h))
                                if (800..=7680).contains(&w) && (600..=4320).contains(&h) =>
                            {
                                Some((w, h))
                            }
                            _ => {
                                println!("\n  {} Некорректные размеры экрана.", c_err("[-]"));
                                pause("Нажмите Enter...");
                                None
                            }
                        }
                    }
                    _ => None,
                };
                if let Some((w, h)) = res {
                    if let Some(p) = db.get_mut(&current_name) {
                        p.width = w;
                        p.height = h;
                    }
                    let _ = save_db(db);
                }
            }
            "4" => {
                match configure_proxy_flow(true).await {
                    ProxyFlowResult::Selected {
                        proxy,
                        timezone,
                        latitude,
                        longitude,
                    } => {
                        if let Some(p) = db.get_mut(&current_name) {
                            p.proxy = Some(proxy);
                            if let Some(tz) = timezone {
                                p.timezone = tz;
                            }
                            p.latitude = latitude;
                            p.longitude = longitude;
                        }
                        let _ = save_db(db);
                    }
                    ProxyFlowResult::Removed => {
                        if let Some(p) = db.get_mut(&current_name) {
                            p.proxy = None;
                            p.latitude = None;
                            p.longitude = None;
                        }
                        let _ = save_db(db);
                        println!("\n  {} Прокси отключён для этого профиля.", c_online("[+]"));
                        pause("Нажмите Enter...");
                    }
                    ProxyFlowResult::Cancelled => {}
                }
            }
            "5" => {
                if let Some(ref p_cfg) = prof.proxy {
                    println!(
                        "\n  {} Проверка {}...",
                        c_amber("[*]"),
                        c_gold(&p_cfg.display_short())
                    );
                    match check_proxy(p_cfg, 7).await {
                        Ok(info) => {
                            if let Some(p) = db.get_mut(&current_name) {
                                if let Some(ref tz) = info.timezone {
                                    p.timezone = tz.clone();
                                }
                                if info.latitude.is_some() {
                                    p.latitude = info.latitude;
                                    p.longitude = info.longitude;
                                }
                            }
                            let _ = save_db(db);
                            println!(
                                "  {} Рабочий! IP: {} ({}, {} • {} • TZ: {})",
                                c_online("[+]"),
                                c_gold(&info.ip),
                                info.country,
                                info.city,
                                info.proxy_type,
                                info.timezone.as_deref().unwrap_or(&prof.timezone)
                            );
                            print_datacenter_hint(&info.proxy_type);
                        }
                        Err(e) => {
                            println!("  {} Недоступен: {}", c_err("[-]"), c_cream(&e));
                        }
                    }
                } else {
                    println!("\n  {} У профиля не задан прокси.", c_muted("[-]"));
                }
                pause("Нажмите Enter...");
            }
            "6" => {
                let tz = prompt(&format!("Часовой пояс [текущий: {}]", prof.timezone));
                if !tz.is_empty() {
                    if let Some(p) = db.get_mut(&current_name) {
                        p.timezone = normalize_timezone(&tz);
                    }
                    let _ = save_db(db);
                }
            }
            "7" => {
                let new_seed = generate_seed();
                if let Some(p) = db.get_mut(&current_name) {
                    p.noise_seed = new_seed;
                }
                let _ = save_db(db);
                println!(
                    "\n  {} Обновлён Seed шума (Canvas/Audio): #{:08X}!",
                    c_online("[+]"),
                    new_seed & 0xFFFF_FFFF
                );
                pause("Нажмите Enter...");
            }
            "8" => {
                println!("\n  {} Чтение cookies профиля...", c_amber("[*]"));
                match export_profile_cookies(&current_name, &prof, sessions).await {
                    Ok(cookies) if cookies.is_empty() => {
                        println!(
                            "\n  {} В профиле '{}' нет сохранённых cookies.",
                            c_muted("[-]"),
                            c_gold(&current_name)
                        );
                        pause("Нажмите Enter...");
                    }
                    Ok(cookies) => {
                        println!(
                            "\n  {} Найдено {} cookies!",
                            c_online("[+]"),
                            c_gold(&cookies.len().to_string())
                        );
                        println!("  {} Сохранить в JSON-файл", key_badge("1"));
                        println!("  {} Сохранить в Netscape-файл (.txt)", key_badge("2"));
                        println!("  {} Вывести на экран в формате JSON", key_badge("3"));
                        println!("  {} Вывести на экран в формате Netscape", key_badge("4"));
                        println!("  {} Отмена", key_badge("0"));

                        match prompt("Ваш выбор [1-4]").as_str() {
                            "1" => {
                                let default_name =
                                    format!("cookies_{}.json", current_name.replace(' ', "_"));
                                let path_str = prompt(&format!(
                                    "Путь к файлу [по умолчанию: {}]",
                                    default_name
                                ));
                                let out_path = if path_str.is_empty() {
                                    default_name
                                } else {
                                    path_str
                                };
                                let content = export_to_json(&cookies);
                                if let Err(e) = fs::write(&out_path, content) {
                                    println!("\n  {} Ошибка записи файла: {e}", c_err("[-]"));
                                } else {
                                    println!(
                                        "\n  {} Cookies успешно сохранены в: {}",
                                        c_online("[+]"),
                                        c_gold(&out_path)
                                    );
                                }
                                pause("Нажмите Enter...");
                            }
                            "2" => {
                                let default_name =
                                    format!("cookies_{}.txt", current_name.replace(' ', "_"));
                                let path_str = prompt(&format!(
                                    "Путь к файлу [по умолчанию: {}]",
                                    default_name
                                ));
                                let out_path = if path_str.is_empty() {
                                    default_name
                                } else {
                                    path_str
                                };
                                let content = export_to_netscape(&cookies);
                                if let Err(e) = fs::write(&out_path, content) {
                                    println!("\n  {} Ошибка записи файла: {e}", c_err("[-]"));
                                } else {
                                    println!(
                                        "\n  {} Cookies успешно сохранены в: {}",
                                        c_online("[+]"),
                                        c_gold(&out_path)
                                    );
                                }
                                pause("Нажмите Enter...");
                            }
                            "3" => {
                                println!("\n{}", c_steel(&export_to_json(&cookies)));
                                pause("Нажмите Enter для продолжения...");
                            }
                            "4" => {
                                println!("\n{}", c_steel(&export_to_netscape(&cookies)));
                                pause("Нажмите Enter для продолжения...");
                            }
                            _ => {}
                        }
                    }
                    Err(err) => {
                        println!("\n  {} Ошибка экспорта cookies: {err}", c_err("[-]"));
                        pause("Нажмите Enter...");
                    }
                }
            }
            "9" => {
                println!("{}", section_header("ИМПОРТ COOKIES (JSON / Netscape)"));
                println!(
                    "  Поддерживаются форматы JSON (массив объектов) и Netscape (tab-separated txt)."
                );
                let raw_input = prompt("Укажите путь к файлу с cookies или вставьте текст");
                if !raw_input.is_empty() {
                    let p = std::path::Path::new(&raw_input);
                    let content = if p.is_file() {
                        match fs::read_to_string(p) {
                            Ok(s) => s,
                            Err(e) => {
                                println!(
                                    "\n  {} Не удалось прочитать файл '{}': {e}",
                                    c_err("[-]"),
                                    p.display()
                                );
                                pause("Нажмите Enter...");
                                continue;
                            }
                        }
                    } else {
                        raw_input
                    };

                    match parse_cookies(&content) {
                        Ok(cookies) => {
                            match import_profile_cookies(&current_name, &prof, sessions, &cookies)
                                .await
                            {
                                Ok(count) => {
                                    let live_tag = if is_online {
                                        " (применено на лету в активный браузер!)"
                                    } else {
                                        " (будут загружены при запуске браузера)"
                                    };
                                    println!(
                                        "\n  {} Успешно импортировано {} cookies{}!",
                                        c_online("[+]"),
                                        c_gold(&count.to_string()),
                                        c_steel(live_tag)
                                    );
                                }
                                Err(e) => {
                                    println!("\n  {} Ошибка импорта: {e}", c_err("[-]"));
                                }
                            }
                        }
                        Err(e) => {
                            println!("\n  {} Не удалось распознать cookies: {e}", c_err("[-]"));
                        }
                    }
                    pause("Нажмите Enter...");
                }
            }
            "d" | "D" | "del" => {
                let confirm = prompt(&format!(
                    "Удалить профиль '{}' и все куки/сессию? (y/n)",
                    current_name
                ))
                .to_ascii_lowercase();
                if confirm == "y" || confirm == "д" {
                    if is_online {
                        sessions.stop_session(&current_name);
                    }
                    let s_dir = session_dir_for_profile(&current_name, &prof);
                    db.remove(&current_name);
                    let _ = save_db(db);
                    if s_dir.exists() {
                        let _ = fs::remove_dir_all(&s_dir);
                    }
                    println!("\n  {} Профиль полностью удалён.", c_online("[+]"));
                    pause("Нажмите Enter...");
                    return None;
                }
            }
            "0" | "" => break,
            _ => {}
        }
    }

    Some(current_name)
}

pub async fn launch_selected_flow(
    db: &mut ProfileDb,
    profile_name: &str,
    sessions: &SessionManager,
) {
    if sessions.is_running(profile_name) {
        println!(
            "\n  {} Профиль {} уже запущен в фоне!",
            c_warn("[!]"),
            c_gold(profile_name)
        );
        let ans = prompt("Перезапустить окно профиля? (y/n)").to_ascii_lowercase();
        if ans == "y" || ans == "д" {
            sessions.stop_session(profile_name);
        } else {
            return;
        }
    }

    let Some(mut prof) = db.get(profile_name).cloned() else {
        return;
    };

    print_hardware_warnings(&prof);

    let mut proxy_geo_synced = false;
    if let Some(ref p_cfg) = prof.proxy {
        println!(
            "\n  {} Проверка прокси {} перед стартом...",
            c_amber("[*]"),
            c_gold(&p_cfg.display_short())
        );
        match check_proxy(p_cfg, 10).await {
            Ok(info) => {
                if let Some(ref tz) = info.timezone {
                    prof.timezone = tz.clone();
                    proxy_geo_synced = true;
                }
                if info.latitude.is_some() {
                    prof.latitude = info.latitude;
                    prof.longitude = info.longitude;
                }
                if let Some(stored) = db.get_mut(profile_name) {
                    stored.timezone = prof.timezone.clone();
                    stored.latitude = prof.latitude;
                    stored.longitude = prof.longitude;
                }
                let _ = save_db(db);
                println!(
                    "  {} Прокси OK: {} ({}, {} • TZ: {})",
                    c_online("[+]"),
                    c_gold(&info.ip),
                    info.country,
                    info.proxy_type,
                    c_amber(&prof.timezone)
                );
                print_datacenter_hint(&info.proxy_type);
            }
            Err(err) => {
                println!(
                    "\n  {} Прокси не ответил вовремя: {}",
                    c_warn("[!]"),
                    c_cream(&err)
                );
                let force = prompt("Запустить браузер всё равно с текущими настройками? (y/n)")
                    .to_ascii_lowercase();
                if force != "y" && force != "д" {
                    println!("  {}", c_err("[-] Запуск отменён."));
                    pause("Нажмите Enter для возврата в меню...");
                    return;
                }
            }
        }
    }

    println!(
        "  {} Запуск нативного движка Chrome для [{}]...",
        c_amber("[*]"),
        c_gold(profile_name)
    );
    let started = Instant::now();
    let proxy_clone = prof.proxy.clone();

    match launch_profile_session(profile_name, &prof, proxy_clone, !proxy_geo_synced).await {
        Ok(running) => {
            let elapsed = started.elapsed().as_secs_f32();
            sessions.insert(running);
            println!(
                "\n  {} Браузер запущен за {:.2} сек! Сессия работает параллельно в фоне.",
                c_online("[+]"),
                elapsed
            );
            println!(
                "  {}",
                c_muted("Можете сразу запускать другие профили или управлять окнами в меню [5].")
            );
            pause("Нажмите Enter для возврата в панель управления...");
        }
        Err(e) => {
            println!("\n  {} Ошибка запуска: {}", c_err("[-]"), c_cream(&e));
            pause("Нажмите Enter...");
        }
    }
}

pub fn manage_active_sessions_flow(sessions: &SessionManager) {
    loop {
        clear_screen();
        let active = sessions.list_active();
        println!(
            "{}",
            section_header(&format!("АКТИВНЫЕ СЕССИИ БРАУЗЕРА ({})", active.len()))
        );
        println!("{}", divider(74));

        if active.is_empty() {
            println!("  {}", c_muted("Сейчас нет открытых окон браузера."));
            pause("Нажмите Enter для возврата...");
            return;
        }

        for (idx, (name, prx, up_secs)) in active.iter().enumerate() {
            println!(
                "  {} {}  {}  │ {}  │ Up: {}",
                key_badge(&format!("{:2}", idx + 1)),
                c_online("[ONLINE]"),
                c_gold(name),
                c_amber(prx),
                c_steel(&format_uptime(*up_secs))
            );
        }

        println!("{}", thin_divider(74));
        println!(
            "  {} Закрыть по номеру   {} Закрыть ВСЕ сессии   {} Обновить   {} Назад",
            key_badge("1..N"),
            key_badge("A"),
            key_badge("R"),
            key_badge("0")
        );

        let cmd = prompt("Команда").to_ascii_lowercase();
        match cmd.as_str() {
            "0" | "" => return,
            "r" => continue,
            "a" => {
                sessions.stop_all();
                println!("\n  {} Все сессии и прокси-мосты остановлены.", c_online("[+]"));
                pause("Нажмите Enter...");
                return;
            }
            _ => {
                if let Ok(n) = cmd.parse::<usize>() {
                    if (1..=active.len()).contains(&n) {
                        let target = &active[n - 1].0;
                        sessions.stop_session(target);
                    }
                }
            }
        }
    }
}
