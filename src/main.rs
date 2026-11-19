mod browser;
mod cookies;
mod fingerprint;
mod flows;
mod hardware;
mod host;
mod models;
mod proxy;
mod secrets;
mod ui;

use browser::SessionManager;
use flows::{
    create_profile_flow, edit_profile_flow, launch_selected_flow, manage_active_sessions_flow,
    print_hardware_warnings, select_profile_flow,
};
use models::load_db;
use ui::{
    c_amber, c_cream, c_gold, c_muted, c_online, c_steel, clear_screen, divider, init_terminal,
    key_badge, pause, prompt, render_banner, thin_divider,
};

#[tokio::main]
async fn main() {
    init_terminal();

    let sessions = SessionManager::new();
    let mut selected_profile: Option<String> = None;

    loop {
        sessions.refresh();
        let mut db = load_db();

        if let Some(ref sel) = selected_profile {
            if !db.contains_key(sel) {
                selected_profile = None;
            }
        }
        if selected_profile.is_none() && db.len() == 1 {
            selected_profile = db.keys().next().cloned();
        }

        clear_screen();
        print!("{}", render_banner());
        println!("{}", divider(72));

        let active_cnt = sessions.active_count();
        let online_pill = if active_cnt > 0 {
            c_online(&format!("[ONLINE: {active_cnt}]"))
        } else {
            c_muted("[ONLINE: 0]")
        };
        println!(
            "  Профилей в базе: {}   │   Активных окон: {}",
            c_gold(&db.len().to_string()),
            online_pill
        );
        println!("{}", thin_divider(72));

        if let Some(ref sel_name) = selected_profile {
            if let Some(prof) = db.get(sel_name) {
                let state_tag = if sessions.is_running(sel_name) {
                    c_online("[ONLINE]")
                } else {
                    c_muted("[ГОТОВ]")
                };
                let prx_tag = prof
                    .proxy
                    .as_ref()
                    .map(|p| c_amber(&p.display_short()))
                    .unwrap_or_else(|| c_muted("Без прокси"));
                println!(
                    "  Выбран профиль : {} {}",
                    c_gold(sel_name),
                    state_tag
                );
                println!(
                    "  Конфигурация   : {}  •  {}  •  {}",
                    c_cream(&format!("{}x{}", prof.width, prof.height)),
                    c_steel(&prof.timezone),
                    prx_tag
                );
                print_hardware_warnings(prof);
            }
        } else {
            println!("  Выбран профиль : {}", c_muted("не выбран"));
        }

        println!("{}", divider(72));
        println!("  {} Создать новый профиль", key_badge("1"));
        println!("  {} Список профилей и выбор", key_badge("2"));
        println!("  {} Запустить выбранный профиль", key_badge("3"));
        println!("  {} Редактировать профиль", key_badge("4"));
        println!(
            "  {} Управление активными сессиями {}",
            key_badge("5"),
            if active_cnt > 0 {
                c_online(&format!("({active_cnt})"))
            } else {
                String::new()
            }
        );
        println!("  {} Выход из RUST ANTY", key_badge("0"));
        println!("{}", divider(72));

        match prompt("Выберите пункт").as_str() {
            "1" => {
                if let Some(created) = create_profile_flow(&mut db).await {
                    selected_profile = Some(created);
                }
            }
            "2" => {
                if let Some(chosen) = select_profile_flow(&db, &sessions) {
                    selected_profile = Some(chosen);
                }
            }
            "3" => {
                let target = match selected_profile.clone() {
                    Some(name) => Some(name),
                    None => select_profile_flow(&db, &sessions),
                };
                if let Some(name) = target {
                    selected_profile = Some(name.clone());
                    launch_selected_flow(&mut db, &name, &sessions).await;
                }
            }
            "4" => {
                if db.is_empty() {
                    println!("\n  {} База профилей пуста.", c_muted("[-]"));
                    pause("Нажмите Enter...");
                    continue;
                }
                let target = match selected_profile.clone() {
                    Some(name) => Some(name),
                    None => select_profile_flow(&db, &sessions),
                };
                if let Some(name) = target {
                    selected_profile = edit_profile_flow(&mut db, name, &sessions).await;
                }
            }
            "5" => {
                manage_active_sessions_flow(&sessions);
            }
            "0" | "6" => {
                let running = sessions.active_count();
                if running > 0 {
                    let ans = prompt(&format!(
                        "Открыто {running} сессий Chrome. Закрыть их и остановить прокси-мосты перед выходом? (y/n)"
                    ))
                    .to_ascii_lowercase();
                    if ans != "n" && ans != "н" {
                        sessions.stop_all();
                    }
                }
                println!("\n  До встречи в {}!\n", c_gold("RUST ANTY"));
                break;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::browser;
    use crate::cookies;
    use crate::fingerprint;
    use crate::hardware::{
        generate_random_fonts, is_compatible, pick_random_hardware_pair, web_device_memory,
        CpuBrand, GpuKind,
    };
    use crate::host::{classify_gpu_name, cpu_ok, gpu_match, host_info, GpuMatch, GpuVendor};
    use crate::models::{generate_seed, validate_profile_name, Profile, ProfileDb, ProxyConfig};
    use crate::proxy::parse_proxy;

    #[test]
    fn test_proxy_formats_parsing() {
        let p1 = parse_proxy("185.199.108.153:8080", "http").unwrap();
        assert_eq!(p1.scheme, "http");
        assert_eq!(p1.host, "185.199.108.153");
        assert_eq!(p1.port, 8080);
        assert_eq!(p1.user, None);

        let p2 = parse_proxy("45.12.34.56:1080:ivan:secret123", "socks5").unwrap();
        assert_eq!(p2.scheme, "socks5");
        assert_eq!(p2.host, "45.12.34.56");
        assert_eq!(p2.port, 1080);
        assert_eq!(p2.user.as_deref(), Some("ivan"));
        assert_eq!(p2.password.as_deref(), Some("secret123"));

        let p3 = parse_proxy(
            "socks5://customer-test.country-de.session-99:pass_word@pr.oxylabs.io:7777",
            "http",
        )
        .unwrap();
        assert_eq!(p3.scheme, "socks5");
        assert_eq!(p3.host, "pr.oxylabs.io");
        assert_eq!(p3.port, 7777);
        assert_eq!(
            p3.user.as_deref(),
            Some("customer-test.country-de.session-99")
        );
        assert_eq!(p3.password.as_deref(), Some("pass_word"));

        let p4 = parse_proxy("192.168.1.50:9000@myuser:mypass", "http").unwrap();
        assert_eq!(p4.host, "192.168.1.50");
        assert_eq!(p4.port, 9000);
        assert_eq!(p4.user.as_deref(), Some("myuser"));
        assert_eq!(p4.password.as_deref(), Some("mypass"));
    }

    #[test]
    fn test_device_memory_w3c_cap() {
        assert_eq!(web_device_memory(2), 2);
        assert_eq!(web_device_memory(4), 4);
        assert_eq!(web_device_memory(8), 8);
        assert_eq!(web_device_memory(16), 8);
        assert_eq!(web_device_memory(32), 8);
        assert_eq!(web_device_memory(64), 8);
    }

    #[test]
    fn test_host_gpu_classification() {
        assert!(classify_gpu_name("MuMuPlayer Virtual Display Adapter").is_none());
        assert!(classify_gpu_name("Microsoft Basic Display Adapter").is_none());
        assert!(classify_gpu_name("Parsec Virtual Display Adapter").is_none());

        let amd_d = classify_gpu_name("AMD Radeon RX 6600 XT").unwrap();
        assert_eq!(amd_d.vendor, GpuVendor::Amd);
        assert!(!amd_d.integrated);
        assert_eq!(amd_d.arch, "rdna-2");

        let amd_i = classify_gpu_name("AMD Radeon(TM) Vega 8 Graphics").unwrap();
        assert_eq!(amd_i.vendor, GpuVendor::Amd);
        assert!(amd_i.integrated);

        let nv = classify_gpu_name("NVIDIA GeForce RTX 3070").unwrap();
        assert_eq!(nv.vendor, GpuVendor::Nvidia);
        assert!(!nv.integrated);
        assert_eq!(nv.arch, "ampere");

        let intel_i = classify_gpu_name("Intel(R) UHD Graphics 770").unwrap();
        assert_eq!(intel_i.vendor, GpuVendor::Intel);
        assert!(intel_i.integrated);
    }

    #[test]
    fn test_hardware_random_pair_always_compatible_and_host_consistent() {
        let host = host_info();
        for _ in 0..500 {
            let (cpu, gpu) = pick_random_hardware_pair();
            assert!(is_compatible(cpu, gpu));
            assert!(
                cpu_ok(cpu),
                "Random CPU {} ({} threads) exceeded host cores ({})",
                cpu.name,
                cpu.threads,
                host.logical_cores
            );
            if host.gpu.is_some() {
                assert_ne!(
                    gpu_match(gpu),
                    GpuMatch::WrongVendor,
                    "Random GPU {} mismatched host GPU vendor {:?}",
                    gpu.name,
                    host.gpu
                );
            }
            if gpu.kind == GpuKind::IntelIgpu {
                assert_eq!(cpu.brand, CpuBrand::Intel);
                assert!(cpu.has_igpu);
            }
            if gpu.kind == GpuKind::AmdIgpu {
                assert_eq!(cpu.brand, CpuBrand::Amd);
                assert!(cpu.has_igpu);
            }
        }
    }

    #[test]
    fn test_profile_name_validation() {
        assert!(validate_profile_name("Рабочий_Акк_1").is_ok());
        assert!(validate_profile_name("Shop #42").is_ok());
        assert!(validate_profile_name("").is_err());
        assert!(validate_profile_name("..").is_err());
        assert!(validate_profile_name("bad/name").is_err());
        assert!(validate_profile_name("CON").is_err());
    }

    #[test]
    fn test_legacy_profiliki_json_compat_and_dpapi_encryption() {
        let legacy_json = r#"{
            "test_prof": {
                "cpu_name": "AMD Ryzen 5 5600X",
                "cpu": 12,
                "ram": 32,
                "gpu_name": "AMD Radeon RX 6600",
                "gpu_vendor": "Google Inc. (AMD)",
                "gpu_renderer": "ANGLE (AMD, AMD Radeon RX 6600 Direct3D11 vs_5_0 ps_5_0, D3D11)",
                "width": 1920,
                "height": 1080,
                "timezone": "Europe/Kiev",
                "fonts": ["Arial", "Segoe UI"],
                "proxy": {
                    "scheme": "socks5",
                    "host": "127.0.0.1",
                    "port": 1080,
                    "user": "u",
                    "pass": "my_plaintext_secret_99"
                }
            }
        }"#;
        let parsed: ProfileDb = serde_json::from_str(legacy_json).unwrap();
        let p = &parsed["test_prof"];
        assert_eq!(p.cpu, 12);
        assert!(p.noise_seed > 0);
        assert_eq!(
            p.proxy.as_ref().unwrap().password.as_deref(),
            Some("my_plaintext_secret_99")
        );

        // Serializing back must encrypt the password via DPAPI on Windows
        let reserialized = serde_json::to_string_pretty(&parsed).unwrap();
        #[cfg(windows)]
        {
            assert!(
                !reserialized.contains("my_plaintext_secret_99"),
                "Plaintext password leaked into JSON!"
            );
            assert!(reserialized.contains("\"dpapi:"));
        }

        // Round-trip deserialize recovers the exact password in memory
        let roundtrip: ProfileDb = serde_json::from_str(&reserialized).unwrap();
        assert_eq!(
            roundtrip["test_prof"].proxy.as_ref().unwrap().password.as_deref(),
            Some("my_plaintext_secret_99")
        );
    }

    #[tokio::test]
    async fn test_live_chrome_zero_mask_detection() {
        use std::process::{Command, Stdio};
        use std::time::Duration;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let Some(chrome_bin) = browser::find_chrome_binary() else {
            return;
        };

        let probe_html = r#"<!DOCTYPE html>
<html><body><script>
(async () => {
    const out = { errors: [] };
    function check(cond, msg) {
        if (!cond) out.errors.push(msg);
    }

    // 1. Webdriver & window globals (must remain 100% native prototype, zero own properties)
    check(navigator.webdriver === false, 'webdriver !== false: ' + navigator.webdriver);
    check(!Object.prototype.hasOwnProperty.call(navigator, 'webdriver'), 'navigator has own webdriver property!');
    const winProps = Object.getOwnPropertyNames(window);
    check(!winProps.includes('__rustAntyApplied'), '__rustAntyApplied leaked on window');
    check(!winProps.some(k => k.startsWith('cdc_')), 'cdc_ leaked on window');

    // 2. Prototype & toString lies
    check(!('prototype' in Function.prototype.toString), 'toString has .prototype');
    check(!('prototype' in WebGLRenderingContext.prototype.getParameter), 'getParameter has .prototype');
    check(!('prototype' in CanvasRenderingContext2D.prototype.measureText), 'measureText has .prototype');
    check(
        Function.prototype.toString.call(WebGLRenderingContext.prototype.getParameter) === 'function getParameter() { [native code] }',
        'bad getParameter toString'
    );
    const hcGet = Object.getOwnPropertyDescriptor(Navigator.prototype, 'hardwareConcurrency').get;
    check(
        Function.prototype.toString.call(hcGet) === 'function get hardwareConcurrency() { [native code] }',
        'bad getter toString: ' + Function.prototype.toString.call(hcGet)
    );

    // 3. Illegal invocation & stack trace check (both getter AND wrapped method!)
    let threwIllegal = false;
    try {
        hcGet.call({});
    } catch (e) {
        threwIllegal = e instanceof TypeError;
        check(!/\(<anonymous>:\d+:\d+\)/.test(String(e.stack).split('\n')[1] || ''), 'getter stack leaked line:col: ' + e.stack);
        check(!String(e.stack).includes('invokeNative') && !String(e.stack).includes('invokeHandler'), 'getter stack leaked helper name: ' + e.stack);
    }
    check(threwIllegal, 'getter on {} did not throw TypeError');

    let glThrewIllegal = false;
    try {
        WebGLRenderingContext.prototype.getParameter.call({}, 37445);
    } catch (e) {
        glThrewIllegal = e instanceof TypeError;
        check(!/\(<anonymous>:\d+:\d+\)/.test(String(e.stack).split('\n')[1] || ''), 'method stack leaked line:col: ' + e.stack);
        check(!String(e.stack).includes('invokeNative') && !String(e.stack).includes('invokeHandler'), 'method stack leaked helper name: ' + e.stack);
    }
    check(glThrewIllegal, 'getParameter.call({}) did not throw TypeError');

    // 4. WebGL pre-extension null + post-extension spoof
    const c = document.createElement('canvas');
    const gl = c.getContext('webgl');
    const beforeExt = gl.getParameter(37445);
    const errBefore = gl.getError();
    check(beforeExt === null, 'WebGL leaked vendor before getExtension: ' + beforeExt);
    check(errBefore === gl.INVALID_ENUM, 'WebGL error before getExtension != INVALID_ENUM');
    const dbg = gl.getExtension('WEBGL_debug_renderer_info');
    out.navCpu = navigator.hardwareConcurrency;
    out.webglActual = [gl.getParameter(dbg.UNMASKED_VENDOR_WEBGL), gl.getParameter(dbg.UNMASKED_RENDERER_WEBGL)];
    check(navigator.hardwareConcurrency === 8, 'wrong main CPU: ' + navigator.hardwareConcurrency);
    check(navigator.deviceMemory === 8, 'wrong main deviceMemory: ' + navigator.deviceMemory);
    check(gl.getParameter(dbg.UNMASKED_VENDOR_WEBGL) === 'Google Inc. (AMD)', 'wrong WebGL vendor');
    check(String(gl.getParameter(dbg.UNMASKED_RENDERER_WEBGL)).includes('RX 6600'), 'wrong WebGL renderer');

    // 5. Canvas solid pixel test (no lie on solid fillRect)
    const c2 = document.createElement('canvas');
    c2.width = 16; c2.height = 16;
    const ctx = c2.getContext('2d');
    ctx.fillStyle = '#123456';
    ctx.fillRect(0, 0, 8, 16);
    ctx.fillStyle = '#654321';
    ctx.fillRect(8, 0, 8, 16);
    const solidData = ctx.getImageData(0, 0, 16, 16).data;
    check(solidData[0] === 0x12 && solidData[1] === 0x34 && solidData[2] === 0x56, 'solid fillRect left mutated');
    const idxRight = 8 * 4;
    check(solidData[idxRight] === 0x65 && solidData[idxRight + 1] === 0x43 && solidData[idxRight + 2] === 0x21, 'solid fillRect right mutated');

    // 6. Screen & matchMedia consistency (including zero own property on MediaQueryList)
    const mql = window.matchMedia('(device-width: 1920px)');
    check(!Object.prototype.hasOwnProperty.call(mql, 'media'), 'MediaQueryList has own media property!');
    check(screen.width === 1920 && screen.height === 1080, 'screen dimensions mismatch');
    check(mql.matches === true, 'matchMedia device-width failed');
    check(mql.media === '(device-width: 1920px)', 'matchMedia .media string corrupted: ' + mql.media);
    check(window.outerWidth <= screen.width, 'outerWidth > screen.width');

    // 7. Synchronous same-origin iframe (`window[0]`, `contentWindow`, `insertAdjacentHTML`, `srcdoc`)
    const ifr = document.createElement('iframe');
    document.body.appendChild(ifr);
    check(window[0].navigator.hardwareConcurrency === 8, 'window[0] leaked real CPU: ' + window[0].navigator.hardwareConcurrency);
    check(ifr.contentWindow.navigator.deviceMemory === 8, 'iframe leaked deviceMemory');
    const igl = window[0].document.createElement('canvas').getContext('webgl');
    const idbg = igl.getExtension('WEBGL_debug_renderer_info');
    check(String(igl.getParameter(idbg.UNMASKED_RENDERER_WEBGL)).includes('RX 6600'), 'iframe leaked real GPU');

    document.body.insertAdjacentHTML('beforeend', '<iframe id="adj"></iframe>');
    check(window[1].navigator.hardwareConcurrency === 8, 'insertAdjacentHTML iframe leaked real CPU: ' + window[1].navigator.hardwareConcurrency);

    const srcdocCpu = await new Promise((resolve) => {
        window.__srcdocCb = (v) => resolve(v);
        const f = document.createElement('iframe');
        f.srcdoc = '<script>parent.__srcdocCb(navigator.hardwareConcurrency);<\/script>';
        document.body.appendChild(f);
        setTimeout(() => resolve(-1), 2500);
    });
    check(srcdocCpu === 8, 'srcdoc iframe leaked real CPU: ' + srcdocCpu);

    // 8. Dedicated Web Worker consistency (CPU, Memory, Timezone, Locale, OffscreenCanvas WebGL)
    const workerRes = await new Promise((resolve) => {
        const timer = setTimeout(() => resolve({ timeout: true }), 4000);
        const wBlob = new Blob([`
            let wVendor = null, wRenderer = null, preNull = false;
            try {
                const oc = new OffscreenCanvas(4, 4);
                const gl = oc.getContext('webgl');
                preNull = (gl.getParameter(37445) === null);
                const ext = gl.getExtension('WEBGL_debug_renderer_info');
                wVendor = gl.getParameter(ext.UNMASKED_VENDOR_WEBGL);
                wRenderer = gl.getParameter(ext.UNMASKED_RENDERER_WEBGL);
            } catch (e) {}
            postMessage({
                hc: navigator.hardwareConcurrency,
                dm: navigator.deviceMemory,
                tz: Intl.DateTimeFormat().resolvedOptions().timeZone,
                lang: navigator.language,
                preNull,
                wVendor,
                wRenderer
            });
        `], { type: 'application/javascript' });
        const w = new Worker(URL.createObjectURL(wBlob));
        w.onmessage = (ev) => { clearTimeout(timer); resolve(ev.data); };
    });

    out.winTz = Intl.DateTimeFormat().resolvedOptions().timeZone;
    out.winLang = navigator.language;
    out.dateStr = new Date().toString();
    out.webdriverVal = navigator.webdriver;
    out.worker = workerRes;

    check(!workerRes.timeout, 'Worker timed out (paused or blocked!)');
    check(workerRes.hc === 8, 'Worker leaked CPU: ' + workerRes.hc);
    check(workerRes.dm === 8, 'Worker leaked deviceMemory: ' + workerRes.dm);
    check(workerRes.tz === 'Europe/London', 'Worker leaked Timezone: ' + workerRes.tz + ' vs Europe/London');
    check(out.winTz === 'Europe/London', 'Window Timezone mismatch: ' + out.winTz);
    check(workerRes.preNull === true, 'Worker WebGL leaked before getExtension');
    check(workerRes.wVendor === 'Google Inc. (AMD)', 'Worker leaked GPU vendor: ' + workerRes.wVendor);
    check(String(workerRes.wRenderer).includes('RX 6600'), 'Worker leaked GPU renderer: ' + workerRes.wRenderer);

    await fetch('/report', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(out)
    });
})();
</script></body></html>"#;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let srv_port = listener.local_addr().unwrap().port();
        let (report_tx, report_rx) = tokio::sync::oneshot::channel::<String>();

        tokio::spawn(async move {
            let mut report_tx = Some(report_tx);
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let mut buf = Vec::new();
                let mut tmp = [0u8; 4096];
                loop {
                    let Ok(n) = stream.read(&mut tmp).await else { break; };
                    if n == 0 { break; }
                    buf.extend_from_slice(&tmp[..n]);
                    if let Some(hdr_pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        let hdr_str = String::from_utf8_lossy(&buf[..hdr_pos]);
                        let cl: usize = hdr_str
                            .lines()
                            .find(|l| l.to_ascii_lowercase().starts_with("content-length:"))
                            .and_then(|l| l.split_once(':'))
                            .and_then(|(_, v)| v.trim().parse().ok())
                            .unwrap_or(0);
                        if buf.len() >= hdr_pos + 4 + cl {
                            break;
                        }
                    }
                }
                let req = String::from_utf8_lossy(&buf);
                if req.starts_with("POST /report") {
                    if let Some(idx) = req.find("\r\n\r\n") {
                        let body = req[idx + 4..].to_string();
                        if let Some(tx) = report_tx.take() {
                            let _ = tx.send(body);
                        }
                    }
                    let _ = stream
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                        .await;
                    break;
                } else {
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
                        probe_html.len(),
                        probe_html
                    );
                    let _ = stream.write_all(resp.as_bytes()).await;
                }
            }
        });

        let tmp_dir = std::env::temp_dir().join(format!("rust_anty_test_{}", generate_seed()));
        let _ = fs::create_dir_all(&tmp_dir);

        let profile = Profile {
            cpu_name: "AMD Ryzen 3 4300G".into(),
            cpu: 8,
            ram: 16,
            gpu_name: "AMD Radeon RX 6600".into(),
            gpu_vendor: "Google Inc. (AMD)".into(),
            gpu_renderer: "ANGLE (AMD, AMD Radeon RX 6600 (0x000073FF) Direct3D11 vs_5_0 ps_5_0, D3D11)".into(),
            width: 1920,
            height: 1080,
            timezone: "Europe/London".into(),
            fonts: generate_random_fonts(),
            proxy: None,
            noise_seed: 2928503833,
            folder_id: None,
            latitude: Some(51.5085),
            longitude: Some(-0.1257),
        };

        let cdp_port = browser::pick_ephemeral_port().unwrap();
        let mut child = Command::new(&chrome_bin)
            .args([
                &format!("--user-data-dir={}", tmp_dir.display()),
                &format!("--remote-debugging-port={cdp_port}"),
                "--headless=new",
                "--no-first-run",
                "--no-default-browser-check",
                "--force-device-scale-factor=1",
                "--lang=en-GB",
                "--window-size=1560,920",
                "about:blank",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();

        let ws_url = browser::wait_for_cdp_ws_url(cdp_port, &mut child).await.unwrap();
        let stealth_js = fingerprint::build_stealth_script(&profile);
        let stop_tx = browser::start_cdp_stealth_guardian(
            ws_url.clone(),
            stealth_js,
            "Europe/London".into(),
            "en-GB".into(),
            Some((51.5085, -0.1257)),
            None,
        )
        .await
        .unwrap();

        let (mut ws, _) = tokio_tungstenite::connect_async(&ws_url).await.unwrap();
        use futures_util::SinkExt;
        let nav_cmd = serde_json::json!({
            "id": 99,
            "method": "Target.createTarget",
            "params": { "url": format!("http://127.0.0.1:{srv_port}/") }
        });
        ws.send(tokio_tungstenite::tungstenite::Message::Text(
            nav_cmd.to_string().into(),
        ))
        .await
        .unwrap();

        let report_json = tokio::time::timeout(Duration::from_secs(10), report_rx)
            .await
            .expect("Timed out waiting for Chrome probe report")
            .unwrap();

        let _ = stop_tx.send(true);
        let _ = child.kill();
        let _ = child.wait();
        let _ = fs::remove_dir_all(&tmp_dir);

        let parsed_report: serde_json::Value = serde_json::from_str(&report_json).unwrap();
        println!("CHROME PROBE REPORT: {parsed_report:#}");
        let errs = parsed_report["errors"].as_array().unwrap();
        assert!(
            errs.is_empty(),
            "Mask detection probe failed with errors: {parsed_report:#}"
        );
    }

    #[tokio::test]
    async fn test_live_chrome_cookie_injection_and_export() {
        use std::process::{Command, Stdio};

        let Some(chrome_bin) = browser::find_chrome_binary() else {
            return;
        };

        let tmp_dir = std::env::temp_dir().join(format!("anty_cookie_test_{}", rand::random::<u32>()));
        let safe_dir = browser::get_short_or_safe_path(&tmp_dir);
        let cdp_port = browser::pick_ephemeral_port().unwrap();

        let mut child = Command::new(&chrome_bin)
            .args([
                &format!("--user-data-dir={safe_dir}"),
                &format!("--remote-debugging-port={cdp_port}"),
                "--headless=new",
                "--disable-gpu",
                "--no-first-run",
                "about:blank",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();

        let ws_url = browser::wait_for_cdp_ws_url(cdp_port, &mut child).await.unwrap();

        let sample_cookie = cookies::Cookie {
            name: "auth_session".into(),
            value: "super_secret_payload_12345".into(),
            domain: ".example.com".into(),
            path: "/".into(),
            expires: Some(1893456000.0),
            http_only: true,
            secure: true,
            same_site: Some("Lax".into()),
        };

        browser::set_cookies_via_cdp(&ws_url, &[sample_cookie.clone()]).await.unwrap();
        let exported = browser::export_cookies_from_cdp(&ws_url).await.unwrap();

        let _ = child.kill();
        let _ = child.wait();
        let _ = fs::remove_dir_all(&tmp_dir);

        let found = exported.iter().find(|c| c.name == "auth_session");
        assert!(found.is_some(), "Injected cookie should be retrieved via CDP export");
        let found_cookie = found.unwrap();
        assert_eq!(found_cookie.value, "super_secret_payload_12345");
        assert_eq!(found_cookie.domain, ".example.com");
    }

    #[test]
    fn test_proxy_config_password_not_in_display_short() {
        let cfg = ProxyConfig {
            scheme: "socks5".into(),
            host: "1.2.3.4".into(),
            port: 1080,
            user: Some("alice".into()),
            password: Some("topsecret".into()),
        };
        let short = cfg.display_short();
        assert!(!short.contains("topsecret"));
        assert!(short.contains("[auth]"));
    }
}
