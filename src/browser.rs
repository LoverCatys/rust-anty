use std::collections::{HashMap, HashSet};
use std::fs;
use std::net::TcpListener as StdTcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::watch;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::cookies::Cookie;
use crate::fingerprint::build_stealth_script;
use crate::models::{
    locale_for_timezone, normalize_timezone, session_dir_for_profile, Profile, ProxyConfig,
};
use crate::proxy::LocalProxyBridge;

pub struct RunningSession {
    pub profile_name: String,
    pub child: Child,
    pub bridge: Option<LocalProxyBridge>,
    pub cdp_stop: watch::Sender<bool>,
    pub started_at: Instant,
    pub proxy_label: String,
    pub cdp_port: u16,
}

#[derive(Clone, Default)]
pub struct SessionManager {
    inner: Arc<Mutex<HashMap<String, RunningSession>>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn refresh(&self) {
        if let Ok(mut map) = self.inner.lock() {
            let mut finished = Vec::new();
            for (name, sess) in map.iter_mut() {
                if let Ok(Some(_status)) = sess.child.try_wait() {
                    finished.push(name.clone());
                }
            }
            for name in finished {
                if let Some(sess) = map.remove(&name) {
                    let _ = sess.cdp_stop.send(true);
                    if let Some(b) = sess.bridge {
                        b.stop();
                    }
                }
            }
        }
    }

    pub fn is_running(&self, profile_name: &str) -> bool {
        self.refresh();
        self.inner
            .lock()
            .map(|m| m.contains_key(profile_name))
            .unwrap_or(false)
    }

    pub fn get_cdp_port(&self, profile_name: &str) -> Option<u16> {
        self.refresh();
        self.inner
            .lock()
            .ok()
            .and_then(|m| m.get(profile_name).map(|s| s.cdp_port))
    }

    pub fn active_count(&self) -> usize {
        self.refresh();
        self.inner.lock().map(|m| m.len()).unwrap_or(0)
    }

    pub fn list_active(&self) -> Vec<(String, String, u64)> {
        self.refresh();
        let Ok(map) = self.inner.lock() else {
            return Vec::new();
        };
        let mut out: Vec<(String, String, u64)> = map
            .values()
            .map(|s| {
                (
                    s.profile_name.clone(),
                    s.proxy_label.clone(),
                    s.started_at.elapsed().as_secs(),
                )
            })
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    pub fn insert(&self, session: RunningSession) {
        if let Ok(mut map) = self.inner.lock() {
            map.insert(session.profile_name.clone(), session);
        }
    }

    pub fn stop_session(&self, profile_name: &str) -> bool {
        let removed = self
            .inner
            .lock()
            .ok()
            .and_then(|mut m| m.remove(profile_name));
        if let Some(mut sess) = removed {
            let _ = sess.cdp_stop.send(true);
            if let Some(b) = sess.bridge {
                b.stop();
            }
            let _ = sess.child.kill();
            let _ = sess.child.wait();
            true
        } else {
            false
        }
    }

    pub fn stop_all(&self) {
        let items: Vec<RunningSession> = if let Ok(mut map) = self.inner.lock() {
            map.drain().map(|(_, v)| v).collect()
        } else {
            Vec::new()
        };
        for mut sess in items {
            let _ = sess.cdp_stop.send(true);
            if let Some(b) = sess.bridge {
                b.stop();
            }
            let _ = sess.child.kill();
            let _ = sess.child.wait();
        }
    }
}

pub fn find_chrome_binary() -> Option<PathBuf> {
    if let Ok(custom) = std::env::var("CHROME_PATH") {
        let p = PathBuf::from(custom);
        if p.is_file() {
            return Some(p);
        }
    }

    let mut candidates = Vec::new();
    if let Ok(pf) = std::env::var("ProgramFiles") {
        candidates.push(PathBuf::from(&pf).join(r"Google\Chrome\Application\chrome.exe"));
    }
    if let Ok(pf86) = std::env::var("ProgramFiles(x86)") {
        candidates.push(PathBuf::from(&pf86).join(r"Google\Chrome\Application\chrome.exe"));
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        candidates.push(PathBuf::from(&local).join(r"Google\Chrome\Application\chrome.exe"));
        candidates.push(PathBuf::from(&local).join(r"Chromium\Application\chrome.exe"));
    }

    #[cfg(not(windows))]
    {
        candidates.push(PathBuf::from("/usr/bin/google-chrome"));
        candidates.push(PathBuf::from("/usr/bin/chromium"));
        candidates.push(PathBuf::from(
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        ));
    }

    candidates.into_iter().find(|p| p.is_file())
}

#[cfg(windows)]
pub fn get_short_or_safe_path(path: &Path) -> String {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows_sys::Win32::Storage::FileSystem::GetShortPathNameW;

    let _ = fs::create_dir_all(path);
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut buf = vec![0u16; 1024];
    let len = unsafe { GetShortPathNameW(wide.as_ptr(), buf.as_mut_ptr(), buf.len() as u32) };
    if len > 0 && (len as usize) < buf.len() {
        let os_str = std::ffi::OsString::from_wide(&buf[..len as usize]);
        if let Some(s) = os_str.to_str() {
            if !s.is_empty() {
                return s.to_string();
            }
        }
    }
    path.to_string_lossy().to_string()
}

#[cfg(not(windows))]
pub fn get_short_or_safe_path(path: &Path) -> String {
    let _ = fs::create_dir_all(path);
    path.to_string_lossy().to_string()
}

pub fn free_profile_locks(session_dir: &Path, safe_dir_arg: &str) {
    #[cfg(windows)]
    {
        let ps_cmd = "$t1 = $env:RANTY_DIR1; $t2 = $env:RANTY_DIR2; \
            Get-CimInstance Win32_Process -Filter \"name = 'chrome.exe'\" | \
            Where-Object { $_.CommandLine -and (($t1 -and $_.CommandLine.Contains($t1)) -or ($t2 -and $_.CommandLine.Contains($t2))) } | \
            ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }";

        let _ = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", ps_cmd])
            .env("RANTY_DIR1", session_dir.to_string_lossy().as_ref())
            .env("RANTY_DIR2", safe_dir_arg)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    for lock_name in [
        "SingletonLock",
        "SingletonSocket",
        "SingletonCookie",
        "LOCK",
        "DevToolsActivePort",
    ] {
        let p = session_dir.join(lock_name);
        if p.exists() || p.symlink_metadata().is_ok() {
            let _ = fs::remove_file(&p);
        }
    }
}

fn prepare_chrome_preferences(
    session_dir: &Path,
    has_proxy: bool,
    primary_locale: &str,
    accept_langs: &str,
) {
    let _ = fs::create_dir_all(session_dir);

    // 1. Set Windows Chrome `Local State` application locale so native UI & `navigator.language` align
    let local_state_path = session_dir.join("Local State");
    let mut local_state: Value = fs::read_to_string(&local_state_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| json!({}));
    if !local_state.is_object() {
        local_state = json!({});
    }
    local_state["intl"]["app_locale"] = json!(primary_locale);
    if let Ok(s) = serde_json::to_string(&local_state) {
        let _ = fs::write(&local_state_path, s);
    }

    // 2. Set profile `Default/Preferences` (`intl.accept_languages`, WebRTC leak policy, clean exit)
    let default_dir = session_dir.join("Default");
    let _ = fs::create_dir_all(&default_dir);
    let pref_path = default_dir.join("Preferences");

    let mut prefs: Value = fs::read_to_string(&pref_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| json!({}));

    if !prefs.is_object() {
        prefs = json!({});
    }

    prefs["intl"]["accept_languages"] = json!(accept_langs);
    prefs["intl"]["selected_languages"] = json!(accept_langs);
    prefs["translate"]["enabled"] = json!(false);
    prefs["safebrowsing"]["enabled"] = json!(false);
    prefs["safebrowsing"]["enhanced"] = json!(false);
    prefs["browser"]["check_default_browser"] = json!(false);

    if has_proxy {
        prefs["webrtc"]["ip_handling_policy"] = json!("disable_non_proxied_udp");
        prefs["webrtc"]["multiple_routes_enabled"] = json!(false);
        prefs["webrtc"]["nonproxied_udp_enabled"] = json!(false);
    } else {
        prefs["webrtc"]["ip_handling_policy"] = json!("default");
        prefs["webrtc"]["multiple_routes_enabled"] = json!(true);
        prefs["webrtc"]["nonproxied_udp_enabled"] = json!(true);
    }

    prefs["profile"]["exit_type"] = json!("Normal");
    prefs["profile"]["exited_cleanly"] = json!(true);

    if let Ok(serialized) = serde_json::to_string(&prefs) {
        let _ = fs::write(&pref_path, serialized);
    }
}

pub fn pick_ephemeral_port() -> Result<u16, String> {
    let listener = StdTcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("Не удалось выделить локальный CDP-порт: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    Ok(port)
}

pub async fn wait_for_cdp_ws_url(cdp_port: u16, child: &mut Child) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_millis(800))
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!("http://127.0.0.1:{cdp_port}/json/version");
    let deadline = Instant::now() + Duration::from_secs(12);

    while Instant::now() < deadline {
        if let Ok(Some(status)) = child.try_wait() {
            return Err(format!(
                "Процесс Chrome завершился преждевременно (код: {status}). Закройте лишние окна профиля."
            ));
        }

        if let Ok(resp) = client.get(&url).send().await {
            if let Ok(val) = resp.json::<Value>().await {
                if let Some(ws) = val.get("webSocketDebuggerUrl").and_then(|v| v.as_str()) {
                    return Ok(ws.to_string());
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(80)).await;
    }

    Err("Таймаут ожидания CDP-интерфейса Chrome.".to_string())
}

/// Connects to Chrome's Browser CDP WebSocket without EVER enabling `Runtime.enable`,
/// applying stealth fingerprint JS, timezone, and locale overrides across ALL:
/// - Top-level pages & popups (`page`)
/// - Cross-origin OOPIF frames (`iframe`)
/// - Dedicated Workers, SharedWorkers, and ServiceWorkers (`worker`, `shared_worker`, `service_worker`)!
pub async fn start_cdp_stealth_guardian(
    ws_url: String,
    stealth_js: String,
    timezone: String,
    primary_locale: String,
    geo_coords: Option<(f64, f64)>,
    pending_cookies: Option<Vec<Cookie>>,
) -> Result<watch::Sender<bool>, String> {
    let (ws_stream, _) = connect_async(&ws_url)
        .await
        .map_err(|e| format!("Ошибка подключения к CDP WebSocket: {e}"))?;

    let (mut write, mut read) = ws_stream.split();
    let msg_id = Arc::new(AtomicU64::new(1));
    let next_id = move || msg_id.fetch_add(1, Ordering::Relaxed);

    // Attach to pages, iframes, shared_workers, and service_workers at browser level
    let init_cmd = json!({
        "id": next_id(),
        "method": "Target.setAutoAttach",
        "params": {
            "autoAttach": true,
            "waitForDebuggerOnStart": true,
            "flatten": true,
            "filter": [
                { "type": "browser", "exclude": true },
                { "type": "tab", "exclude": true },
                {}
            ]
        }
    });
    write
        .send(Message::Text(init_cmd.to_string().into()))
        .await
        .map_err(|e| format!("Ошибка инициализации CDP: {e}"))?;

    // Seed pending cookies at browser storage level if present
    if let Some(ref cookies) = pending_cookies {
        if !cookies.is_empty() {
            let cdp_params: Vec<Value> = cookies.iter().map(|c| c.to_cdp_param()).collect();
            let storage_cookies_cmd = json!({
                "id": next_id(),
                "method": "Storage.setCookies",
                "params": { "cookies": cdp_params }
            });
            let _ = write.send(Message::Text(storage_cookies_cmd.to_string().into())).await;
        }
    }

    let icu_locale = primary_locale.replace('-', "_");
    let (stop_tx, mut stop_rx) = watch::channel(false);
    let mut pending_cookies_page = pending_cookies;

    tokio::spawn(async move {
        let mut reloaded_initial_blank = false;
        let mut initialized_sessions: HashSet<String> = HashSet::new();

        loop {
            tokio::select! {
                changed = stop_rx.changed() => {
                    if changed.is_err() || *stop_rx.borrow() {
                        break;
                    }
                }
                incoming = read.next() => {
                    let Some(Ok(Message::Text(txt))) = incoming else {
                        if incoming.is_none() {
                            break;
                        }
                        continue;
                    };

                    let Ok(parsed) = serde_json::from_str::<Value>(&txt) else {
                        continue;
                    };

                    let method = parsed.get("method").and_then(|v| v.as_str()).unwrap_or("");
                    if method == "Target.detachedFromTarget" {
                        if let Some(sid) = parsed.pointer("/params/sessionId").and_then(|v| v.as_str()) {
                            initialized_sessions.remove(sid);
                        }
                        continue;
                    }

                    if method == "Target.attachedToTarget" {
                        let session_id = parsed
                            .pointer("/params/sessionId")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let target_type = parsed
                            .pointer("/params/targetInfo/type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let target_url = parsed
                            .pointer("/params/targetInfo/url")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let waiting = parsed
                            .pointer("/params/waitingForDebugger")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);

                        if session_id.is_empty() {
                            continue;
                        }

                        if initialized_sessions.insert(session_id.clone()) {
                            if target_type == "page" || target_type == "iframe" {
                                let mut cmds = vec![
                                    json!({
                                        "id": next_id(),
                                        "sessionId": session_id,
                                        "method": "Page.enable",
                                        "params": {}
                                    }),
                                    json!({
                                        "id": next_id(),
                                        "sessionId": session_id,
                                        "method": "Emulation.setTimezoneOverride",
                                        "params": { "timezoneId": timezone }
                                    }),
                                    json!({
                                        "id": next_id(),
                                        "sessionId": session_id,
                                        "method": "Emulation.setLocaleOverride",
                                        "params": { "locale": icu_locale }
                                    }),
                                    json!({
                                        "id": next_id(),
                                        "sessionId": session_id,
                                        "method": "Page.addScriptToEvaluateOnNewDocument",
                                        "params": { "source": stealth_js }
                                    }),
                                    // Immediately patch the already-created Window context when Chromium swaps
                                    // renderer processes (e.g. omnibox navigation or new tab with URL) while paused!
                                    json!({
                                        "id": next_id(),
                                        "sessionId": session_id,
                                        "method": "Runtime.evaluate",
                                        "params": {
                                            "expression": stealth_js,
                                            "silent": true,
                                            "returnByValue": false
                                        }
                                    }),
                                    // Attach to Dedicated Workers & nested OOPIFs spawned inside this frame!
                                    json!({
                                        "id": next_id(),
                                        "sessionId": session_id,
                                        "method": "Target.setAutoAttach",
                                        "params": {
                                            "autoAttach": true,
                                            "waitForDebuggerOnStart": true,
                                            "flatten": true
                                        }
                                    }),
                                ];

                                // Apply pending cookies at page level ONLY ONCE (never overwrite fresh cookies on new tabs!)
                                if target_type == "page" {
                                    if let Some(cookies) = pending_cookies_page.take() {
                                        if !cookies.is_empty() {
                                            let cdp_params: Vec<Value> =
                                                cookies.iter().map(|c| c.to_cdp_param()).collect();
                                            cmds.push(json!({
                                                "id": next_id(),
                                                "sessionId": session_id,
                                                "method": "Network.setCookies",
                                                "params": { "cookies": cdp_params }
                                            }));
                                        }
                                    }
                                }

                                for cmd in cmds {
                                    let _ = write.send(Message::Text(cmd.to_string().into())).await;
                                }

                                if let Some((lat, lon)) = geo_coords {
                                    let geo_cmd = json!({
                                        "id": next_id(),
                                        "sessionId": session_id,
                                        "method": "Emulation.setGeolocationOverride",
                                        "params": {
                                            "latitude": lat,
                                            "longitude": lon,
                                            "accuracy": 45.0
                                        }
                                    });
                                    let _ = write.send(Message::Text(geo_cmd.to_string().into())).await;
                                }

                                if target_type == "page"
                                    && !reloaded_initial_blank
                                    && (target_url == "about:blank" || target_url.is_empty())
                                {
                                    reloaded_initial_blank = true;
                                    let reload_cmd = json!({
                                        "id": next_id(),
                                        "sessionId": session_id,
                                        "method": "Page.reload",
                                        "params": {}
                                    });
                                    let _ = write.send(Message::Text(reload_cmd.to_string().into())).await;
                                }
                            } else if target_type == "worker"
                                || target_type == "shared_worker"
                                || target_type == "service_worker"
                            {
                                // Patch Web Worker before its first line executes!
                                let worker_cmds = [
                                    json!({
                                        "id": next_id(),
                                        "sessionId": session_id,
                                        "method": "Emulation.setTimezoneOverride",
                                        "params": { "timezoneId": timezone }
                                    }),
                                    json!({
                                        "id": next_id(),
                                        "sessionId": session_id,
                                        "method": "Emulation.setLocaleOverride",
                                        "params": { "locale": icu_locale }
                                    }),
                                    json!({
                                        "id": next_id(),
                                        "sessionId": session_id,
                                        "method": "Runtime.evaluate",
                                        "params": {
                                            "expression": stealth_js,
                                            "silent": true,
                                            "returnByValue": false
                                        }
                                    }),
                                ];
                                for cmd in worker_cmds {
                                    let _ = write.send(Message::Text(cmd.to_string().into())).await;
                                }
                            }
                        }

                        if waiting {
                            let resume_cmd = json!({
                                "id": next_id(),
                                "sessionId": session_id,
                                "method": "Runtime.runIfWaitingForDebugger",
                                "params": {}
                            });
                            let _ = write.send(Message::Text(resume_cmd.to_string().into())).await;
                        }
                    }
                }
            }
        }
    });

    tokio::time::sleep(Duration::from_millis(140)).await;
    Ok(stop_tx)
}

pub async fn launch_profile_session(
    profile_name: &str,
    profile: &Profile,
    proxy_override: Option<ProxyConfig>,
    sync_geo_from_proxy: bool,
) -> Result<RunningSession, String> {
    let chrome_bin = find_chrome_binary().ok_or_else(|| {
        "Google Chrome не найден! Установите Chrome или укажите путь в переменной окружения CHROME_PATH.".to_string()
    })?;

    let bridge = if let Some(ref p_cfg) = proxy_override {
        Some(LocalProxyBridge::start(p_cfg.clone()).await?)
    } else {
        None
    };

    let mut tz = normalize_timezone(&profile.timezone);
    let mut geo_coords = profile.latitude.zip(profile.longitude);

    // If proxy is active and caller didn't already sync geo right before launch,
    // probe the exit IP's timezone & coordinates via ipwho.is (fallback ipinfo.io).
    if sync_geo_from_proxy {
        if let Some(ref b) = bridge {
            let proxy_url = format!("http://127.0.0.1:{}", b.port);
            if let Ok(req_proxy) = reqwest::Proxy::all(&proxy_url) {
                if let Ok(client) = reqwest::Client::builder()
                    .proxy(req_proxy)
                    .timeout(Duration::from_millis(2500))
                    .user_agent("Mozilla/5.0")
                    .build()
                {
                    let mut synced = false;
                    if let Ok(resp) = client.get("https://ipwho.is/").send().await {
                        if let Ok(json) = resp.json::<Value>().await {
                            if json.get("success").and_then(|v| v.as_bool()) != Some(false) {
                                if let Some(tz_id) = json
                                    .pointer("/timezone/id")
                                    .and_then(|v| v.as_str())
                                    .filter(|s| !s.trim().is_empty())
                                {
                                    tz = normalize_timezone(tz_id);
                                    if let (Some(la), Some(lo)) = (
                                        json.get("latitude").and_then(|v| v.as_f64()),
                                        json.get("longitude").and_then(|v| v.as_f64()),
                                    ) {
                                        geo_coords = Some((la, lo));
                                    }
                                    synced = true;
                                }
                            }
                        }
                    }
                    if !synced {
                        if let Ok(resp) = client.get("https://ipinfo.io/json").send().await {
                            if let Ok(json) = resp.json::<Value>().await {
                                if let Some(tz_id) = json
                                    .get("timezone")
                                    .and_then(|v| v.as_str())
                                    .filter(|s| !s.trim().is_empty())
                                {
                                    tz = normalize_timezone(tz_id);
                                    if let Some((la, lo)) = json
                                        .get("loc")
                                        .and_then(|v| v.as_str())
                                        .and_then(|s| s.split_once(','))
                                        .and_then(|(a, b)| Some((a.parse::<f64>().ok()?, b.parse::<f64>().ok()?)))
                                    {
                                        geo_coords = Some((la, lo));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let (primary_locale, accept_langs) = locale_for_timezone(&tz);

    let raw_session_dir = session_dir_for_profile(profile_name, profile);
    let safe_dir = get_short_or_safe_path(&raw_session_dir);

    free_profile_locks(&raw_session_dir, &safe_dir);
    prepare_chrome_preferences(
        &raw_session_dir,
        bridge.is_some(),
        primary_locale,
        accept_langs,
    );

    let proxy_label = proxy_override
        .as_ref()
        .map(|p| p.display_short())
        .unwrap_or_else(|| "Прямое соединение".to_string());

    let cdp_port = pick_ephemeral_port()?;
    let screen_w = profile.width.max(800);
    let screen_h = profile.height.max(600);
    // Natural desktop window bounds strictly <= screen dimensions
    let win_w = screen_w.min(1560);
    let win_h = screen_h.saturating_sub(56).min(920).max(600);

    let mut args = vec![
        format!("--user-data-dir={safe_dir}"),
        format!("--remote-debugging-port={cdp_port}"),
        "--no-first-run".to_string(),
        "--no-default-browser-check".to_string(),
        "--disable-features=AutofillServerCommunication,OptimizationHints,Translate,SafeBrowsingExtendedReporting".to_string(),
        "--disable-breakpad".to_string(),
        "--disable-component-update".to_string(),
        "--disable-default-apps".to_string(),
        "--disable-domain-reliability".to_string(),
        "--disable-sync".to_string(),
        "--force-device-scale-factor=1".to_string(),
        format!("--lang={primary_locale}"),
        format!("--accept-lang={accept_langs}"),
        format!("--window-size={win_w},{win_h}"),
    ];

    if let Some(ref b) = bridge {
        args.push("--disable-quic".to_string());
        args.push(format!("--proxy-server=http://127.0.0.1:{}", b.port));
        args.push("--proxy-bypass-list=<-loopback>".to_string());
        args.push("--force-webrtc-ip-handling-policy=disable_non_proxied_udp".to_string());
        args.push("--enforce-webrtc-ip-permission-check".to_string());
    } else {
        args.push("--no-proxy-server".to_string());
    }

    args.push("about:blank".to_string());

    let mut child = Command::new(&chrome_bin)
        .args(&args)
        .env("TZ", &tz)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Не удалось запустить Chrome ({}): {e}", chrome_bin.display()))?;

    let ws_url = match wait_for_cdp_ws_url(cdp_port, &mut child).await {
        Ok(u) => u,
        Err(err) => {
            let _ = child.kill();
            if let Some(b) = bridge {
                b.stop();
            }
            return Err(err);
        }
    };

    let stealth_js = build_stealth_script(profile);
    let pending_cookies = crate::cookies::load_and_consume_pending_cookies(&raw_session_dir);

    let cdp_stop = match start_cdp_stealth_guardian(
        ws_url,
        stealth_js,
        tz,
        primary_locale.to_string(),
        geo_coords,
        pending_cookies,
    )
    .await
    {
            Ok(tx) => tx,
            Err(err) => {
                let _ = child.kill();
                if let Some(b) = bridge {
                    b.stop();
                }
                return Err(err);
            }
        };

    Ok(RunningSession {
        profile_name: profile_name.to_string(),
        child,
        bridge,
        cdp_stop,
        started_at: Instant::now(),
        proxy_label,
        cdp_port,
    })
}

pub async fn export_cookies_from_cdp(ws_url: &str) -> Result<Vec<Cookie>, String> {
    let (ws_stream, _) = connect_async(ws_url)
        .await
        .map_err(|e| format!("Ошибка подключения к CDP WebSocket: {e}"))?;
    let (mut write, mut read) = ws_stream.split();
    let req_id = 9999u64;
    let cmd = json!({
        "id": req_id,
        "method": "Storage.getCookies",
        "params": {}
    });
    write
        .send(Message::Text(cmd.to_string().into()))
        .await
        .map_err(|e| format!("Ошибка отправки команды Storage.getCookies: {e}"))?;

    let deadline = Instant::now() + Duration::from_secs(6);
    while Instant::now() < deadline {
        let msg = tokio::time::timeout(Duration::from_millis(500), read.next()).await;
        if let Ok(Some(Ok(Message::Text(txt)))) = msg {
            if let Ok(val) = serde_json::from_str::<Value>(&txt) {
                if val.get("id").and_then(|v| v.as_u64()) == Some(req_id) {
                    if let Some(list_val) = val.pointer("/result/cookies") {
                        let cookies: Vec<Cookie> =
                            serde_json::from_value(list_val.clone()).unwrap_or_default();
                        let _ = write.close().await;
                        return Ok(cookies);
                    }
                }
            }
        }
    }
    let _ = write.close().await;
    Err("Не удалось получить cookies из CDP (таймаут ответа).".into())
}

pub async fn set_cookies_via_cdp(ws_url: &str, cookies: &[Cookie]) -> Result<(), String> {
    if cookies.is_empty() {
        return Ok(());
    }
    let (ws_stream, _) = connect_async(ws_url)
        .await
        .map_err(|e| format!("Ошибка подключения к CDP WebSocket: {e}"))?;
    let (mut write, mut read) = ws_stream.split();
    let req_id = 9998u64;
    let cdp_params: Vec<Value> = cookies.iter().map(|c| c.to_cdp_param()).collect();
    let cmd = json!({
        "id": req_id,
        "method": "Storage.setCookies",
        "params": {
            "cookies": cdp_params
        }
    });
    write
        .send(Message::Text(cmd.to_string().into()))
        .await
        .map_err(|e| format!("Ошибка отправки команды Storage.setCookies: {e}"))?;

    let deadline = Instant::now() + Duration::from_secs(6);
    while Instant::now() < deadline {
        let msg = tokio::time::timeout(Duration::from_millis(500), read.next()).await;
        if let Ok(Some(Ok(Message::Text(txt)))) = msg {
            if let Ok(val) = serde_json::from_str::<Value>(&txt) {
                if val.get("id").and_then(|v| v.as_u64()) == Some(req_id) {
                    let _ = write.close().await;
                    return Ok(());
                }
            }
        }
    }
    let _ = write.close().await;
    Ok(())
}

pub async fn export_profile_cookies(
    profile_name: &str,
    profile: &Profile,
    sessions: &SessionManager,
) -> Result<Vec<Cookie>, String> {
    if let Some(port) = sessions.get_cdp_port(profile_name) {
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .build()
            .map_err(|e| e.to_string())?;
        let url = format!("http://127.0.0.1:{port}/json/version");
        let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
        let val: Value = resp.json().await.map_err(|e| e.to_string())?;
        let ws = val
            .get("webSocketDebuggerUrl")
            .and_then(|v| v.as_str())
            .ok_or("Не найден webSocketDebuggerUrl")?;
        export_cookies_from_cdp(ws).await
    } else {
        let chrome_bin = find_chrome_binary().ok_or_else(|| "Google Chrome не найден.".to_string())?;
        let raw_dir = session_dir_for_profile(profile_name, profile);
        let safe_dir = get_short_or_safe_path(&raw_dir);
        let port = pick_ephemeral_port()?;

        let mut child = Command::new(&chrome_bin)
            .args([
                &format!("--user-data-dir={safe_dir}"),
                &format!("--remote-debugging-port={port}"),
                "--headless=new",
                "--disable-gpu",
                "--no-first-run",
                "about:blank",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Не удалось запустить временный Chrome: {e}"))?;

        let ws_url = match wait_for_cdp_ws_url(port, &mut child).await {
            Ok(u) => u,
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                free_profile_locks(&raw_dir, &safe_dir);
                return Err(e);
            }
        };

        let result = export_cookies_from_cdp(&ws_url).await;
        let _ = child.kill();
        let _ = child.wait();
        free_profile_locks(&raw_dir, &safe_dir);
        result
    }
}

pub async fn import_profile_cookies(
    profile_name: &str,
    profile: &Profile,
    sessions: &SessionManager,
    cookies: &[Cookie],
) -> Result<usize, String> {
    if cookies.is_empty() {
        return Err("Список cookies пуст.".to_string());
    }

    let count = cookies.len();
    let raw_dir = session_dir_for_profile(profile_name, profile);

    // If currently running, inject immediately via live CDP so we don't leave a stale
    // pending_cookies.json that could overwrite newer session cookies on the next launch.
    let mut injected_live = false;
    if let Some(port) = sessions.get_cdp_port(profile_name) {
        if let Ok(client) = reqwest::Client::builder().no_proxy().build() {
            let url = format!("http://127.0.0.1:{port}/json/version");
            if let Ok(resp) = client.get(&url).send().await {
                if let Ok(val) = resp.json::<Value>().await {
                    if let Some(ws) = val.get("webSocketDebuggerUrl").and_then(|v| v.as_str()) {
                        if set_cookies_via_cdp(ws, cookies).await.is_ok() {
                            injected_live = true;
                        }
                    }
                }
            }
        }
    }

    if !injected_live {
        crate::cookies::save_pending_cookies(&raw_dir, cookies)?;
    }

    Ok(count)
}
