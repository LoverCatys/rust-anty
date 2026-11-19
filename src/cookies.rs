use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    #[serde(default = "default_path")]
    pub path: String,
    #[serde(default, rename = "expirationDate", alias = "expires")]
    pub expires: Option<f64>,
    #[serde(default, rename = "httpOnly", alias = "http_only")]
    pub http_only: bool,
    #[serde(default)]
    pub secure: bool,
    #[serde(default, rename = "sameSite", alias = "same_site")]
    pub same_site: Option<String>,
}

fn default_path() -> String {
    "/".to_string()
}

impl Cookie {
    /// Converts this cookie to a CDP `Network.CookieParam` JSON Value for `Network.setCookies`.
    pub fn to_cdp_param(&self) -> Value {
        let mut obj = serde_json::json!({
            "name": self.name,
            "value": self.value,
            "domain": self.domain,
            "path": if self.path.is_empty() { "/" } else { &self.path },
            "secure": self.secure,
            "httpOnly": self.http_only,
        });

        if let Some(exp) = self.expires {
            if exp > 0.0 {
                obj["expires"] = serde_json::json!(exp);
            }
        }

        if let Some(ref ss) = self.same_site {
            let s_norm = match ss.to_ascii_lowercase().as_str() {
                "strict" => "Strict",
                "lax" => "Lax",
                "none" | "no_restriction" => "None",
                _ => "",
            };
            if !s_norm.is_empty() {
                obj["sameSite"] = serde_json::json!(s_norm);
            }
        }

        obj
    }
}

/// Parses a cookie file content or raw string in either JSON or Netscape format.
pub fn parse_cookies(input: &str) -> Result<Vec<Cookie>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("Входная строка пуста.".to_string());
    }

    // Try parsing as JSON first
    if trimmed.starts_with('[') || trimmed.starts_with('{') {
        if trimmed.starts_with('[') {
            if let Ok(cookies) = serde_json::from_str::<Vec<Cookie>>(trimmed) {
                if !cookies.is_empty() {
                    return Ok(cookies);
                }
            }
        } else if let Ok(single) = serde_json::from_str::<Cookie>(trimmed) {
            return Ok(vec![single]);
        }
    }

    // Otherwise, parse as Netscape HTTP Cookie File format
    let mut cookies = Vec::new();
    for line in trimmed.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with("# HttpOnly_") == false && l.starts_with('#') {
            continue;
        }

        let is_http_only = l.starts_with("# HttpOnly_");
        let content_line = if is_http_only {
            l.trim_start_matches("# HttpOnly_").trim()
        } else {
            l
        };

        let parts: Vec<&str> = content_line.split('\t').collect();
        if parts.len() >= 7 {
            let domain = parts[0].trim().to_string();
            // parts[1] is include_subdomains (TRUE/FALSE)
            let path = parts[2].trim().to_string();
            let secure = parts[3].trim().eq_ignore_ascii_case("true");
            let expires = parts[4].trim().parse::<f64>().ok().filter(|&x| x > 0.0);
            let name = parts[5].trim().to_string();
            let value = parts[6..].join("\t"); // value may contain tabs in edge cases

            if !name.is_empty() {
                cookies.push(Cookie {
                    name,
                    value,
                    domain,
                    path: if path.is_empty() { "/".into() } else { path },
                    expires,
                    http_only: is_http_only,
                    secure,
                    same_site: None,
                });
            }
        } else {
            // Also accept space-separated if tabs were converted to spaces
            let space_parts: Vec<&str> = content_line.split_whitespace().collect();
            if space_parts.len() >= 7 {
                let domain = space_parts[0].to_string();
                let path = space_parts[2].to_string();
                let secure = space_parts[3].eq_ignore_ascii_case("true");
                let expires = space_parts[4].parse::<f64>().ok().filter(|&x| x > 0.0);
                let name = space_parts[5].to_string();
                let value = space_parts[6..].join(" ");

                if !name.is_empty() {
                    cookies.push(Cookie {
                        name,
                        value,
                        domain,
                        path: if path.is_empty() { "/".into() } else { path },
                        expires,
                        http_only: is_http_only,
                        secure,
                        same_site: None,
                    });
                }
            }
        }
    }

    if cookies.is_empty() {
        Err("Не удалось распознать формат куки. Поддерживаются JSON (массив объектов) и Netscape (tab-separated txt).".into())
    } else {
        Ok(cookies)
    }
}

/// Converts a slice of cookies to pretty-printed JSON.
pub fn export_to_json(cookies: &[Cookie]) -> String {
    serde_json::to_string_pretty(cookies).unwrap_or_else(|_| "[]".to_string())
}

/// Converts a slice of cookies to Netscape HTTP Cookie File format.
pub fn export_to_netscape(cookies: &[Cookie]) -> String {
    let mut out = String::from("# Netscape HTTP Cookie File\n# Generated by RUST ANTY\n\n");
    for c in cookies {
        let prefix = if c.http_only { "# HttpOnly_" } else { "" };
        let subdomains = if c.domain.starts_with('.') { "TRUE" } else { "FALSE" };
        let secure_str = if c.secure { "TRUE" } else { "FALSE" };
        let exp = c.expires.unwrap_or(0.0) as i64;
        let path = if c.path.is_empty() { "/" } else { &c.path };
        out.push_str(&format!(
            "{}{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            prefix, c.domain, subdomains, path, secure_str, exp, c.name, c.value
        ));
    }
    out
}

pub const PENDING_COOKIES_FILE: &str = "pending_cookies.json";

/// Saves imported cookies into the profile session directory so they are loaded on launch.
pub fn save_pending_cookies(session_dir: &Path, cookies: &[Cookie]) -> Result<(), String> {
    let _ = fs::create_dir_all(session_dir);
    let path = session_dir.join(PENDING_COOKIES_FILE);
    let json = export_to_json(cookies);
    fs::write(&path, json).map_err(|e| format!("Не удалось сохранить cookies: {e}"))
}

/// Loads pending cookies if any, removing the file after reading so it isn't repeatedly reapplied.
pub fn load_and_consume_pending_cookies(session_dir: &Path) -> Option<Vec<Cookie>> {
    let path = session_dir.join(PENDING_COOKIES_FILE);
    if !path.is_file() {
        return None;
    }
    let content = fs::read_to_string(&path).ok()?;
    let _ = fs::remove_file(&path);
    parse_cookies(&content).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_cookies() {
        let json_data = r#"[
            {
                "name": "session_id",
                "value": "abc123xyz",
                "domain": ".example.com",
                "path": "/",
                "expirationDate": 1893456000.0,
                "httpOnly": true,
                "secure": true,
                "sameSite": "Lax"
            }
        ]"#;

        let parsed = parse_cookies(json_data).expect("Should parse JSON cookies");
        assert_eq!(parsed.len(), 1);
        let c = &parsed[0];
        assert_eq!(c.name, "session_id");
        assert_eq!(c.value, "abc123xyz");
        assert_eq!(c.domain, ".example.com");
        assert!(c.http_only);
        assert!(c.secure);
        assert_eq!(c.same_site.as_deref(), Some("Lax"));

        let cdp = c.to_cdp_param();
        assert_eq!(cdp["name"], "session_id");
        assert_eq!(cdp["sameSite"], "Lax");
    }

    #[test]
    fn test_parse_netscape_cookies() {
        let netscape_data = "# Netscape HTTP Cookie File\n\
            # HttpOnly_.example.com\tTRUE\t/\tTRUE\t1893456000\tsession_id\tabc123xyz\n\
            example.com\tFALSE\t/app\tFALSE\t0\tuser_pref\tdark_mode\n";

        let parsed = parse_cookies(netscape_data).expect("Should parse Netscape cookies");
        assert_eq!(parsed.len(), 2);

        let c1 = &parsed[0];
        assert_eq!(c1.name, "session_id");
        assert_eq!(c1.value, "abc123xyz");
        assert_eq!(c1.domain, ".example.com");
        assert!(c1.http_only);
        assert!(c1.secure);

        let c2 = &parsed[1];
        assert_eq!(c2.name, "user_pref");
        assert_eq!(c2.value, "dark_mode");
        assert_eq!(c2.domain, "example.com");
        assert_eq!(c2.path, "/app");
        assert!(!c2.http_only);
        assert!(!c2.secure);
    }

    #[test]
    fn test_export_roundtrip() {
        let cookies = vec![Cookie {
            name: "auth_token".to_string(),
            value: "secret_val".to_string(),
            domain: ".site.org".to_string(),
            path: "/".to_string(),
            expires: Some(1794937500.0),
            http_only: true,
            secure: true,
            same_site: Some("Strict".to_string()),
        }];

        let json_str = export_to_json(&cookies);
        let parsed_json = parse_cookies(&json_str).unwrap();
        assert_eq!(parsed_json, cookies);

        let netscape_str = export_to_netscape(&cookies);
        let parsed_netscape = parse_cookies(&netscape_str).unwrap();
        assert_eq!(parsed_netscape[0].name, cookies[0].name);
        assert_eq!(parsed_netscape[0].value, cookies[0].value);
        assert_eq!(parsed_netscape[0].domain, cookies[0].domain);
        assert_eq!(parsed_netscape[0].http_only, cookies[0].http_only);
        assert_eq!(parsed_netscape[0].secure, cookies[0].secure);
    }
}
