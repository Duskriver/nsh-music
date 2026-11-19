//! NetEase Cloud Music HTTP API client, EAPI requests, session storage, and QR code login.

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::crypto::eapi_encrypt;
use crate::music::model::{
    Collection, ErrorCode, EngineError, Header, Home, Item, Library, LyricLine, Lyrics, Me,
    SearchResults, Section, Track,
};

const SITE: &str = "https://music.163.com";
const EAPI: &str = "https://interfacepc.music.163.com";
const UA: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15";
const EAPI_UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Safari/537.36 Chrome/91.0.4472.164 NeteaseMusicDesktop/3.1.17.204416";
// Keep cookie names such as MUSIC_U and __csrf intact, as encodeURIComponent does.
const EAPI_COOKIE_ENCODING: &percent_encoding::AsciiSet = &percent_encoding::NON_ALPHANUMERIC
    .remove(b'-').remove(b'_').remove(b'.').remove(b'!').remove(b'~')
    .remove(b'*').remove(b'\'').remove(b'(').remove(b')');
const COOKIE_NAMES: [&str; 3] = ["MUSIC_U", "__csrf", "NMTID"];
const PROFILE_TTL: Duration = Duration::from_secs(60);
const LIKED_TTL: Duration = Duration::from_secs(60);
const COLLECTION_PLAYBACK_TTL: Duration = Duration::from_secs(60);
const STREAM_TTL: Duration = Duration::from_secs(600);
const PLAYLIST_PREVIEW_LIMIT: usize = 1000;
const SONG_DETAIL_BATCH: usize = 400;
const SONG_DETAIL_CONCURRENCY: usize = 4;

/// A stored NetEase user profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetEaseProfile {
    /// NetEase numeric user ID as string.
    pub id: String,
    /// User display name.
    pub name: String,
    /// User avatar URL.
    pub avatar_url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct StoredSession {
    cookies: HashMap<String, String>,
    device_id: String,
    profile: Option<NetEaseProfile>,
}

/// Status returned when polling QR code login.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QrPollResult {
    /// QR code expired (800).
    Expired,
    /// Waiting for user to scan (801).
    WaitingScan,
    /// Scanned by phone, waiting for user confirmation (802).
    ScannedWaitingConfirm,
    /// Login succeeded (803).
    Success,
}

struct LikedCache {
    uid: String,
    ids: Vec<String>,
    id_set: HashSet<String>,
    at: Instant,
}

/// The NetEase API client.
pub struct NetEaseClient {
    session_file: PathBuf,
    device_id: String,
    cookies: Mutex<HashMap<String, String>>,
    pending_qr_cookies: Mutex<HashMap<String, String>>,
    profile: Mutex<Option<NetEaseProfile>>,
    profile_checked_at: Mutex<Option<Instant>>,
    agent: ureq::Agent,
    download_agent: ureq::Agent,
    stream_cache: Mutex<HashMap<u64, (String, Instant)>>,
    liked_cache: Mutex<Option<LikedCache>>,
    liked_playlist_id: Mutex<Option<(String, String)>>,
    collection_cache: Mutex<HashMap<String, (Collection, Instant)>>,
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn generate_device_id() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};

    let mut hex = String::with_capacity(64);
    let now = now_millis();
    let pid = std::process::id();
    for i in 0..4u64 {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_u64(now);
        hasher.write_u32(pid);
        hasher.write_u64(i);
        let val = hasher.finish();
        use std::fmt::Write;
        let _ = write!(&mut hex, "{val:016X}");
    }
    hex.truncate(52);
    hex
}

fn value_id_str(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() || trimmed == "0" { None } else { Some(trimmed.to_string()) }
        }
        Value::Number(n) => n
            .as_u64()
            .filter(|&id| id > 0)
            .map(|id| id.to_string())
            .or_else(|| n.as_i64().filter(|&id| id > 0).map(|id| id.to_string())),
        _ => None,
    }
}

fn value_id_u64(v: &Value) -> Option<u64> {
    match v {
        Value::Number(n) => n.as_u64().or_else(|| n.as_i64().and_then(|id| u64::try_from(id).ok())),
        Value::String(s) => s.trim().parse::<u64>().ok(),
        _ => None,
    }
}

fn valid_positive_id(raw: &str) -> Result<String, EngineError> {
    let id = raw.trim();
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) || id.parse::<u64>().ok().is_none_or(|n| n == 0) {
        return Err(EngineError::new(ErrorCode::NotFound, "无效的网易云音乐 ID"));
    }
    Ok(id.to_string())
}

fn is_music_126_image_cdn_host(host: &str) -> bool {
    let Some(rest) = host.strip_prefix('p') else { return false };
    let Some(digits) = rest.strip_suffix(".music.126.net") else { return false };
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

fn is_music_126_audio_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    host == "music.126.net" || host.ends_with(".music.126.net")
}

fn url_host(url: &str) -> Option<&str> {
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"))?;
    let authority = rest.split(['/', '?', '#']).next()?;
    if authority.is_empty() || authority.contains('@') || authority.contains(':') {
        return None;
    }
    Some(authority)
}

fn artwork_url(raw: Option<&str>) -> String {
    let Some(url) = raw else { return String::new() };
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let secure = if trimmed.starts_with("http://") {
        trimmed.replacen("http://", "https://", 1)
    } else {
        trimmed.to_string()
    };
    if !secure.starts_with("https://") {
        return secure;
    }
    let Some(host) = url_host(&secure) else { return secure };
    if !is_music_126_image_cdn_host(host) {
        return secure;
    }

    let (base_and_query, fragment) = match secure.split_once('#') {
        Some((before, frag)) => (before, Some(frag)),
        None => (secure.as_str(), None),
    };
    let (base, query) = match base_and_query.split_once('?') {
        Some((b, q)) => (b, Some(q)),
        None => (base_and_query, None),
    };

    let mut params: Vec<String> = query
        .into_iter()
        .flat_map(|q| q.split('&'))
        .filter(|pair| !pair.is_empty())
        .filter(|pair| {
            let key = pair.split_once('=').map_or(*pair, |(k, _)| k);
            key != "param"
        })
        .map(ToString::to_string)
        .collect();
    params.push("param=300y300".to_string());

    let mut out = format!("{base}?{}", params.join("&"));
    if let Some(frag) = fragment {
        out.push('#');
        out.push_str(frag);
    }
    out
}

fn unix_secs_from_utc(year: i64, month: i64, day: i64, hour: i64, min: i64, sec: i64) -> Option<i64> {
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || !(0..=23).contains(&hour)
        || !(0..=59).contains(&min)
        || !(0..=60).contains(&sec)
    {
        return None;
    }
    let y = year - if month <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    Some(days * 86_400 + hour * 3_600 + min * 60 + sec)
}

fn parse_month(s: &str) -> Option<i64> {
    match s.to_ascii_lowercase().as_str() {
        "jan" => Some(1),
        "feb" => Some(2),
        "mar" => Some(3),
        "apr" => Some(4),
        "may" => Some(5),
        "jun" => Some(6),
        "jul" => Some(7),
        "aug" => Some(8),
        "sep" => Some(9),
        "oct" => Some(10),
        "nov" => Some(11),
        "dec" => Some(12),
        _ => None,
    }
}

fn parse_http_date_unix(raw: &str) -> Option<i64> {
    // Handles IMF-fixdate / RFC 1123 ("Wed, 09 Jun 2027 10:18:14 GMT") and RFC 850 ("Wednesday, 09-Jun-27 10:18:14 GMT").
    let without_weekday = raw.split_once(',').map_or(raw.trim(), |(_, rest)| rest.trim());
    let parts: Vec<&str> = without_weekday.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }
    let (day, month, year, time_str) = if parts[0].contains('-') {
        let date_parts: Vec<&str> = parts[0].split('-').collect();
        if date_parts.len() != 3 {
            return None;
        }
        let d = date_parts[0].parse::<i64>().ok()?;
        let m = parse_month(date_parts[1])?;
        let mut y = date_parts[2].parse::<i64>().ok()?;
        if y < 100 {
            y += if y >= 70 { 1900 } else { 2000 };
        }
        (d, m, y, parts[1])
    } else {
        if parts.len() < 4 {
            return None;
        }
        let d = parts[0].parse::<i64>().ok()?;
        let m = parse_month(parts[1])?;
        let y = parts[2].parse::<i64>().ok()?;
        (d, m, y, parts[3])
    };
    let mut t = time_str.split(':');
    let hour = t.next()?.parse::<i64>().ok()?;
    let min = t.next()?.parse::<i64>().ok()?;
    let sec = t.next()?.parse::<i64>().ok()?;
    unix_secs_from_utc(year, month, day, hour, min, sec)
}

fn apply_set_cookie_header(cookies: &mut HashMap<String, String>, header: &str, now_unix: i64) -> bool {
    let mut segments = header.split(';');
    let Some(pair) = segments.next() else { return false };
    let Some((raw_name, raw_value)) = pair.split_once('=') else { return false };
    let name = raw_name.trim();
    if !COOKIE_NAMES.contains(&name) {
        return false;
    }
    let value = raw_value.trim();

    let mut max_age: Option<i64> = None;
    let mut expires: Option<i64> = None;
    for attr in segments {
        let Some((k, v)) = attr.trim().split_once('=') else { continue };
        if k.trim().eq_ignore_ascii_case("max-age") {
            if let Ok(secs) = v.trim().parse::<i64>() {
                max_age = Some(secs);
            }
        } else if k.trim().eq_ignore_ascii_case("expires") {
            if let Some(ts) = parse_http_date_unix(v.trim()) {
                expires = Some(ts);
            }
        }
    }

    let expired = if let Some(ma) = max_age {
        ma <= 0
    } else if let Some(exp) = expires {
        exp <= now_unix
    } else {
        false
    };

    if value.is_empty() || value == "deleted" || value == "\"\"" || expired {
        cookies.remove(name).is_some()
    } else {
        cookies.insert(name.to_string(), value.to_string()).as_deref() != Some(value)
    }
}

fn apply_cookie_str(cookies: &mut HashMap<String, String>, cookie_str: &str) -> bool {
    let mut changed = false;
    for raw_item in cookie_str.split(|c| c == ';' || c == '\n') {
        let part = raw_item.trim();
        if let Some((k, v)) = part.split_once('=') {
            let k = k.trim();
            let v = v.trim();
            if COOKIE_NAMES.contains(&k) && !v.is_empty() && v != "deleted" && v != "\"\"" {
                if cookies.insert(k.to_string(), v.to_string()).as_deref() != Some(v) {
                    changed = true;
                }
            }
        }
    }
    changed
}

fn profile_from_response(response: &Value) -> Option<NetEaseProfile> {
    let profile = response.get("profile")?;
    let id = profile.get("userId").and_then(value_id_str)?;
    Some(NetEaseProfile {
        id,
        name: profile.get("nickname").and_then(Value::as_str).unwrap_or("NetEase User").to_string(),
        avatar_url: artwork_url(profile.get("avatarUrl").and_then(Value::as_str)),
    })
}

impl NetEaseClient {
    /// Creates a client and loads existing session cookies from disk if present.
    pub fn new(data_dir: &Path) -> Self {
        let session_file = data_dir.join(crate::music::paths::SESSION_FILE);
        let mut device_id = String::new();
        let mut cookies = HashMap::new();
        let mut profile = None;

        if let Ok(bytes) = fs::read(&session_file) {
            if let Ok(stored) = serde_json::from_slice::<StoredSession>(&bytes) {
                cookies = stored.cookies;
                cookies.retain(|k, v| COOKIE_NAMES.contains(&k.as_str()) && !v.is_empty());
                device_id = stored.device_id;
                profile = stored.profile;
            }
        }

        if device_id.is_empty() {
            device_id = generate_device_id();
        }

        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(12)))
            .max_redirects(0)
            .http_status_as_error(false)
            .build()
            .into();

        let download_agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(10)))
            .timeout_per_read(Some(Duration::from_secs(15)))
            .timeout_global(Some(Duration::from_secs(60)))
            .http_status_as_error(false)
            .build()
            .into();

        NetEaseClient {
            session_file,
            device_id,
            cookies: Mutex::new(cookies),
            pending_qr_cookies: Mutex::new(HashMap::new()),
            profile: Mutex::new(profile),
            profile_checked_at: Mutex::new(None),
            agent,
            download_agent,
            stream_cache: Mutex::new(HashMap::new()),
            liked_cache: Mutex::new(None),
            liked_playlist_id: Mutex::new(None),
            collection_cache: Mutex::new(HashMap::new()),
        }
    }

    /// Whether an active login session cookie (`MUSIC_U`) exists.
    pub fn is_signed_in(&self) -> bool {
        let lock = self.cookies.lock().unwrap();
        lock.get("MUSIC_U").is_some_and(|v| !v.is_empty())
    }

    /// The signed-in user's profile, if available.
    pub fn profile(&self) -> Option<NetEaseProfile> {
        if !self.is_signed_in() {
            return None;
        }
        let cached = self.profile.lock().unwrap().clone();
        cached.or_else(|| self.refresh_profile().ok().flatten())
    }

    fn require_profile(&self) -> Result<NetEaseProfile, EngineError> {
        if !self.is_signed_in() {
            return Err(EngineError::new(ErrorCode::SignedOut, "请先扫码登录网易云音乐"));
        }
        let recently_checked = self
            .profile_checked_at
            .lock()
            .unwrap()
            .is_some_and(|at| at.elapsed() < PROFILE_TTL);
        if recently_checked {
            if let Some(cached) = self.profile.lock().unwrap().clone() {
                return Ok(cached);
            }
        }
        self.refresh_profile()?
            .ok_or_else(|| EngineError::new(ErrorCode::SignedOut, "请先扫码登录网易云音乐"))
    }

    /// Clears the login session and removes the stored session file.
    pub fn clear_session(&self) {
        self.cookies.lock().unwrap().clear();
        *self.profile.lock().unwrap() = None;
        *self.profile_checked_at.lock().unwrap() = None;
        self.stream_cache.lock().unwrap().clear();
        *self.liked_cache.lock().unwrap() = None;
        *self.liked_playlist_id.lock().unwrap() = None;
        self.collection_cache.lock().unwrap().clear();
        let _ = fs::remove_file(&self.session_file);
    }

    fn persist_session(&self) {
        let cookies = self.cookies.lock().unwrap().clone();
        if !cookies.get("MUSIC_U").is_some_and(|v| !v.is_empty()) {
            return;
        }
        let profile = self.profile.lock().unwrap().clone();
        let stored = StoredSession {
            cookies,
            device_id: self.device_id.clone(),
            profile,
        };
        if let Ok(bytes) = serde_json::to_vec_pretty(&stored) {
            let saved = self
                .session_file
                .parent()
                .filter(|dir| !dir.as_os_str().is_empty())
                .map_or(Ok(()), fs::create_dir_all)
                .and_then(|()| fs::write(&self.session_file, bytes));
            if let Err(error) = saved {
                log::warn!("could not save the NetEase session: {error}");
            }
        }
    }

    /// Parses and saves raw cookies from a cookie string or header.
    pub fn save_cookie_str(&self, cookie_str: &str) {
        let changed = {
            let mut lock = self.cookies.lock().unwrap();
            apply_cookie_str(&mut lock, cookie_str)
        };
        if changed {
            self.persist_session();
        }
    }

    #[cfg(test)]
    fn accept_response_cookies(&self, headers: &ureq::http::HeaderMap) {
        let _ = self.accept_response_cookies_into(headers, &self.cookies, true);
    }

    fn accept_response_cookies_into(
        &self,
        headers: &ureq::http::HeaderMap,
        target: &Mutex<HashMap<String, String>>,
        persist_on_change: bool,
    ) -> Result<(), EngineError> {
        let now_unix = now_secs() as i64;
        let (was_signed_in, still_signed_in, changed) = {
            let mut lock = target.lock().unwrap();
            let was_signed_in = lock.get("MUSIC_U").is_some_and(|v| !v.is_empty());
            let mut changed = false;
            for value in headers.get_all(ureq::http::header::SET_COOKIE) {
                if let Ok(cookie) = value.to_str() {
                    if apply_set_cookie_header(&mut lock, cookie, now_unix) {
                        changed = true;
                    }
                }
            }
            let still_signed_in = lock.get("MUSIC_U").is_some_and(|v| !v.is_empty());
            (was_signed_in, still_signed_in, changed)
        };

        if persist_on_change {
            if was_signed_in && !still_signed_in {
                self.clear_session();
                return Err(EngineError::new(
                    ErrorCode::SignedOut,
                    "网易云登录状态已失效，请重新登录",
                ));
            }
            if changed {
                self.persist_session();
            }
        }
        Ok(())
    }

    /// Executes an HTTP request against the NetEase Web or EAPI endpoint.
    pub fn request(
        &self,
        path: &str,
        params: &HashMap<&str, Value>,
        is_eapi: bool,
    ) -> Result<Value, EngineError> {
        self.request_with_codes(path, params, is_eapi, &[200], &self.cookies, true)
    }

    fn request_with_codes(
        &self,
        path: &str,
        params: &HashMap<&str, Value>,
        is_eapi: bool,
        allowed_codes: &[i64],
        cookie_jar: &Mutex<HashMap<String, String>>,
        persist_cookies: bool,
    ) -> Result<Value, EngineError> {
        let response = self.request_raw_into(path, params, is_eapi, cookie_jar, persist_cookies)?;
        let code = response.get("code").and_then(Value::as_i64).unwrap_or(200);
        if allowed_codes.contains(&code) {
            return Ok(response);
        }
        if matches!(code, 301 | 302 | 401) {
            if persist_cookies && self.is_signed_in() {
                // Content can be restricted even while the account is signed in.
                match self.refresh_profile() {
                    Ok(Some(_)) => {
                        log::warn!("NetEase {path} returned code {code}; keeping the login session");
                        return Err(EngineError::new(ErrorCode::Internal, "当前账号无权访问该内容"));
                    }
                    Ok(None) => {
                        log::warn!(
                            "NetEase cleared the login session after {path} returned code {code} and the account check confirmed sign-out"
                        );
                    }
                    Err(err) => {
                        log::warn!("NetEase {path} returned code {code} and account verification failed: {err}");
                        return Err(err);
                    }
                }
            }
            return Err(EngineError::new(ErrorCode::SignedOut, "网易云登录已失效，请重新登录"));
        }
        if code == 460 || code == -460 {
            log::warn!("NetEase {path} returned risk-control code {code}");
            return Err(EngineError::new(ErrorCode::Internal, format!("460:{code}")));
        }
        if code == 405 {
            return Err(EngineError::new(ErrorCode::Internal, "网易云暂时限制了请求，请稍后再试"));
        }
        let msg = response
            .get("message")
            .or_else(|| response.get("msg"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let detail = match msg {
            Some(m) => format!("网易云接口返回错误（{code}）：{m}"),
            None => format!("网易云接口返回错误（{code}）"),
        };
        Err(EngineError::new(ErrorCode::Internal, detail))
    }

    fn request_raw_into(
        &self,
        path: &str,
        params: &HashMap<&str, Value>,
        is_eapi: bool,
        cookie_jar: &Mutex<HashMap<String, String>>,
        persist_cookies: bool,
    ) -> Result<Value, EngineError> {
        let cookies_snapshot = cookie_jar.lock().unwrap().clone();

        let resp = if is_eapi {
            let now_m = now_millis();
            let now_s = now_secs();
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos();
            let rand_val = (nanos / 1000) ^ ((now_m as u32).wrapping_mul(17));
            let mut header_obj = serde_json::Map::new();
            header_obj.insert("osver".into(), json!("Microsoft-Windows-10-Professional-build-19045-64bit"));
            header_obj.insert("deviceId".into(), json!(self.device_id));
            header_obj.insert("os".into(), json!("pc"));
            header_obj.insert("appver".into(), json!("3.1.17.204416"));
            header_obj.insert("versioncode".into(), json!("140"));
            header_obj.insert("mobilename".into(), json!(""));
            header_obj.insert("buildver".into(), json!(now_s.to_string()));
            header_obj.insert("resolution".into(), json!("1920x1080"));
            header_obj.insert("channel".into(), json!("netease"));
            header_obj.insert("requestId".into(), json!(format!("{}_{:04}", now_m, rand_val % 10000)));
            header_obj.insert("__csrf".into(), json!(""));
            for name in COOKIE_NAMES {
                if let Some(val) = cookies_snapshot.get(name) {
                    if !val.is_empty() {
                        header_obj.insert(name.into(), json!(val));
                    }
                }
            }

            let mut merged_params = serde_json::Map::new();
            for (k, v) in params {
                merged_params.insert((*k).to_string(), (*v).clone());
            }
            merged_params.insert("header".into(), Value::Object(header_obj.clone()));
            merged_params.insert("e_r".into(), json!(false));

            let text = serde_json::to_string(&merged_params).unwrap_or_default();
            let encrypted = eapi_encrypt(path, &text);
            let eapi_path = path.replace("/api/", "/eapi/");
            let url = format!("{EAPI}{eapi_path}");

            let mut cookie_header = String::new();
            for (k, v) in header_obj {
                if let Some(s) = v.as_str() {
                    if !cookie_header.is_empty() {
                        cookie_header.push_str("; ");
                    }
                    cookie_header.push_str(&format!(
                        "{}={}",
                        percent_encoding::utf8_percent_encode(k.as_str(), EAPI_COOKIE_ENCODING),
                        percent_encoding::utf8_percent_encode(s, EAPI_COOKIE_ENCODING)
                    ));
                }
            }

            let body_str = format!("params={encrypted}");
            let mut req = self.agent.post(&url);
            req = req.header("User-Agent", EAPI_UA);
            req = req.header("Content-Type", "application/x-www-form-urlencoded");
            req = req.header("Referer", format!("{SITE}/"));
            if !cookie_header.is_empty() {
                req = req.header("Cookie", cookie_header);
            }

            req.send(body_str.as_bytes()).map_err(map_ureq_error)?
        } else {
            let mut url = format!("{SITE}{path}");
            let mut query = String::new();
            for (k, v) in params {
                if !query.is_empty() {
                    query.push('&');
                }
                let val_str = match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                query.push_str(&format!(
                    "{}={}",
                    percent_encoding::utf8_percent_encode(k, percent_encoding::NON_ALPHANUMERIC),
                    percent_encoding::utf8_percent_encode(&val_str, percent_encoding::NON_ALPHANUMERIC)
                ));
            }
            if !query.is_empty() {
                url.push('?');
                url.push_str(&query);
            }

            let mut cookie_header = String::new();
            for name in COOKIE_NAMES {
                if let Some(v) = cookies_snapshot.get(name) {
                    if !v.is_empty() {
                        if !cookie_header.is_empty() {
                            cookie_header.push_str("; ");
                        }
                        cookie_header.push_str(&format!("{name}={v}"));
                    }
                }
            }

            let mut req = self.agent.get(&url);
            req = req.header("User-Agent", UA);
            req = req.header("Referer", format!("{SITE}/"));
            if !cookie_header.is_empty() {
                req = req.header("Cookie", cookie_header);
            }

            req.call().map_err(map_ureq_error)?
        };

        let status = resp.status();
        if !status.is_success() {
            return Err(EngineError::new(
                ErrorCode::Internal,
                format!("网易云请求失败（HTTP {}）", status.as_u16()),
            ));
        }

        let headers = resp.headers().clone();
        let body_bytes = resp
            .into_body()
            .into_with_config()
            .limit(64 * 1024 * 1024)
            .read_to_vec()
            .map_err(|e| EngineError::new(ErrorCode::Shape, format!("读取网易云响应失败: {e}")))?;

        let parsed: Value = serde_json::from_slice(&body_bytes)
            .map_err(|e| EngineError::new(ErrorCode::Shape, format!("网易云返回了无效数据: {e}")))?;

        if !parsed.is_object() {
            return Err(EngineError::new(ErrorCode::Shape, "网易云返回了无效数据"));
        }

        self.accept_response_cookies_into(&headers, cookie_jar, persist_cookies)?;

        Ok(parsed)
    }

    /// Starts a QR code login flow: returns `(unikey, qr_image_data_uri)`.
    pub fn start_qr_login(&self) -> Result<(String, String), EngineError> {
        self.pending_qr_cookies.lock().unwrap().clear();
        let mut params = HashMap::new();
        params.insert("type", json!(3));
        let res = self.request_with_codes(
            "/api/login/qrcode/unikey",
            &params,
            true,
            &[200],
            &self.pending_qr_cookies,
            false,
        )?;
        let unikey = res
            .get("unikey")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineError::new(ErrorCode::Internal, "NetEase did not return unikey"))?
            .to_string();

        let qr_url = format!("{SITE}/login?codekey={unikey}");
        let qr = qrcode::QrCode::new(qr_url.as_bytes())
            .map_err(|e| EngineError::new(ErrorCode::Internal, format!("failed to generate QR code: {e}")))?;
        let svg = qr
            .render::<qrcode::render::svg::Color>()
            .min_dimensions(200, 200)
            .build();
        let encoded_svg = percent_encoding::utf8_percent_encode(&svg, percent_encoding::NON_ALPHANUMERIC);
        let qr_data_uri = format!("data:image/svg+xml,{encoded_svg}");

        Ok((unikey, qr_data_uri))
    }

    /// Polls the status of an ongoing QR code login.
    pub fn poll_qr_login(&self, unikey: &str) -> Result<QrPollResult, EngineError> {
        let mut params = HashMap::new();
        params.insert("key", json!(unikey));
        params.insert("type", json!(3));
        let res = self.request_with_codes(
            "/api/login/qrcode/client/login",
            &params,
            true,
            &[800, 801, 802, 803],
            &self.pending_qr_cookies,
            false,
        )?;
        let code = res.get("code").and_then(|v| v.as_i64()).unwrap_or(801);

        match code {
            800 => {
                self.pending_qr_cookies.lock().unwrap().clear();
                Ok(QrPollResult::Expired)
            }
            801 => Ok(QrPollResult::WaitingScan),
            802 => Ok(QrPollResult::ScannedWaitingConfirm),
            803 => self.finish_qr_login_from(&self.pending_qr_cookies, &res),
            _ => Ok(QrPollResult::WaitingScan),
        }
    }

    #[cfg(test)]
    fn finish_qr_login(&self, response: &Value) -> Result<QrPollResult, EngineError> {
        self.finish_qr_login_from(&self.cookies, response)
    }

    fn finish_qr_login_from(
        &self,
        source_jar: &Mutex<HashMap<String, String>>,
        response: &Value,
    ) -> Result<QrPollResult, EngineError> {
        let mut candidate_cookies = {
            let mut lock = source_jar.lock().unwrap();
            for field in ["cookie", "cookies"] {
                if let Some(cookie) = response.get(field).and_then(Value::as_str) {
                    apply_cookie_str(&mut lock, cookie);
                }
            }
            lock.clone()
        };

        if !candidate_cookies.get("MUSIC_U").is_some_and(|v| !v.is_empty()) {
            return Err(EngineError::new(
                ErrorCode::SignedOut,
                "网易云未返回登录凭据，请重新扫码。",
            ));
        }

        let candidate_jar = Mutex::new(std::mem::take(&mut candidate_cookies));
        let profile = if let Some(profile) = profile_from_response(response) {
            profile
        } else {
            match self.fetch_profile_from_jar(&candidate_jar, false) {
                Ok(Some(profile)) => profile,
                Ok(None) | Err(_) => match self.fetch_profile_from_jar(&candidate_jar, true) {
                    Ok(Some(profile)) => profile,
                    _ => {
                        return Err(EngineError::new(
                            ErrorCode::SignedOut,
                            "无法验证网易云账号，请重新扫码。",
                        ));
                    }
                },
            }
        };

        let committed_cookies = candidate_jar.into_inner().unwrap();
        *self.cookies.lock().unwrap() = committed_cookies;
        self.pending_qr_cookies.lock().unwrap().clear();
        *self.profile.lock().unwrap() = Some(profile);
        *self.profile_checked_at.lock().unwrap() = Some(Instant::now());
        self.stream_cache.lock().unwrap().clear();
        *self.liked_cache.lock().unwrap() = None;
        *self.liked_playlist_id.lock().unwrap() = None;
        self.collection_cache.lock().unwrap().clear();
        self.persist_session();
        Ok(QrPollResult::Success)
    }

    /// Refreshes the user profile from `/api/nuser/account/get`.
    pub fn refresh_profile(&self) -> Result<Option<NetEaseProfile>, EngineError> {
        if !self.is_signed_in() {
            return Ok(None);
        }
        let web = self.request_profile(false);
        let profile = match web {
            Ok(Some(profile)) => Some(profile),
            Ok(None) => match self.request_profile(true) {
                Ok(p) => p,
                Err(_) => None,
            },
            Err(web_err) => match self.request_profile(true) {
                Ok(p) => p,
                Err(_) => return Err(web_err),
            },
        };
        if let Some(profile) = &profile {
            *self.profile.lock().unwrap() = Some(profile.clone());
            *self.profile_checked_at.lock().unwrap() = Some(Instant::now());
            self.persist_session();
        } else {
            self.clear_session();
        }
        Ok(profile)
    }

    fn request_profile(&self, is_eapi: bool) -> Result<Option<NetEaseProfile>, EngineError> {
        self.fetch_profile_from_jar(&self.cookies, is_eapi)
    }

    fn fetch_profile_from_jar(
        &self,
        cookie_jar: &Mutex<HashMap<String, String>>,
        is_eapi: bool,
    ) -> Result<Option<NetEaseProfile>, EngineError> {
        let persist = std::ptr::eq(cookie_jar, &self.cookies);
        let response = self.request_raw_into("/api/nuser/account/get", &HashMap::new(), is_eapi, cookie_jar, persist)?;
        match response.get("code").and_then(Value::as_i64) {
            Some(301 | 302 | 401) => Ok(None),
            Some(200) if response.get("profile") == Some(&Value::Null) => Ok(None),
            Some(200) => profile_from_response(&response).map(Some).ok_or_else(|| {
                EngineError::new(ErrorCode::Shape, "网易云未返回有效的账号资料")
            }),
            code => Err(EngineError::new(
                ErrorCode::Internal,
                format!("NetEase account check returned code {code:?}"),
            )),
        }
    }

    /// Fetches song details by batch from `/api/v3/song/detail`.
    pub fn song_details(&self, ids: &[String]) -> Result<Vec<Track>, EngineError> {
        self.song_details_with_fallback(ids, &[])
    }

    fn fetch_song_detail_batch(&self, chunk: &[u64]) -> Result<Value, EngineError> {
        let c_arr: Vec<Value> = chunk.iter().map(|num| json!({ "id": num })).collect();
        let mut params = HashMap::new();
        let c_str = serde_json::to_string(&c_arr).unwrap_or_default();
        params.insert("c", json!(c_str));
        self.request("/api/v3/song/detail", &params, true)
    }

    fn song_details_with_fallback(
        &self,
        ids: &[String],
        originals: &[Value],
    ) -> Result<Vec<Track>, EngineError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        // Deduplicate numeric IDs for batching while preserving the full `ids` order below.
        let mut seen_req = HashSet::with_capacity(ids.len());
        let unique_nums: Vec<u64> = ids
            .iter()
            .filter_map(|id| id.parse::<u64>().ok())
            .filter(|&num| seen_req.insert(num))
            .collect();

        let chunks: Vec<&[u64]> = unique_nums.chunks(SONG_DETAIL_BATCH).collect();
        let batch_results: Vec<Result<Value, EngineError>> = if chunks.len() <= 1 {
            chunks.into_iter().map(|c| self.fetch_song_detail_batch(c)).collect()
        } else {
            let next_idx = AtomicUsize::new(0);
            let slots: Vec<Mutex<Option<Result<Value, EngineError>>>> =
                (0..chunks.len()).map(|_| Mutex::new(None)).collect();
            let workers = chunks.len().min(SONG_DETAIL_CONCURRENCY);
            std::thread::scope(|scope| {
                for _ in 0..workers {
                    scope.spawn(|| {
                        loop {
                            let idx = next_idx.fetch_add(1, Ordering::Relaxed);
                            if idx >= chunks.len() {
                                break;
                            }
                            let res = self.fetch_song_detail_batch(chunks[idx]);
                            *slots[idx].lock().unwrap() = Some(res);
                        }
                    });
                }
            });
            slots
                .into_iter()
                .map(|slot| {
                    slot.into_inner()
                        .unwrap()
                        .unwrap_or_else(|| Err(EngineError::new(ErrorCode::Internal, "song detail batch failed")))
                })
                .collect()
        };

        let mut songs_by_id: HashMap<String, Value> = HashMap::with_capacity(unique_nums.len());
        let mut privs_by_id: HashMap<String, (i64, i64)> = HashMap::with_capacity(unique_nums.len());
        for res in batch_results {
            let res = res?;
            if let Some(privs) = res.get("privileges").and_then(Value::as_array) {
                for p in privs {
                    if let Some(id) = p.get("id").and_then(value_id_str) {
                        let st = p.get("st").and_then(Value::as_i64).unwrap_or(0);
                        let pl = p.get("pl").and_then(Value::as_i64).unwrap_or(0);
                        privs_by_id.insert(id, (st, pl));
                    }
                }
            }
            if let Some(songs) = res.get("songs").and_then(Value::as_array) {
                for s in songs {
                    if let Some(id) = s.get("id").and_then(value_id_str) {
                        songs_by_id.insert(id, s.clone());
                    }
                }
            }
        }

        Ok(build_tracks_from_details(ids, &songs_by_id, &privs_by_id, originals))
    }

    /// The home page content (recommended playlists and charts).
    pub fn home(&self) -> Result<Home, EngineError> {
        let profile = if self.is_signed_in() {
            match self.require_profile() {
                Ok(p) => Some(p),
                Err(e) if e.code == ErrorCode::SignedOut => None,
                Err(_) => self.profile.lock().unwrap().clone(),
            }
        } else {
            None
        };
        let greeting = if let Some(ref p) = profile {
            format!("发现音乐 · {}", p.name)
        } else {
            "网易云音乐 · 发现音乐".to_string()
        };

        let (rec_res, top_res) = std::thread::scope(|scope| {
            let rec_handle = scope.spawn(|| {
                let mut params = HashMap::new();
                params.insert("limit", json!(24));
                self.request("/api/personalized/playlist", &params, false)
            });
            let top_handle = scope.spawn(|| self.request("/api/toplist/detail", &HashMap::new(), false));
            (
                rec_handle.join().unwrap_or_else(|_| Err(EngineError::new(ErrorCode::Internal, "home request failed"))),
                top_handle.join().unwrap_or_else(|_| Err(EngineError::new(ErrorCode::Internal, "toplist request failed"))),
            )
        });

        let rec_items: Vec<Item> = match &rec_res {
            Ok(res) => res
                .get("result")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|p| {
                            let id = p.get("id").and_then(value_id_str)?;
                            let name = p.get("name")?.as_str()?.to_string();
                            let image = artwork_url(p.get("picUrl").and_then(Value::as_str));
                            let play_count = p.get("playCount").and_then(Value::as_f64).unwrap_or(0.0);
                            let subtitle = if play_count >= 10000.0 {
                                format!("{:.1} 万播放", play_count / 10000.0)
                            } else {
                                "推荐歌单".to_string()
                            };
                            Some(Item {
                                uri: format!("netease:playlist:{id}"),
                                kind: "playlist".to_string(),
                                name,
                                subtitle,
                                image,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default(),
            Err(_) => Vec::new(),
        };

        let top_items: Vec<Item> = match &top_res {
            Ok(res) => res
                .get("list")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .take(12)
                        .filter_map(|p| {
                            let id = p.get("id").and_then(value_id_str)?;
                            let name = p.get("name")?.as_str()?.to_string();
                            let image = artwork_url(p.get("coverImgUrl").and_then(Value::as_str));
                            let subtitle = p
                                .get("updateFrequency")
                                .and_then(Value::as_str)
                                .unwrap_or("官方榜单")
                                .to_string();
                            Some(Item {
                                uri: format!("netease:playlist:{id}"),
                                kind: "playlist".to_string(),
                                name,
                                subtitle,
                                image,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default(),
            Err(_) => Vec::new(),
        };

        if let (Err(rec_err), Err(top_err)) = (&rec_res, &top_res) {
            if rec_err.code == ErrorCode::SignedOut {
                return Err(rec_err.clone());
            }
            if top_err.code == ErrorCode::SignedOut {
                return Err(top_err.clone());
            }
            return Err(rec_err.clone());
        }

        let mut sections = Vec::new();

        // 1. Personalized recommendation section when signed in
        if profile.is_some() {
            let cover0 = rec_items.first().map(|i| i.image.clone()).unwrap_or_default();
            let cover1 = rec_items.get(1).map(|i| i.image.clone()).unwrap_or_else(|| cover0.clone());
            sections.push(Section {
                title: "为你推荐".to_string(),
                items: vec![
                    Item {
                        uri: "netease:playlist:daily".to_string(),
                        kind: "playlist".to_string(),
                        name: "每日推荐".to_string(),
                        subtitle: "每天 6:00 更新 · 专属推荐".to_string(),
                        image: cover0,
                    },
                    Item {
                        uri: "netease:collection:tracks".to_string(),
                        kind: "collection".to_string(),
                        name: "我喜欢的音乐".to_string(),
                        subtitle: "红心歌单".to_string(),
                        image: String::new(),
                    },
                    Item {
                        uri: "netease:playlist:3136952023".to_string(),
                        kind: "playlist".to_string(),
                        name: "私人雷达".to_string(),
                        subtitle: "根据你的听歌偏好生成".to_string(),
                        image: cover1,
                    },
                ],
            });
        }

        // 2. Recommended playlists
        if !rec_items.is_empty() {
            sections.push(Section {
                title: "推荐歌单".to_string(),
                items: rec_items,
            });
        }

        // 3. Top lists / Charts
        if !top_items.is_empty() {
            sections.push(Section {
                title: "排行榜".to_string(),
                items: top_items,
            });
        }

        Ok(Home { greeting, sections })
    }

    /// Searches NetEase for tracks, albums, artists, and playlists.
    pub fn search(&self, query: &str) -> Result<SearchResults, EngineError> {
        let text = query.trim();
        if text.is_empty() {
            return Ok(SearchResults::default());
        }

        let search_type = |kind: i64, limit: i64| {
            let mut params = HashMap::new();
            params.insert("s", json!(text));
            params.insert("type", json!(kind));
            params.insert("limit", json!(limit));
            params.insert("offset", json!(0));
            params.insert("total", json!("true"));
            self.request("/api/cloudsearch/pc", &params, false)
        };

        let (track_res, artist_res, album_res, playlist_res) = std::thread::scope(|scope| {
            let t = scope.spawn(|| search_type(1, 30));
            let ar = scope.spawn(|| search_type(100, 8));
            let al = scope.spawn(|| search_type(10, 8));
            let pl = scope.spawn(|| search_type(1000, 8));
            (
                t.join().unwrap_or_else(|_| Err(EngineError::new(ErrorCode::Internal, "search failed"))),
                ar.join().unwrap_or_else(|_| Err(EngineError::new(ErrorCode::Internal, "search failed"))),
                al.join().unwrap_or_else(|_| Err(EngineError::new(ErrorCode::Internal, "search failed"))),
                pl.join().unwrap_or_else(|_| Err(EngineError::new(ErrorCode::Internal, "search failed"))),
            )
        });

        let track_res = track_res?;
        let artist_res = artist_res?;
        let album_res = album_res?;
        let playlist_res = playlist_res?;

        let raw_songs = track_res
            .get("result")
            .and_then(|r| r.get("songs"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let song_ids: Vec<String> = raw_songs
            .iter()
            .filter_map(|s| s.get("id").and_then(value_id_str))
            .collect();
        let tracks = self.song_details_with_fallback(&song_ids, &raw_songs)?;

        let artists: Vec<Item> = artist_res
            .get("result")
            .and_then(|r| r.get("artists"))
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|a| {
                        let id = a.get("id").and_then(value_id_str)?;
                        let name = a.get("name")?.as_str()?.to_string();
                        let image = artwork_url(a.get("picUrl").or_else(|| a.get("img1v1Url")).and_then(Value::as_str));
                        Some(Item {
                            uri: format!("netease:artist:{id}"),
                            kind: "artist".to_string(),
                            name,
                            subtitle: "歌手".to_string(),
                            image,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let albums: Vec<Item> = album_res
            .get("result")
            .and_then(|r| r.get("albums"))
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|a| {
                        let id = a.get("id").and_then(value_id_str)?;
                        let name = a.get("name")?.as_str()?.to_string();
                        let artist_name = a
                            .get("artist")
                            .and_then(|ar| ar.get("name").and_then(Value::as_str))
                            .unwrap_or("未知歌手");
                        let image = artwork_url(a.get("picUrl").and_then(Value::as_str));
                        Some(Item {
                            uri: format!("netease:album:{id}"),
                            kind: "album".to_string(),
                            name,
                            subtitle: artist_name.to_string(),
                            image,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let playlists: Vec<Item> = playlist_res
            .get("result")
            .and_then(|r| r.get("playlists"))
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|p| {
                        let id = p.get("id").and_then(value_id_str)?;
                        let name = p.get("name")?.as_str()?.to_string();
                        let owner = p
                            .get("creator")
                            .and_then(|c| c.get("nickname").and_then(Value::as_str))
                            .unwrap_or("网易云歌单");
                        let image = artwork_url(p.get("coverImgUrl").and_then(Value::as_str));
                        Some(Item {
                            uri: format!("netease:playlist:{id}"),
                            kind: "playlist".to_string(),
                            name,
                            subtitle: format!("by {owner}"),
                            image,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(SearchResults {
            tracks,
            artists,
            albums,
            playlists,
        })
    }

    /// Fetches a playlist or daily songs as a Collection.
    pub fn playlist(&self, id_or_uri: &str) -> Result<Collection, EngineError> {
        let uri = if id_or_uri.starts_with("netease:") {
            id_or_uri.to_string()
        } else {
            format!("netease:playlist:{id_or_uri}")
        };

        if uri == "netease:playlist:daily" {
            // Daily recommend songs
            let _ = self.require_profile()?;
            let res = self.request("/api/v3/discovery/recommend/songs", &HashMap::new(), false)?;
            let songs = res
                .get("data")
                .and_then(|d| d.get("dailySongs"))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let ids: Vec<String> = songs
                .iter()
                .filter_map(|s| s.get("id").and_then(value_id_str))
                .collect();
            let tracks = self.song_details_with_fallback(&ids, &songs)?;

            let collection = Collection {
                header: Header {
                    uri: "netease:playlist:daily".to_string(),
                    kind: "playlist".to_string(),
                    name: "每日推荐".to_string(),
                    subtitle: "网易云音乐".to_string(),
                    image: tracks.first().map(|t| t.image.clone()).unwrap_or_default(),
                    description: "根据你的音乐口味生成，每天 6:00 更新".to_string(),
                },
                total: tracks.len() as f64,
                tracks,
                releases: Vec::new(),
            };
            self.cache_collection(&collection);
            return Ok(collection);
        }

        let raw_id = valid_positive_id(uri.strip_prefix("netease:playlist:").unwrap_or(&uri))?;
        let p = self.fetch_playlist_detail(&raw_id)?;
        let parsed = parse_playlist_detail(&p);
        let tracks = self.song_details_with_fallback(&parsed.track_ids, parsed.preview_tracks)?;
        let total = parsed.track_count.unwrap_or(tracks.len() as f64);

        let collection = Collection {
            header: Header {
                uri: format!("netease:playlist:{raw_id}"),
                kind: "playlist".to_string(),
                name: parsed.name,
                subtitle: parsed.subtitle,
                image: parsed.image,
                description: parsed.description,
            },
            total,
            tracks,
            releases: Vec::new(),
        };
        self.cache_collection(&collection);
        Ok(collection)
    }

    /// Fetches an album by id.
    pub fn album(&self, id_or_uri: &str) -> Result<Collection, EngineError> {
        let raw_id = valid_positive_id(id_or_uri.strip_prefix("netease:album:").unwrap_or(id_or_uri))?;
        let path = format!("/api/v1/album/{raw_id}");
        let res = self.request(&path, &HashMap::new(), false)?;
        let a = res
            .get("album")
            .ok_or_else(|| EngineError::new(ErrorCode::NotFound, "专辑不存在或不可用"))?;

        let name = a.get("name").and_then(Value::as_str).unwrap_or("专辑").to_string();
        let mut subtitle = a
            .get("artists")
            .and_then(Value::as_array)
            .map(|artists| {
                artists
                    .iter()
                    .filter_map(|ar| ar.get("name").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        if subtitle.is_empty() {
            subtitle = a
                .get("artist")
                .and_then(|ar| ar.get("name").and_then(Value::as_str))
                .unwrap_or_default()
                .to_string();
        }
        let image = artwork_url(a.get("picUrl").and_then(Value::as_str));
        let description = a.get("description").and_then(Value::as_str).unwrap_or("").to_string();

        let raw_songs = res
            .get("songs")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let song_ids: Vec<String> = raw_songs
            .iter()
            .filter_map(|s| s.get("id").and_then(value_id_str))
            .collect();

        let tracks = self.song_details_with_fallback(&song_ids, raw_songs)?;

        let collection = Collection {
            header: Header {
                uri: format!("netease:album:{raw_id}"),
                kind: "album".to_string(),
                name,
                subtitle,
                image,
                description,
            },
            total: tracks.len() as f64,
            tracks,
            releases: Vec::new(),
        };
        self.cache_collection(&collection);
        Ok(collection)
    }

    /// Fetches an artist by id.
    pub fn artist(&self, id_or_uri: &str) -> Result<Collection, EngineError> {
        let raw_id = valid_positive_id(id_or_uri.strip_prefix("netease:artist:").unwrap_or(id_or_uri))?;
        let artist_path = format!("/api/v1/artist/{raw_id}");
        let albums_path = format!("/api/artist/albums/{raw_id}");

        let (res, alb_res) = std::thread::scope(|scope| {
            let artist_h = scope.spawn(|| self.request(&artist_path, &HashMap::new(), false));
            let albums_h = scope.spawn(|| {
                let mut album_params = HashMap::new();
                album_params.insert("limit", json!(50));
                self.request(&albums_path, &album_params, false)
            });
            (
                artist_h.join().unwrap_or_else(|_| Err(EngineError::new(ErrorCode::Internal, "artist request failed"))),
                albums_h.join().unwrap_or_else(|_| Err(EngineError::new(ErrorCode::Internal, "artist albums request failed"))),
            )
        });

        let res = res?;
        let ar = res
            .get("artist")
            .ok_or_else(|| EngineError::new(ErrorCode::NotFound, "歌手不存在或不可用"))?;

        let name = ar.get("name").and_then(Value::as_str).unwrap_or("歌手").to_string();
        let image = artwork_url(ar.get("picUrl").or_else(|| ar.get("img1v1Url")).and_then(Value::as_str));
        let description = ar.get("briefDesc").and_then(Value::as_str).unwrap_or("").to_string();

        let raw_hot = res
            .get("hotSongs")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let hot_ids: Vec<String> = raw_hot
            .iter()
            .filter_map(|s| s.get("id").and_then(value_id_str))
            .collect();
        let tracks = self.song_details_with_fallback(&hot_ids, raw_hot)?;

        let releases = alb_res
            .ok()
            .and_then(|r| r.get("hotAlbums").and_then(Value::as_array).cloned())
            .map(|arr| {
                arr.iter()
                    .filter_map(|a| {
                        let id = a.get("id").and_then(value_id_str)?;
                        let a_name = a.get("name")?.as_str()?.to_string();
                        let a_img = artwork_url(a.get("picUrl").and_then(Value::as_str));
                        Some(Item {
                            uri: format!("netease:album:{id}"),
                            kind: "album".to_string(),
                            name: a_name,
                            subtitle: "专辑".to_string(),
                            image: a_img,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let collection = Collection {
            header: Header {
                uri: format!("netease:artist:{raw_id}"),
                kind: "artist".to_string(),
                name,
                subtitle: "歌手".to_string(),
                image,
                description,
            },
            total: tracks.len() as f64,
            tracks,
            releases,
        };
        self.cache_collection(&collection);
        Ok(collection)
    }

    fn liked_ids_for(&self, uid: &str) -> Result<Vec<String>, EngineError> {
        {
            let lock = self.liked_cache.lock().unwrap();
            if let Some(ref cache) = *lock {
                if cache.uid == uid && cache.at.elapsed() < LIKED_TTL {
                    return Ok(cache.ids.clone());
                }
            }
        }

        let mut params = HashMap::new();
        params.insert("uid", json!(uid));
        let res = self.request("/api/song/like/get", &params, true)?;
        let ids: Vec<String> = res
            .get("ids")
            .and_then(Value::as_array)
            .map(|arr| arr.iter().filter_map(value_id_str).collect())
            .unwrap_or_default();
        let id_set: HashSet<String> = ids.iter().cloned().collect();

        *self.liked_cache.lock().unwrap() = Some(LikedCache {
            uid: uid.to_string(),
            ids: ids.clone(),
            id_set,
            at: Instant::now(),
        });

        Ok(ids)
    }

    /// Best-effort check of whether `id_or_uri` is in the signed-in user's Liked Songs.
    pub fn is_track_liked(&self, id_or_uri: &str) -> bool {
        if !self.is_signed_in() {
            return false;
        }
        let raw_id = id_or_uri.strip_prefix("netease:track:").unwrap_or(id_or_uri);
        if raw_id.is_empty() {
            return false;
        }
        let Ok(profile) = self.require_profile() else {
            return false;
        };
        if self.liked_ids_for(&profile.id).is_err() {
            return false;
        }
        let lock = self.liked_cache.lock().unwrap();
        lock.as_ref().is_some_and(|c| c.uid == profile.id && c.id_set.contains(raw_id))
    }

    fn cache_collection(&self, collection: &Collection) {
        let mut cache = self.collection_cache.lock().unwrap();
        cache.retain(|_, (_, at)| at.elapsed() < COLLECTION_PLAYBACK_TTL);
        cache.insert(collection.header.uri.clone(), (collection.clone(), Instant::now()));
    }

    /// Returns tracks for `uri` when starting playback, reusing a recent
    /// collection fetch if still fresh to avoid re-fetching large lists.
    pub fn collection_tracks_for_playback(&self, uri: &str) -> Result<Vec<Track>, EngineError> {
        {
            let lock = self.collection_cache.lock().unwrap();
            if let Some((col, at)) = lock.get(uri) {
                if at.elapsed() < COLLECTION_PLAYBACK_TTL {
                    return Ok(col.tracks.clone());
                }
            }
        }
        let col = if uri == "netease:collection:tracks" {
            self.liked_songs()?
        } else if uri.starts_with("netease:playlist:") {
            self.playlist(uri)?
        } else if uri.starts_with("netease:album:") {
            self.album(uri)?
        } else if uri.starts_with("netease:artist:") {
            self.artist(uri)?
        } else {
            return Ok(Vec::new());
        };
        Ok(col.tracks)
    }

    fn fetch_playlist_detail(&self, raw_id: &str) -> Result<Value, EngineError> {
        let mut params = HashMap::new();
        params.insert("id", json!(raw_id));
        params.insert("n", json!(PLAYLIST_PREVIEW_LIMIT));
        let res = self.request("/api/v6/playlist/detail", &params, false)?;
        res.get("playlist")
            .cloned()
            .ok_or_else(|| EngineError::new(ErrorCode::NotFound, "歌单不存在或无权访问"))
    }

    fn fetch_user_playlist_page(&self, uid: &str, offset: usize) -> Result<Value, EngineError> {
        let mut params = HashMap::new();
        params.insert("uid", json!(uid));
        params.insert("limit", json!(200));
        params.insert("offset", json!(offset));
        self.request("/api/user/playlist", &params, false)
    }

    fn liked_playlist_id(&self, uid: &str) -> Result<Option<String>, EngineError> {
        {
            let lock = self.liked_playlist_id.lock().unwrap();
            if let Some((cached_uid, pid)) = lock.as_ref() {
                if cached_uid == uid {
                    return Ok(Some(pid.clone()));
                }
            }
        }

        let mut offset = 0usize;
        while offset < 2000 {
            let res = self.fetch_user_playlist_page(uid, offset)?;
            let arr = res.get("playlist").and_then(Value::as_array);
            let page_len = arr.map_or(0, Vec::len);

            if let Some(arr) = arr {
                for p in arr {
                    if is_own_liked_playlist(p, uid) {
                        if let Some(id) = p.get("id").and_then(value_id_str) {
                            *self.liked_playlist_id.lock().unwrap() = Some((uid.to_string(), id.clone()));
                            return Ok(Some(id));
                        }
                    }
                }
            }

            let more = res.get("more").and_then(Value::as_bool).unwrap_or(false);
            if !more || page_len < 200 {
                break;
            }
            offset += page_len;
        }

        Ok(None)
    }

    /// Fetches the signed-in user's Liked Songs.
    ///
    /// Prefers the user's `specialType == 5` playlist via `/api/v6/playlist/detail`
    /// so tracks keep NetEase's official order (newest liked on top) and include
    /// the full `trackIds` list without the 300/1000 truncation. Falls back to
    /// `/api/song/like/get` only if that playlist cannot be located.
    pub fn liked_songs(&self) -> Result<Collection, EngineError> {
        let profile = self.require_profile()?;

        let mut playlist_result = None;
        if let Some(pid) = self.liked_playlist_id(&profile.id)? {
            match self.fetch_playlist_detail(&pid) {
                Ok(p) => {
                    let parsed = parse_playlist_detail(&p);
                    let tracks = self.song_details_with_fallback(&parsed.track_ids, parsed.preview_tracks)?;
                    let cover = tracks
                        .first()
                        .map(|t| t.image.clone())
                        .filter(|s| !s.is_empty())
                        .unwrap_or(parsed.image);
                    let total_count = parsed
                        .track_count
                        .map(|n| n.max(0.0).round() as usize)
                        .unwrap_or(tracks.len())
                        .max(tracks.len());
                    playlist_result = Some((tracks, cover, total_count));
                }
                Err(err) if err.code == ErrorCode::NotFound => {
                    *self.liked_playlist_id.lock().unwrap() = None;
                }
                Err(err) => return Err(err),
            }
        }

        let (tracks, cover, total_count) = match playlist_result {
            Some(found) => found,
            None => {
                let ids = self.liked_ids_for(&profile.id)?;
                let tracks = self.song_details(&ids)?;
                let cover = tracks.first().map(|t| t.image.clone()).unwrap_or_default();
                let total_count = ids.len().max(tracks.len());
                (tracks, cover, total_count)
            }
        };

        let collection = Collection {
            header: Header {
                uri: "netease:collection:tracks".to_string(),
                kind: "collection".to_string(),
                name: "我喜欢的音乐".to_string(),
                subtitle: profile.name,
                image: cover,
                description: format!("网易云音乐 · 共 {total_count} 首喜欢的音乐"),
            },
            total: total_count as f64,
            tracks,
            releases: Vec::new(),
        };
        self.cache_collection(&collection);
        Ok(collection)
    }

    /// Fetches the user's library playlists.
    pub fn library(&self) -> Result<Library, EngineError> {
        let profile = self.require_profile()?;

        let mut playlist_items = Vec::new();
        let mut liked_cover = String::new();
        let mut offset = 0usize;

        while offset < 2000 {
            let res = self.fetch_user_playlist_page(&profile.id, offset)?;
            let arr = res.get("playlist").and_then(Value::as_array);
            let page_len = arr.map_or(0, Vec::len);

            if let Some(arr) = arr {
                for p in arr {
                    if is_own_liked_playlist(p, &profile.id) {
                        if let Some(id) = p.get("id").and_then(value_id_str) {
                            *self.liked_playlist_id.lock().unwrap() = Some((profile.id.clone(), id));
                        }
                        if liked_cover.is_empty() {
                            liked_cover = artwork_url(p.get("coverImgUrl").and_then(Value::as_str));
                        }
                        continue;
                    }
                    if let Some(id) = p.get("id").and_then(value_id_str) {
                        let name = p.get("name").and_then(Value::as_str).unwrap_or("").to_string();
                        let count = p.get("trackCount").and_then(Value::as_i64).unwrap_or(0);
                        let owner = p
                            .get("creator")
                            .and_then(|c| c.get("nickname").and_then(Value::as_str))
                            .unwrap_or("");
                        let image = artwork_url(p.get("coverImgUrl").and_then(Value::as_str));
                        playlist_items.push(Item {
                            uri: format!("netease:playlist:{id}"),
                            kind: "playlist".to_string(),
                            name,
                            subtitle: format!("{count} 首 · by {owner}"),
                            image,
                        });
                    }
                }
            }

            let more = res.get("more").and_then(Value::as_bool).unwrap_or(false);
            if !more || page_len < 200 {
                break;
            }
            offset += page_len;
        }

        let daily_cover = playlist_items
            .first()
            .map(|i| i.image.clone())
            .unwrap_or_default();

        let mut items = vec![
            Item {
                uri: "netease:collection:tracks".to_string(),
                kind: "collection".to_string(),
                name: "我喜欢的音乐".to_string(),
                subtitle: "心动精选".to_string(),
                image: liked_cover,
            },
            Item {
                uri: "netease:playlist:daily".to_string(),
                kind: "playlist".to_string(),
                name: "每日推荐".to_string(),
                subtitle: "每天更新".to_string(),
                image: daily_cover,
            },
        ];
        items.extend(playlist_items);

        Ok(Library { items })
    }

    /// Gets the signed-in user info (`Me`).
    pub fn me(&self) -> Result<Me, EngineError> {
        let profile = self.require_profile()?;

        Ok(Me {
            id: profile.id,
            name: profile.name.clone(),
            username: profile.name,
            image: profile.avatar_url,
        })
    }

    /// Fetches lyrics for a track ID.
    pub fn lyrics(&self, id_or_uri: &str) -> Result<Lyrics, EngineError> {
        let raw_id = valid_positive_id(id_or_uri.strip_prefix("netease:track:").unwrap_or(id_or_uri))?;
        let mut params = HashMap::new();
        params.insert("id", json!(raw_id));
        params.insert("lv", json!(1));
        params.insert("tv", json!(-1));

        let res = self.request("/api/song/lyric", &params, false)?;
        let mut lrc = res
            .get("lrc")
            .and_then(|l| l.get("lyric"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        if lrc.trim().is_empty() {
            // Fallback to desktop lyric API
            let mut eapi_params = HashMap::new();
            eapi_params.insert("id", json!(raw_id));
            eapi_params.insert("cp", json!(false));
            eapi_params.insert("tv", json!(0));
            eapi_params.insert("lv", json!(0));
            eapi_params.insert("rv", json!(0));
            eapi_params.insert("kv", json!(0));
            eapi_params.insert("yv", json!(0));
            eapi_params.insert("ytv", json!(0));
            eapi_params.insert("yrv", json!(0));
            if let Ok(desktop_res) = self.request("/api/song/lyric/v1", &eapi_params, true) {
                if let Some(d_lrc) = desktop_res
                    .get("lrc")
                    .and_then(|l| l.get("lyric"))
                    .and_then(Value::as_str)
                {
                    lrc = d_lrc.to_string();
                }
            }
        }

        Ok(parse_lyrics(&lrc))
    }
}

struct ParsedPlaylist<'a> {
    name: String,
    subtitle: String,
    image: String,
    description: String,
    preview_tracks: &'a [Value],
    track_ids: Vec<String>,
    track_count: Option<f64>,
}

fn is_own_liked_playlist(p: &Value, uid: &str) -> bool {
    p.get("specialType").and_then(Value::as_i64) == Some(5)
        && p.get("creator")
            .and_then(|c| c.get("userId"))
            .and_then(value_id_str)
            .as_deref()
            == Some(uid)
}

fn parse_playlist_detail(p: &Value) -> ParsedPlaylist<'_> {
    let name = p.get("name").and_then(Value::as_str).unwrap_or("歌单").to_string();
    let subtitle = p
        .get("creator")
        .and_then(|c| c.get("nickname").and_then(Value::as_str))
        .unwrap_or("网易云歌单")
        .to_string();
    let image = artwork_url(p.get("coverImgUrl").or_else(|| p.get("picUrl")).and_then(Value::as_str));
    let description = p.get("description").and_then(Value::as_str).unwrap_or("").to_string();
    let preview_tracks = p
        .get("tracks")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let track_ids: Vec<String> = p
        .get("trackIds")
        .and_then(Value::as_array)
        .or_else(|| p.get("tracks").and_then(Value::as_array))
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t.get("id").and_then(value_id_str))
                .collect()
        })
        .unwrap_or_default();
    let track_count = p.get("trackCount").and_then(Value::as_f64);

    ParsedPlaylist {
        name,
        subtitle,
        image,
        description,
        preview_tracks,
        track_ids,
        track_count,
    }
}

fn unavailable_track(id_str: &str) -> Track {
    Track {
        uri: format!("netease:track:{id_str}"),
        uid: id_str.to_string(),
        name: "歌曲暂不可用".to_string(),
        artists: String::new(),
        album: String::new(),
        album_uri: String::new(),
        image: String::new(),
        duration_ms: 0.0,
        explicit: false,
        playable: false,
    }
}

fn build_tracks_from_details(
    ids: &[String],
    songs_by_id: &HashMap<String, Value>,
    privs_by_id: &HashMap<String, (i64, i64)>,
    originals: &[Value],
) -> Vec<Track> {
    let fallback_by_id: HashMap<String, &Value> = originals
        .iter()
        .filter_map(|s| s.get("id").and_then(value_id_str).map(|id| (id, s)))
        .collect();

    let mut seen_uids = HashSet::with_capacity(ids.len());
    let mut tracks = Vec::with_capacity(ids.len());
    for (idx, raw_id) in ids.iter().enumerate() {
        let mut track = if let Some(s) = songs_by_id.get(raw_id) {
            track_from_json(raw_id, s, privs_by_id.get(raw_id).copied(), false)
        } else if let Some(s) = fallback_by_id.get(raw_id) {
            track_from_json(raw_id, s, None, true)
        } else {
            unavailable_track(raw_id)
        };
        if !seen_uids.insert(raw_id.clone()) {
            track.uid = format!("{raw_id}#{idx}");
        }
        tracks.push(track);
    }
    tracks
}

fn map_ureq_error(err: ureq::Error) -> EngineError {
    match err {
        ureq::Error::Timeout(_) => {
            EngineError::new(ErrorCode::Timeout, "网易云请求超时，请稍后重试")
        }
        other => EngineError::new(
            ErrorCode::Internal,
            format!("无法读取网易云音乐，请检查网络及系统代理后重试: {other}"),
        ),
    }
}

fn track_from_json(
    id_str: &str,
    s: &Value,
    priv_entry: Option<(i64, i64)>,
    force_unavailable: bool,
) -> Track {
    let name = s
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(if force_unavailable { "歌曲暂不可用" } else { "" })
        .to_string();

    let artists = s
        .get("ar")
        .or_else(|| s.get("artists"))
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|a| a.get("name").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();

    let album_obj = s.get("al").or_else(|| s.get("album"));
    let album = album_obj
        .and_then(|a| a.get("name").and_then(Value::as_str))
        .unwrap_or("")
        .to_string();
    let album_id = album_obj
        .and_then(|a| a.get("id").and_then(value_id_str))
        .unwrap_or_default();
    let album_uri = if album_id.is_empty() {
        String::new()
    } else {
        format!("netease:album:{album_id}")
    };
    let image = artwork_url(album_obj.and_then(|a| a.get("picUrl").and_then(Value::as_str)));

    let duration_ms = s
        .get("dt")
        .or_else(|| s.get("duration"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);

    let playable = if force_unavailable {
        false
    } else if let Some((st, pl)) = priv_entry {
        st >= 0 && pl > 0
    } else if let Some(p) = s.get("privilege") {
        let st = p.get("st").and_then(Value::as_i64).unwrap_or(0);
        let pl = p.get("pl").and_then(Value::as_i64).unwrap_or(0);
        st >= 0 && pl > 0
    } else {
        let fee = s.get("fee").and_then(Value::as_i64).unwrap_or(0);
        fee != 1 && fee != 4
    };

    Track {
        uri: format!("netease:track:{id_str}"),
        uid: id_str.to_string(),
        name,
        artists,
        album,
        album_uri,
        image,
        duration_ms,
        explicit: false,
        playable,
    }
}

fn parse_lrc_timestamp_ms(tag: &str) -> Option<f64> {
    let (min_str, sec_part) = tag.split_once(':')?;
    let mins: f64 = min_str.trim().parse().ok()?;
    if mins < 0.0 || !mins.is_finite() {
        return None;
    }
    // Support both `[mm:ss.xx]` and NetEase legacy `[mm:ss:xx]`.
    let normalized_sec = if let Some((s, frac)) = sec_part.split_once(':') {
        format!("{s}.{frac}")
    } else {
        sec_part.to_string()
    };
    let secs: f64 = normalized_sec.trim().parse().ok()?;
    if secs < 0.0 || !secs.is_finite() {
        return None;
    }
    Some(mins * 60_000.0 + secs * 1000.0)
}

fn is_lrc_metadata_tag(tag: &str) -> bool {
    let Some((key, _)) = tag.split_once(':') else { return false };
    let key = key.trim();
    !key.is_empty() && key.chars().all(|c| c.is_ascii_alphabetic())
}

fn parse_lyrics(lrc: &str) -> Lyrics {
    let mut timed_lines = Vec::new();
    let mut plain_lines = Vec::new();

    for raw_line in lrc.lines() {
        let mut rest = raw_line.trim();
        if rest.is_empty() {
            continue;
        }
        let mut timestamps = Vec::new();
        let mut metadata_only = false;
        while rest.starts_with('[') {
            let Some(close_idx) = rest.find(']') else { break };
            let tag = &rest[1..close_idx];
            if let Some(ms) = parse_lrc_timestamp_ms(tag) {
                timestamps.push(ms);
                rest = rest[close_idx + 1..].trim_start();
            } else if is_lrc_metadata_tag(tag) {
                metadata_only = true;
                rest = rest[close_idx + 1..].trim_start();
            } else {
                break;
            }
        }

        if !timestamps.is_empty() {
            let words = if rest.trim().is_empty() {
                "♪".to_string()
            } else {
                rest.trim().to_string()
            };
            for start_ms in timestamps {
                timed_lines.push(LyricLine {
                    start_ms,
                    words: words.clone(),
                });
            }
        } else if !metadata_only && !rest.trim().is_empty() {
            plain_lines.push(LyricLine {
                start_ms: 0.0,
                words: rest.trim().to_string(),
            });
        }
    }

    if !timed_lines.is_empty() {
        timed_lines.sort_by(|a, b| a.start_ms.partial_cmp(&b.start_ms).unwrap_or(std::cmp::Ordering::Equal));
        Lyrics {
            synced: true,
            lines: timed_lines,
            provider: "网易云音乐".to_string(),
        }
    } else {
        Lyrics {
            synced: false,
            lines: plain_lines,
            provider: "网易云音乐".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StreamLookup {
    /// A playable full-track stream URL.
    Url(String),
    /// The track is restricted (e.g., VIP trial clip or copyright unavailable);
    /// querying additional quality tiers on the same endpoint is pointless.
    Unavailable,
    /// This quality tier or endpoint returned no URL, so a fallback tier or endpoint may be tried.
    TryFallback,
}

fn parse_player_url_response(res: &Value, id_num: u64) -> Result<StreamLookup, EngineError> {
    let code = res.get("code").and_then(Value::as_i64).unwrap_or(200);
    if code == 460 || code == -460 {
        return Err(EngineError::new(ErrorCode::Internal, format!("460:{code}")));
    }

    let audio_obj = res
        .get("data")
        .and_then(Value::as_array)
        .and_then(|arr| arr.iter().find(|item| item.get("id").and_then(value_id_u64) == Some(id_num)));

    let Some(audio) = audio_obj else {
        return Ok(StreamLookup::TryFallback);
    };

    if let Some(audio_code) = audio.get("code").and_then(Value::as_i64) {
        if audio_code == 460 || audio_code == -460 {
            return Err(EngineError::new(ErrorCode::Internal, format!("460:{audio_code}")));
        }
        if audio_code != 200 {
            return Ok(StreamLookup::Unavailable);
        }
    }

    // freeTrialInfo present and not null indicates only a preview clip rather than complete track
    if let Some(trial) = audio.get("freeTrialInfo") {
        if !trial.is_null() {
            return Ok(StreamLookup::Unavailable);
        }
    }

    let raw_url = audio.get("url").and_then(Value::as_str).unwrap_or("");
    if raw_url.is_empty() {
        return Ok(StreamLookup::TryFallback);
    }

    let secure_url = if raw_url.starts_with("http://") {
        raw_url.replacen("http://", "https://", 1)
    } else {
        raw_url.to_string()
    };

    if !secure_url.starts_with("https://") {
        return Ok(StreamLookup::TryFallback);
    }
    let Some(host) = url_host(&secure_url) else {
        return Ok(StreamLookup::TryFallback);
    };
    if !is_music_126_audio_host(host) {
        return Ok(StreamLookup::TryFallback);
    }

    let audio_type = audio
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    if !audio_type.is_empty() && !matches!(audio_type.as_str(), "mp3" | "flac" | "ogg" | "m4a" | "aac" | "wav") {
        return Ok(StreamLookup::TryFallback);
    }

    Ok(StreamLookup::Url(secure_url))
}

impl NetEaseClient {
    fn query_player_url(
        &self,
        id_num: u64,
        level: &str,
        encode_type: &str,
        is_eapi: bool,
    ) -> Result<StreamLookup, EngineError> {
        let mut params = HashMap::new();
        params.insert("ids", json!(format!("[{id_num}]")));
        params.insert("level", json!(level));
        params.insert("encodeType", json!(encode_type));

        let res = self.request("/api/song/enhance/player/url/v1", &params, is_eapi)?;
        parse_player_url_response(&res, id_num)
    }

    fn fetch_stream_url_once(&self, id_num: u64) -> Result<Option<String>, EngineError> {
        let tiers: &[(&str, &str)] = &[("exhigh", "mp3"), ("standard", "mp3")];
        let mut last_err: Option<EngineError> = None;

        for &(level, encode_type) in tiers {
            match self.query_player_url(id_num, level, encode_type, true) {
                Ok(StreamLookup::Url(url)) => return Ok(Some(url)),
                Ok(StreamLookup::Unavailable) => return Ok(None),
                Ok(StreamLookup::TryFallback) => continue,
                Err(err) if err.code == ErrorCode::SignedOut => return Err(err),
                Err(err) if err.message.starts_with("460:") => {
                    // When EAPI hits risk control 460, try Web API endpoint for this tier (which also refreshes NMTID).
                    match self.query_player_url(id_num, level, encode_type, false) {
                        Ok(StreamLookup::Url(url)) => return Ok(Some(url)),
                        Ok(StreamLookup::Unavailable) => return Ok(None),
                        Ok(StreamLookup::TryFallback) => {
                            last_err = Some(err);
                            continue;
                        }
                        Err(web_err) if web_err.code == ErrorCode::SignedOut => return Err(web_err),
                        Err(web_err) if web_err.message.starts_with("460:") => return Err(web_err),
                        Err(_) => return Err(err),
                    }
                }
                Err(err) => {
                    log::warn!("player url query failed for {id_num} at {level}: {err}");
                    last_err = Some(err);
                    continue;
                }
            }
        }
        if let Some(err) = last_err {
            return Err(err);
        }
        Ok(None)
    }

    /// Invalidates any cached audio stream URL for `id_or_uri`.
    pub fn invalidate_stream_cache(&self, id_or_uri: &str) {
        let raw_id = id_or_uri.strip_prefix("netease:track:").unwrap_or(id_or_uri);
        if let Ok(id_num) = raw_id.parse::<u64>() {
            self.stream_cache.lock().unwrap().remove(&id_num);
        }
    }

    /// Downloads raw audio bytes from a CDN stream URL without sending account cookies.
    pub fn download_bytes(&self, stream_url: &str, max_bytes: u64) -> Result<Vec<u8>, EngineError> {
        let resp = self
            .download_agent
            .get(stream_url)
            .header("User-Agent", UA)
            .header("Referer", format!("{SITE}/"))
            .call()
            .map_err(map_ureq_error)?;
        let status = resp.status();
        if !status.is_success() {
            return Err(EngineError::new(
                ErrorCode::NotFound,
                format!("音频下载失败（HTTP {}）", status.as_u16()),
            ));
        }
        resp.into_body()
            .into_with_config()
            .limit(max_bytes)
            .read_to_vec()
            .map_err(|e| EngineError::new(ErrorCode::Shape, format!("读取音频数据失败: {e}")))
    }

    /// Fetches audio stream URL (MP3/FLAC) for a track.
    pub fn stream_url(&self, id_or_uri: &str) -> Result<String, EngineError> {
        let raw_id = id_or_uri.strip_prefix("netease:track:").unwrap_or(id_or_uri);
        let id_num = raw_id.parse::<u64>().map_err(|_| {
            EngineError::new(ErrorCode::NotFound, format!("无效的歌曲 ID: {raw_id}"))
        })?;

        // 1. Check in-memory stream cache
        {
            let mut cache = self.stream_cache.lock().unwrap();
            if let Some((url, at)) = cache.get(&id_num) {
                if at.elapsed() < STREAM_TTL {
                    return Ok(url.clone());
                }
            }
            cache.retain(|_, (_, at)| at.elapsed() < STREAM_TTL);
        }

        // 2. Fetch with automatic 460 retry
        let mut final_url = None;
        for attempt in 0..2 {
            match self.fetch_stream_url_once(id_num) {
                Ok(Some(url)) => {
                    final_url = Some(url);
                    break;
                }
                Ok(None) => break,
                Err(err) if err.message.starts_with("460:") => {
                    if attempt == 1 {
                        return Err(EngineError::new(
                            ErrorCode::Internal,
                            "网易云仍拒绝播放请求（460），请稍后再试，并在官方客户端确认该歌曲可正常播放",
                        ));
                    }
                    log::warn!("NetEase returned risk control 460; delaying 1000ms for NMTID renewal");
                    std::thread::sleep(Duration::from_millis(1000));
                }
                Err(err) => return Err(err),
            }
        }

        let Some(url) = final_url else {
            let reason = if !self.is_signed_in() {
                "该歌曲需要登录或开通 VIP 才能播放，请先扫码登录"
            } else {
                "该歌曲暂无可用播放地址或需要黑胶 VIP"
            };
            return Err(EngineError::new(ErrorCode::NotFound, reason));
        };

        // Cache the resolved URL
        {
            let mut cache = self.stream_cache.lock().unwrap();
            cache.insert(id_num, (url.clone(), Instant::now()));
        }

        Ok(url)
    }

    /// Adds or removes a track from Liked Songs.
    pub fn set_liked(&self, id_or_uri: &str, like: bool) -> Result<(), EngineError> {
        if !self.is_signed_in() {
            return Err(EngineError::new(ErrorCode::SignedOut, "请先扫码登录网易云音乐"));
        }
        let raw_id = valid_positive_id(id_or_uri.strip_prefix("netease:track:").unwrap_or(id_or_uri))?;
        let mut params = HashMap::new();
        params.insert("alg", json!("itembased"));
        params.insert("trackId", json!(raw_id));
        params.insert("like", json!(like.to_string()));
        params.insert("time", json!("3"));

        self.request("/api/radio/like", &params, true)?;
        *self.liked_cache.lock().unwrap() = None;
        self.collection_cache
            .lock()
            .unwrap()
            .remove("netease:collection:tracks");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ureq::http::{HeaderMap, HeaderValue, header::SET_COOKIE};

    #[test]
    fn qr_confirmation_requires_credentials_and_accepts_multiple_cookie_headers() {
        let data = std::env::temp_dir().join(format!("nsh-music-netease-qr-{}", std::process::id()));
        fs::create_dir(&data).unwrap();
        let client = NetEaseClient::new(&data);
        let confirmed = json!({ "code": 803, "profile": { "userId": "42", "nickname": "Fixture" } });
        let mut headers = HeaderMap::new();
        headers.append(SET_COOKIE, HeaderValue::from_static("NMTID=device; Path=/"));
        client.accept_response_cookies(&headers);

        assert_eq!(client.finish_qr_login(&confirmed).unwrap_err().code, ErrorCode::SignedOut);
        assert!(client.profile().is_none());

        headers.append(SET_COOKIE, HeaderValue::from_static("MUSIC_U=fixture; Path=/"));
        headers.append(SET_COOKIE, HeaderValue::from_static("__csrf=fixture; Path=/"));
        client.accept_response_cookies(&headers);
        assert_eq!(client.finish_qr_login(&confirmed), Ok(QrPollResult::Success));
        let restored = NetEaseClient::new(&data);
        assert!(restored.is_signed_in());
        assert_eq!(restored.profile().unwrap().id, "42");

        client.clear_session();
        fs::remove_dir(data).unwrap();
    }

    #[test]
    fn set_cookie_handles_revocation_expiry_and_max_age_precedence() {
        let data = std::env::temp_dir().join(format!("nsh-music-netease-cookie-{}", std::process::id()));
        let _ = fs::remove_dir_all(&data);
        fs::create_dir_all(&data).unwrap();

        for revoke in [
            "MUSIC_U=deleted; Max-Age=0; Path=/",
            "MUSIC_U=deleted; Max-Age=-1; Path=/",
            "MUSIC_U=; Path=/",
            "MUSIC_U=deleted; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Path=/",
            "MUSIC_U=still-set; Expires=Thursday, 01-Jan-70 00:00:00 GMT; Path=/",
        ] {
            let client = NetEaseClient::new(&data);
            client.save_cookie_str("MUSIC_U=synthetic-old-session; __csrf=abc");
            *client.profile.lock().unwrap() = Some(NetEaseProfile {
                id: "8".into(),
                name: "Fixture".into(),
                avatar_url: String::new(),
            });
            client.persist_session();
            assert!(client.is_signed_in());

            let mut headers = HeaderMap::new();
            headers.append(SET_COOKIE, HeaderValue::from_static(revoke));
            let res = client.accept_response_cookies_into(&headers, &client.cookies, true);
            assert_eq!(res.unwrap_err().code, ErrorCode::SignedOut);
            assert!(!client.is_signed_in());
            assert!(!data.join(crate::music::paths::SESSION_FILE).exists());
        }

        // Positive Max-Age overrides past Expires and persists the renewed cookie.
        let client = NetEaseClient::new(&data);
        client.save_cookie_str("MUSIC_U=synthetic-old");
        let mut headers = HeaderMap::new();
        headers.append(
            SET_COOKIE,
            HeaderValue::from_static("MUSIC_U=synthetic-renewed; Max-Age=3600; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Path=/"),
        );
        client.accept_response_cookies_into(&headers, &client.cookies, true).unwrap();
        let restored = NetEaseClient::new(&data);
        assert_eq!(
            restored.cookies.lock().unwrap().get("MUSIC_U").map(String::as_str),
            Some("synthetic-renewed")
        );

        client.clear_session();
        let _ = fs::remove_dir_all(&data);
    }

    #[test]
    fn stream_cache_hits_and_clears_with_session() {
        let data = std::env::temp_dir().join(format!("nsh-music-netease-cache-{}", std::process::id()));
        fs::create_dir_all(&data).unwrap();
        let client = NetEaseClient::new(&data);

        // Prepopulate stream cache
        let fake_url = "https://m701.music.126.net/fixture.mp3".to_string();
        client.stream_cache.lock().unwrap().insert(123456, (fake_url.clone(), Instant::now()));

        // Stream url hits cache directly without hitting network
        assert_eq!(client.stream_url("netease:track:123456").unwrap(), fake_url);

        // Invalidating single track removes only that entry
        client.invalidate_stream_cache("netease:track:123456");
        assert!(client.stream_cache.lock().unwrap().is_empty());

        // Clearing session clears the stream cache
        client.stream_cache.lock().unwrap().insert(123456, (fake_url, Instant::now()));
        client.clear_session();
        assert!(client.stream_cache.lock().unwrap().is_empty());

        let _ = fs::remove_dir_all(data);
    }

    #[test]
    fn player_url_response_distinguishes_vip_restriction_fallback_and_risk_control() {
        let trial_res = json!({
            "code": 200,
            "data": [{
                "id": 1001,
                "code": 200,
                "url": "http://m701.music.126.net/trial.mp3",
                "freeTrialInfo": { "start": 0, "end": 30 }
            }]
        });
        assert_eq!(parse_player_url_response(&trial_res, 1001), Ok(StreamLookup::Unavailable));

        let no_copyright_res = json!({
            "code": 200,
            "data": [{ "id": 1001, "code": 404, "url": null }]
        });
        assert_eq!(parse_player_url_response(&no_copyright_res, 1001), Ok(StreamLookup::Unavailable));

        let empty_tier_res = json!({
            "code": 200,
            "data": [{ "id": 1001, "code": 200, "url": "" }]
        });
        assert_eq!(parse_player_url_response(&empty_tier_res, 1001), Ok(StreamLookup::TryFallback));

        let unexpected_host_res = json!({
            "code": 200,
            "data": [{ "id": 1001, "code": 200, "url": "https://evil.example/music.126.net/a.mp3" }]
        });
        assert_eq!(parse_player_url_response(&unexpected_host_res, 1001), Ok(StreamLookup::TryFallback));

        let unsupported_type_res = json!({
            "code": 200,
            "data": [{ "id": 1001, "code": 200, "url": "https://m701.music.126.net/a.ncm", "type": "ncm" }]
        });
        assert_eq!(parse_player_url_response(&unsupported_type_res, 1001), Ok(StreamLookup::TryFallback));

        let risk_res = json!({
            "code": 200,
            "data": [{ "id": 1001, "code": -460, "url": null }]
        });
        assert_eq!(
            parse_player_url_response(&risk_res, 1001),
            Err(EngineError::new(ErrorCode::Internal, "460:-460"))
        );

        let ok_res = json!({
            "code": 200,
            "data": [{
                "id": "1001",
                "code": 200,
                "url": "http://m701.music.126.net/full.mp3",
                "freeTrialInfo": null,
                "type": "mp3"
            }]
        });
        assert_eq!(
            parse_player_url_response(&ok_res, 1001),
            Ok(StreamLookup::Url("https://m701.music.126.net/full.mp3".to_string()))
        );
    }

    #[test]
    fn artwork_url_preserves_query_and_only_rewrites_cdn_hosts() {
        assert_eq!(
            artwork_url(Some("http://p1.music.126.net/cover.jpg")),
            "https://p1.music.126.net/cover.jpg?param=300y300"
        );
        assert_eq!(
            artwork_url(Some("https://p12.music.126.net/cover.jpg?quality=90&param=800y800#cover")),
            "https://p12.music.126.net/cover.jpg?quality=90&param=300y300#cover"
        );
        assert_eq!(
            artwork_url(Some("https://images.example/cover.jpg?param=200y200")),
            "https://images.example/cover.jpg?param=200y200"
        );
        assert_eq!(
            artwork_url(Some("https://p1.music.126.net:8443/cover.jpg")),
            "https://p1.music.126.net:8443/cover.jpg"
        );
    }

    #[test]
    fn lyrics_parser_handles_multi_timestamps_colon_ms_and_plain_text() {
        let multi = parse_lyrics("[ti:Test]\n[00:10.00][01:02:50]Chorus\n[00:05.50]Verse");
        assert!(multi.synced);
        assert_eq!(multi.lines.len(), 3);
        assert_eq!(multi.lines[0].start_ms, 5500.0);
        assert_eq!(multi.lines[1].start_ms, 10_000.0);
        assert_eq!(multi.lines[1].words, "Chorus");
        assert_eq!(multi.lines[2].start_ms, 62_500.0);
        assert_eq!(multi.lines[2].words, "Chorus");

        let plain = parse_lyrics("First line\nSecond line");
        assert!(!plain.synced);
        assert_eq!(plain.lines.len(), 2);
        assert_eq!(plain.lines[0].words, "First line");
    }

    #[test]
    fn track_from_json_avoids_zero_album_uri_and_uses_embedded_privilege() {
        let raw = json!({
            "name": "Local",
            "ar": [{ "name": "Artist" }],
            "al": { "id": 0, "name": "" },
            "dt": 120000,
            "fee": 1,
            "privilege": { "st": 0, "pl": 320000 }
        });
        let track = track_from_json("99", &raw, None, false);
        assert!(track.album_uri.is_empty());
        assert!(track.playable);
    }

    #[test]
    fn parse_playlist_detail_keeps_all_track_ids_beyond_1000_in_order() {
        let track_ids: Vec<Value> = (1..=1250).rev().map(|id| json!({ "id": id })).collect();
        let playlist = json!({
            "name": "我喜欢的音乐",
            "creator": { "nickname": "Tester" },
            "coverImgUrl": "http://p1.music.126.net/liked.jpg",
            "trackCount": 1250,
            "tracks": [],
            "trackIds": track_ids,
        });
        let parsed = parse_playlist_detail(&playlist);
        assert_eq!(parsed.track_ids.len(), 1250);
        assert_eq!(parsed.track_ids.first().map(String::as_str), Some("1250"));
        assert_eq!(parsed.track_ids.last().map(String::as_str), Some("1"));
        assert_eq!(parsed.track_count, Some(1250.0));
    }

    #[test]
    fn build_tracks_preserves_order_placeholders_and_unique_duplicate_uids() {
        let ids = vec!["10".to_string(), "20".to_string(), "30".to_string(), "10".to_string()];
        let mut songs_by_id = HashMap::new();
        songs_by_id.insert(
            "10".to_string(),
            json!({ "id": 10, "name": "Newest", "ar": [{ "name": "A" }], "al": { "id": 1, "name": "Al" }, "dt": 1000 }),
        );
        let mut privs_by_id = HashMap::new();
        privs_by_id.insert("10".to_string(), (0, 320000));
        let originals = vec![json!({ "id": 20, "name": "Fallback", "ar": [{ "name": "B" }], "al": { "id": 2, "name": "Al2" }, "dt": 2000 })];

        let tracks = build_tracks_from_details(&ids, &songs_by_id, &privs_by_id, &originals);
        assert_eq!(tracks.len(), 4);
        assert_eq!(tracks[0].name, "Newest");
        assert_eq!(tracks[0].uid, "10");
        assert!(tracks[0].playable);
        assert_eq!(tracks[1].name, "Fallback");
        assert!(!tracks[1].playable);
        assert_eq!(tracks[2].name, "歌曲暂不可用");
        assert!(!tracks[2].playable);
        assert_eq!(tracks[3].name, "Newest");
        assert_eq!(tracks[3].uid, "10#3");
    }

    #[test]
    fn own_liked_playlist_check_requires_special_type_5_and_owner() {
        let own = json!({ "id": 100, "specialType": 5, "creator": { "userId": 42 } });
        let other = json!({ "id": 101, "specialType": 5, "creator": { "userId": 99 } });
        let normal = json!({ "id": 102, "specialType": 0, "creator": { "userId": 42 } });

        assert!(is_own_liked_playlist(&own, "42"));
        assert!(!is_own_liked_playlist(&other, "42"));
        assert!(!is_own_liked_playlist(&normal, "42"));
    }
}
