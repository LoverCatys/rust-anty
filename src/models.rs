use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use rand::Rng;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::hardware::{FONT_CATALOG, GPU_POOL};
use crate::proxy::parse_proxy;
use crate::secrets::{decrypt_secret, encrypt_secret, is_encrypted};

pub const DB_FILE: &str = "profiliki.json";
pub const SESSIONS_DIR: &str = "sessions";

fn serialize_secret_opt<S>(val: &Option<String>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match val {
        Some(s) if !s.is_empty() => serializer.serialize_some(&encrypt_secret(s)),
        other => other.serialize(serializer),
    }
}

fn deserialize_secret_opt<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    Ok(opt.map(|s| decrypt_secret(&s)))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProxyConfig {
    #[serde(default = "default_scheme")]
    pub scheme: String,
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(
        rename = "pass",
        default,
        serialize_with = "serialize_secret_opt",
        deserialize_with = "deserialize_secret_opt"
    )]
    pub password: Option<String>,
}

fn default_scheme() -> String {
    "http".to_string()
}

impl ProxyConfig {
    pub fn display_short(&self) -> String {
        let auth_badge = if self.user.is_some() { " [auth]" } else { "" };
        format!("{}://{}:{}{}", self.scheme, self.host, self.port, auth_badge)
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawProxyField {
    Map(ProxyConfig),
    Text(String),
}

fn deserialize_proxy_opt<'de, D>(deserializer: D) -> Result<Option<ProxyConfig>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<RawProxyField>::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(RawProxyField::Map(cfg)) => Ok(Some(cfg)),
        Some(RawProxyField::Text(raw)) => {
            if raw.trim().is_empty() {
                Ok(None)
            } else {
                Ok(parse_proxy(&raw, "http"))
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    #[serde(default = "default_cpu_name")]
    pub cpu_name: String,
    #[serde(default = "default_cpu_threads")]
    pub cpu: u32,
    #[serde(default = "default_ram")]
    pub ram: u32,
    #[serde(default = "default_gpu_name")]
    pub gpu_name: String,
    #[serde(default = "default_gpu_vendor")]
    pub gpu_vendor: String,
    #[serde(default = "default_gpu_renderer")]
    pub gpu_renderer: String,
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_height")]
    pub height: u32,
    #[serde(default = "default_timezone")]
    pub timezone: String,
    #[serde(default = "default_fonts")]
    pub fonts: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_proxy_opt")]
    pub proxy: Option<ProxyConfig>,
    #[serde(default = "generate_seed")]
    pub noise_seed: u64,
    #[serde(default)]
    pub folder_id: Option<String>,
    #[serde(default)]
    pub latitude: Option<f64>,
    #[serde(default)]
    pub longitude: Option<f64>,
}

fn default_cpu_name() -> String {
    "Intel Core i5-12400F".to_string()
}
fn default_cpu_threads() -> u32 {
    12
}
fn default_ram() -> u32 {
    16
}
fn default_gpu_name() -> String {
    "NVIDIA GeForce RTX 3060".to_string()
}
fn default_gpu_vendor() -> String {
    "Google Inc. (NVIDIA)".to_string()
}
fn default_gpu_renderer() -> String {
    "ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 (0x00002504) Direct3D11 vs_5_0 ps_5_0, D3D11)"
        .to_string()
}
fn default_width() -> u32 {
    1920
}
fn default_height() -> u32 {
    1080
}
fn default_timezone() -> String {
    "Europe/Kyiv".to_string()
}
fn default_fonts() -> Vec<String> {
    FONT_CATALOG.iter().take(15).map(|s| (*s).to_string()).collect()
}
pub fn generate_seed() -> u64 {
    let mut rng = rand::thread_rng();
    loop {
        let val: u64 = rng.gen_range(100_000..=u32::MAX as u64);
        if val != 0 {
            return val;
        }
    }
}

pub fn normalize_timezone(tz: &str) -> String {
    let trimmed = tz.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("Europe/Kiev") {
        "Europe/Kyiv".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Maps a timezone ID to a geo-consistent `(primary_locale, accept_languages)` pair
/// so Chrome's native `--lang`, `Accept-Language` HTTP header, `Intl`, and `navigator.languages`
/// match the proxy geolocation without needing detectable JS overrides on `Navigator.prototype`.
pub fn locale_for_timezone(tz: &str) -> (&'static str, &'static str) {
    let t = tz.trim();
    match t {
        "Europe/Dublin" | "Europe/London" => ("en-GB", "en-GB,en-US,en"),
        "Europe/Berlin" | "Europe/Vienna" | "Europe/Zurich" => ("de-DE", "de-DE,de,en-US,en"),
        "Europe/Paris" | "Europe/Brussels" | "Europe/Luxembourg" => ("fr-FR", "fr-FR,fr,en-US,en"),
        "Europe/Warsaw" => ("pl-PL", "pl-PL,pl,en-US,en"),
        "Europe/Amsterdam" => ("nl-NL", "nl-NL,nl,en-US,en"),
        "Europe/Madrid" => ("es-ES", "es-ES,es,en-US,en"),
        "Europe/Rome" => ("it-IT", "it-IT,it,en-US,en"),
        "Europe/Lisbon" => ("pt-PT", "pt-PT,pt,en-US,en"),
        "America/Sao_Paulo" => ("pt-BR", "pt-BR,pt,en-US,en"),
        "Europe/Prague" => ("cs-CZ", "cs-CZ,cs,en-US,en"),
        "Europe/Stockholm" => ("sv-SE", "sv-SE,sv,en-US,en"),
        "Europe/Helsinki" => ("fi-FI", "fi-FI,fi,en-US,en"),
        "Europe/Oslo" => ("nb-NO", "nb-NO,nb,no,en-US,en"),
        "Europe/Copenhagen" => ("da-DK", "da-DK,da,en-US,en"),
        "Europe/Bucharest" => ("ro-RO", "ro-RO,ro,en-US,en"),
        "Europe/Budapest" => ("hu-HU", "hu-HU,hu,en-US,en"),
        "Europe/Athens" => ("el-GR", "el-GR,el,en-US,en"),
        "Europe/Istanbul" => ("tr-TR", "tr-TR,tr,en-US,en"),
        "Europe/Kyiv" | "Europe/Uzhgorod" | "Europe/Zaporozhye" => {
            ("uk-UA", "uk-UA,uk,ru-RU,ru,en-US,en")
        }
        "Europe/Moscow"
        | "Europe/Minsk"
        | "Europe/Samara"
        | "Europe/Kaliningrad"
        | "Asia/Yekaterinburg"
        | "Asia/Novosibirsk"
        | "Asia/Krasnoyarsk"
        | "Asia/Vladivostok" => ("ru-RU", "ru-RU,ru,en-US,en"),
        "Asia/Almaty" | "Asia/Aqtobe" => ("ru-RU", "ru-RU,ru,kk-KZ,kk,en-US,en"),
        "Asia/Tokyo" => ("ja-JP", "ja-JP,ja,en-US,en"),
        "Asia/Seoul" => ("ko-KR", "ko-KR,ko,en-US,en"),
        _ if t.starts_with("Australia/") => ("en-AU", "en-AU,en-GB,en-US,en"),
        _ if t.starts_with("America/Toronto")
            || t.starts_with("America/Vancouver")
            || t.starts_with("America/Edmonton") =>
        {
            ("en-CA", "en-CA,en-US,en")
        }
        _ if t.starts_with("America/Mexico")
            || t.starts_with("America/Bogota")
            || t.starts_with("America/Buenos_Aires")
            || t.starts_with("America/Santiago")
            || t.starts_with("America/Lima") =>
        {
            ("es-419", "es-419,es,en-US,en")
        }
        _ => ("en-US", "en-US,en"),
    }
}

pub type ProfileDb = BTreeMap<String, Profile>;

pub fn load_db() -> ProfileDb {
    let path = Path::new(DB_FILE);
    if !path.exists() {
        let empty = ProfileDb::new();
        let _ = save_db(&empty);
        return empty;
    }

    let raw = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return ProfileDb::new(),
    };

    let mut db: ProfileDb = serde_json::from_str(&raw).unwrap_or_default();
    let mut dirty = false;
    // Auto-migrate legacy plaintext proxy passwords to DPAPI ciphertext on load
    if let Ok(raw_val) = serde_json::from_str::<serde_json::Value>(&raw) {
        if let Some(obj) = raw_val.as_object() {
            for (_k, v) in obj {
                if let Some(pass) = v.pointer("/proxy/pass").and_then(|p| p.as_str()) {
                    if !pass.is_empty() && !is_encrypted(pass) {
                        dirty = true;
                    }
                }
            }
        }
    }
    for (_name, prof) in db.iter_mut() {
        if prof.noise_seed == 0 {
            prof.noise_seed = generate_seed();
            dirty = true;
        }
        let norm_tz = normalize_timezone(&prof.timezone);
        if norm_tz != prof.timezone {
            prof.timezone = norm_tz;
            dirty = true;
        }
        // Auto-upgrade legacy ANGLE renderer strings without modern `(0x0000XXXX)` PCI Device ID
        if !prof.gpu_renderer.contains("(0x") {
            if let Some(spec) = GPU_POOL.iter().find(|g| g.name == prof.gpu_name) {
                prof.gpu_renderer = spec.renderer.to_string();
                dirty = true;
            }
        }
    }
    if dirty {
        let _ = save_db(&db);
    }
    db
}

pub fn save_db(db: &ProfileDb) -> Result<(), String> {
    let json = serde_json::to_string_pretty(db).map_err(|e| e.to_string())?;
    let tmp_path = format!("{DB_FILE}.tmp");
    fs::write(&tmp_path, json).map_err(|e| e.to_string())?;
    fs::rename(&tmp_path, DB_FILE).map_err(|e| e.to_string())?;
    Ok(())
}

/// Validates user-supplied profile name so it cannot break Windows paths or traverse directories.
pub fn validate_profile_name(name: &str) -> Result<(), &'static str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Название профиля не может быть пустым.");
    }
    if trimmed.len() > 64 {
        return Err("Название слишком длинное (максимум 64 символа).");
    }
    if trimmed == "." || trimmed == ".." || trimmed.ends_with('.') {
        return Err("Недопустимое системное имя папки.");
    }
    let forbidden = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    if trimmed.chars().any(|c| forbidden.contains(&c) || c.is_control()) {
        return Err("Имя содержит запрещённые символы (< > : \" / \\ | ? *).");
    }
    let upper = trimmed.to_ascii_uppercase();
    let reserved = [
        "CON", "PRN", "AUX", "NUL",
        "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
        "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if reserved.contains(&upper.as_str()) {
        return Err("Это зарезервированное имя устройства Windows.");
    }
    Ok(())
}

pub fn sanitize_folder_component(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.trim().chars() {
        if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || c.is_control() {
            out.push('_');
        } else {
            out.push(c);
        }
    }
    let trimmed = out.trim_end_matches(['.', ' ']);
    if trimmed.is_empty() {
        "profile".to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn session_dir_for_profile(profile_name: &str, profile: &Profile) -> PathBuf {
    let folder = profile
        .folder_id
        .as_deref()
        .map(sanitize_folder_component)
        .unwrap_or_else(|| sanitize_folder_component(profile_name));
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(SESSIONS_DIR)
        .join(folder)
}

#[derive(Debug, Clone)]
pub struct ProxyCheckInfo {
    pub ip: String,
    pub country: String,
    pub city: String,
    pub timezone: Option<String>,
    pub isp: String,
    pub asn: String,
    pub proxy_type: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}
