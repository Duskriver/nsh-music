//! NetEase Cloud Music HTTP API client, EAPI requests, session storage, and QR code login.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
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

/// The NetEase API client.
pub struct NetEaseClient {
    session_file: PathBuf,
    device_id: String,
    cookies: Mutex<HashMap<String, String>>,
    profile: Mutex<Option<NetEaseProfile>>,
    agent: ureq::Agent,
    stream_cache: Mutex<HashMap<u64, (String, Instant)>>,
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

fn artwork_url(raw: Option<&str>) -> String {
    let Some(url) = raw else { return String::new() };
    if url.is_empty() {
        return String::new();
    }
    let secure = if url.starts_with("http://") {
        url.replacen("http://", "https://", 1)
    } else {
        url.to_string()
    };
    if secure.contains("music.126.net") && !secure.contains("?param=") {
        format!("{secure}?param=300y300")
    } else {
        secure
    }
}

fn profile_from_response(response: &Value) -> Option<NetEaseProfile> {
    let profile = response.get("profile")?;
    let user_id = profile.get("userId")?;
    let id = match user_id {
        Value::String(id) if !id.is_empty() => id.clone(),
        Value::Number(id) if id.is_u64() => id.to_string(),
        _ => return None,
    };
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
                device_id = stored.device_id;
                profile = stored.profile;
            }
        }

        if device_id.is_empty() {
            // Generate a 52-character hex device ID
            let mut rand_bytes = [0u8; 26];
            let now = now_millis();
            for i in 0..rand_bytes.len() {
                rand_bytes[i] = ((now >> (i % 8)) ^ (i as u64 * 37)) as u8;
            }
            device_id = rand_bytes.iter().map(|b| format!("{:02X}", b)).collect();
        }

        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(12)))
            .build()
            .into();

        NetEaseClient {
            session_file,
            device_id,
            cookies: Mutex::new(cookies),
            profile: Mutex::new(profile),
            agent,
            stream_cache: Mutex::new(HashMap::new()),
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

    /// Clears the login session and removes the stored session file.
    pub fn clear_session(&self) {
        {
            let mut cookies = self.cookies.lock().unwrap();
            cookies.clear();
        }
        {
            let mut profile = self.profile.lock().unwrap();
            *profile = None;
        }
        {
            let mut cache = self.stream_cache.lock().unwrap();
            cache.clear();
        }
        let _ = fs::remove_file(&self.session_file);
    }

    fn persist_session(&self) {
        let cookies = self.cookies.lock().unwrap().clone();
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
        let mut lock = self.cookies.lock().unwrap();
        let mut changed = false;
        for raw_item in cookie_str.split(|c| c == ';' || c == '\n') {
            let part = raw_item.trim();
            if let Some((k, v)) = part.split_once('=') {
                let k = k.trim();
                let v = v.trim();
                if matches!(k, "MUSIC_U" | "__csrf" | "NMTID" | "MUSIC_A")
                    && !v.is_empty()
                    && v != "deleted"
                    && v != "\"\""
                {
                    lock.insert(k.to_string(), v.to_string());
                    changed = true;
                }
            }
        }
        drop(lock);
        if changed {
            self.persist_session();
        }
    }

    fn accept_response_cookies(&self, headers: &ureq::http::HeaderMap) {
        for value in headers.get_all(ureq::http::header::SET_COOKIE) {
            if let Ok(cookie) = value.to_str() {
                self.save_cookie_str(cookie);
            }
        }
    }

    /// Executes an HTTP request against the NetEase Web or EAPI endpoint.
    pub fn request(
        &self,
        path: &str,
        params: &HashMap<&str, Value>,
        is_eapi: bool,
    ) -> Result<Value, EngineError> {
        let response = self.request_raw(path, params, is_eapi)?;
        let code = response.get("code").and_then(Value::as_i64).unwrap_or(200);
        if matches!(code, 301 | 302 | 401) {
            if self.is_signed_in() {
                // Content can be restricted even while the account is signed in.
                if !matches!(self.refresh_profile(), Ok(None)) {
                    log::warn!("NetEase {path} returned code {code}; keeping the login session");
                    return Err(EngineError::new(ErrorCode::Internal, format!("网易云接口拒绝了请求（{code}）")));
                }
                log::warn!("NetEase cleared the login session after {path} returned code {code} and the account check confirmed sign-out");
            }
            return Err(EngineError::new(ErrorCode::SignedOut, "网易云登录已失效，请重新登录"));
        }
        if code == 460 || code == -460 {
            log::warn!("NetEase {path} returned risk-control code {code}");
            return Err(EngineError::new(ErrorCode::Internal, format!("460:{code}")));
        }
        Ok(response)
    }

    fn request_raw(
        &self,
        path: &str,
        params: &HashMap<&str, Value>,
        is_eapi: bool,
    ) -> Result<Value, EngineError> {
        let mut cookies_snapshot = self.cookies.lock().unwrap().clone();
        if cookies_snapshot.contains_key("MUSIC_U") {
            cookies_snapshot.remove("MUSIC_A");
        }

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
            for (k, v) in &cookies_snapshot {
                header_obj.insert(k.clone(), json!(v));
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

            req.send(body_str.as_bytes())
                .map_err(|e| EngineError::new(ErrorCode::Internal, format!("NetEase EAPI network error: {e}")))?
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
            for (k, v) in &cookies_snapshot {
                if !cookie_header.is_empty() {
                    cookie_header.push_str("; ");
                }
                cookie_header.push_str(&format!("{k}={v}"));
            }

            let mut req = self.agent.get(&url);
            req = req.header("User-Agent", UA);
            req = req.header("Referer", format!("{SITE}/"));
            if !cookie_header.is_empty() {
                req = req.header("Cookie", cookie_header);
            }

            req.call()
                .map_err(|e| EngineError::new(ErrorCode::Internal, format!("NetEase Web API network error: {e}")))?
        };

        self.accept_response_cookies(resp.headers());

        let body_bytes = resp
            .into_body()
            .read_to_vec()
            .map_err(|e| EngineError::new(ErrorCode::Shape, format!("failed to read response: {e}")))?;

        serde_json::from_slice(&body_bytes)
            .map_err(|e| EngineError::new(ErrorCode::Shape, format!("invalid JSON from NetEase: {e}")))
    }

    /// Starts a QR code login flow: returns `(unikey, qr_image_data_uri)`.
    pub fn start_qr_login(&self) -> Result<(String, String), EngineError> {
        let mut params = HashMap::new();
        params.insert("type", json!(3));
        let res = self.request("/api/login/qrcode/unikey", &params, true)?;
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
        let res = self.request("/api/login/qrcode/client/login", &params, true)?;
        let code = res.get("code").and_then(|v| v.as_i64()).unwrap_or(801);

        match code {
            800 => Ok(QrPollResult::Expired),
            801 => Ok(QrPollResult::WaitingScan),
            802 => Ok(QrPollResult::ScannedWaitingConfirm),
            803 => self.finish_qr_login(&res),
            _ => Ok(QrPollResult::WaitingScan),
        }
    }

    fn finish_qr_login(&self, response: &Value) -> Result<QrPollResult, EngineError> {
        for field in ["cookie", "cookies"] {
            if let Some(cookie) = response.get(field).and_then(Value::as_str) {
                self.save_cookie_str(cookie);
            }
        }
        if !self.is_signed_in() {
            return Err(EngineError::new(ErrorCode::SignedOut, "网易云未返回登录凭据，请重新扫码。"));
        }

        if let Some(profile) = profile_from_response(response) {
            *self.profile.lock().unwrap() = Some(profile);
            self.persist_session();
        } else if !matches!(self.refresh_profile(), Ok(Some(_))) {
            self.clear_session();
            return Err(EngineError::new(ErrorCode::SignedOut, "无法验证网易云账号，请重新扫码。"));
        }
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
            web => match self.request_profile(true)? {
                Some(profile) => Some(profile),
                None => web?,
            },
        };
        if let Some(profile) = &profile {
            *self.profile.lock().unwrap() = Some(profile.clone());
            self.persist_session();
        } else {
            self.clear_session();
        }
        Ok(profile)
    }

    fn request_profile(&self, is_eapi: bool) -> Result<Option<NetEaseProfile>, EngineError> {
        let response = self.request_raw("/api/nuser/account/get", &HashMap::new(), is_eapi)?;
        match response.get("code").and_then(Value::as_i64) {
            Some(301 | 302 | 401) => Ok(None),
            Some(200) if response.get("profile") == Some(&Value::Null) => Ok(None),
            Some(200) => profile_from_response(&response).map(Some).ok_or_else(|| {
                EngineError::new(ErrorCode::Shape, "网易云未返回有效的账号资料")
            }),
            code => Err(EngineError::new(ErrorCode::Internal, format!("NetEase account check returned code {code:?}"))),
        }
    }

    /// Fetches song details by batch from `/api/v3/song/detail`.
    pub fn song_details(&self, ids: &[String]) -> Result<Vec<Track>, EngineError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut tracks = Vec::with_capacity(ids.len());

        for chunk in ids.chunks(200) {
            let c_arr: Vec<Value> = chunk
                .iter()
                .filter_map(|id| id.parse::<u64>().ok())
                .map(|num| json!({ "id": num }))
                .collect();
            let mut params = HashMap::new();
            let c_str = serde_json::to_string(&c_arr).unwrap_or_default();
            params.insert("c", json!(c_str));

            let res = self.request("/api/v3/song/detail", &params, true)?;
            let songs = res.get("songs").and_then(|v| v.as_array());
            let privileges = res.get("privileges").and_then(|v| v.as_array());

            let mut priv_map = HashMap::new();
            if let Some(privs) = privileges {
                for p in privs {
                    if let Some(id) = p.get("id").and_then(|v| v.as_i64()) {
                        let st = p.get("st").and_then(|v| v.as_i64()).unwrap_or(0);
                        let pl = p.get("pl").and_then(|v| v.as_i64()).unwrap_or(0);
                        priv_map.insert(id, (st, pl));
                    }
                }
            }

            if let Some(songs) = songs {
                for s in songs {
                    let id_val = s.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                    let id_str = id_val.to_string();
                    let name = s.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();

                    let artists = s
                        .get("ar")
                        .or_else(|| s.get("artists"))
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|a| a.get("name").and_then(|n| n.as_str()))
                                .collect::<Vec<_>>()
                                .join(", ")
                        })
                        .unwrap_or_default();

                    let album_obj = s.get("al").or_else(|| s.get("album"));
                    let album = album_obj
                        .and_then(|a| a.get("name").and_then(|n| n.as_str()))
                        .unwrap_or("")
                        .to_string();
                    let album_id = album_obj
                        .and_then(|a| a.get("id").map(|v| v.to_string()))
                        .unwrap_or_default();
                    let album_uri = if album_id.is_empty() {
                        String::new()
                    } else {
                        format!("netease:album:{album_id}")
                    };
                    let image = artwork_url(album_obj.and_then(|a| a.get("picUrl").and_then(|p| p.as_str())));

                    let duration_ms = s
                        .get("dt")
                        .or_else(|| s.get("duration"))
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);

                    let playable = if let Some(&(st, pl)) = priv_map.get(&id_val) {
                        st >= 0 && pl > 0
                    } else {
                        let fee = s.get("fee").and_then(|v| v.as_i64()).unwrap_or(0);
                        fee != 1 && fee != 4
                    };

                    tracks.push(Track {
                        uri: format!("netease:track:{id_str}"),
                        uid: id_str,
                        name,
                        artists,
                        album,
                        album_uri,
                        image,
                        duration_ms,
                        explicit: false,
                        playable,
                    });
                }
            }
        }

        Ok(tracks)
    }

    /// The home page content (recommended playlists and charts).
    pub fn home(&self) -> Result<Home, EngineError> {
        let profile = self.profile();
        let greeting = if let Some(ref p) = profile {
            format!("发现音乐 · {}", p.name)
        } else {
            "网易云音乐 · 发现音乐".to_string()
        };

        let mut sections = Vec::new();

        // 1. Personalized recommendation section when signed in
        if profile.is_some() {
            let mut items = Vec::new();
            items.push(Item {
                uri: "netease:playlist:daily".to_string(),
                kind: "playlist".to_string(),
                name: "每日推荐".to_string(),
                subtitle: "每天 6:00 更新 · 专属推荐".to_string(),
                image: "https://p1.music.126.net/19b5c3bfd5cb8d2db82c5f1566a7b3db/109951163351984240.jpg?param=300y300".to_string(),
            });
            items.push(Item {
                uri: "netease:collection:tracks".to_string(),
                kind: "collection".to_string(),
                name: "我喜欢的音乐".to_string(),
                subtitle: "红心歌单".to_string(),
                image: "https://p1.music.126.net/vC_Hn86qfS-L_eS1y9k81g==/109951167878895057.jpg?param=300y300".to_string(),
            });
            items.push(Item {
                uri: "netease:playlist:3136952023".to_string(),
                kind: "playlist".to_string(),
                name: "私人雷达".to_string(),
                subtitle: "根据你的听歌偏好生成".to_string(),
                image: "https://p1.music.126.net/H7v9t9o0E4dG1QvP9Hh4rA==/109951168128365287.jpg?param=300y300".to_string(),
            });

            sections.push(Section {
                title: "为你推荐".to_string(),
                items,
            });
        }

        // 2. Recommended playlists
        let mut params = HashMap::new();
        params.insert("limit", json!(24));
        if let Ok(res) = self.request("/api/personalized/playlist", &params, false) {
            if let Some(arr) = res.get("result").and_then(|v| v.as_array()) {
                let items: Vec<Item> = arr
                    .iter()
                    .filter_map(|p| {
                        let id = p.get("id")?.to_string();
                        let name = p.get("name")?.as_str()?.to_string();
                        let image = artwork_url(p.get("picUrl").and_then(|u| u.as_str()));
                        let play_count = p.get("playCount").and_then(|v| v.as_f64()).unwrap_or(0.0);
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
                    .collect();

                if !items.is_empty() {
                    sections.push(Section {
                        title: "推荐歌单".to_string(),
                        items,
                    });
                }
            }
        }

        // 3. Top lists / Charts
        if let Ok(res) = self.request("/api/toplist/detail", &HashMap::new(), false) {
            if let Some(arr) = res.get("list").and_then(|v| v.as_array()) {
                let items: Vec<Item> = arr
                    .iter()
                    .take(12)
                    .filter_map(|p| {
                        let id = p.get("id")?.to_string();
                        let name = p.get("name")?.as_str()?.to_string();
                        let image = artwork_url(p.get("coverImgUrl").and_then(|u| u.as_str()));
                        let subtitle = p
                            .get("updateFrequency")
                            .and_then(|v| v.as_str())
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
                    .collect();

                if !items.is_empty() {
                    sections.push(Section {
                        title: "排行榜".to_string(),
                        items,
                    });
                }
            }
        }

        Ok(Home { greeting, sections })
    }

    /// Searches NetEase for tracks, albums, artists, and playlists.
    pub fn search(&self, query: &str) -> Result<SearchResults, EngineError> {
        let text = query.trim();
        if text.is_empty() {
            return Ok(SearchResults::default());
        }

        // 1. Search tracks (type = 1)
        let mut track_params = HashMap::new();
        track_params.insert("s", json!(text));
        track_params.insert("type", json!(1));
        track_params.insert("limit", json!(30));
        let track_res = self.request("/api/cloudsearch/pc", &track_params, false)?;
        let raw_songs = track_res
            .get("result")
            .and_then(|r| r.get("songs"))
            .and_then(|s| s.as_array())
            .cloned()
            .unwrap_or_default();

        let song_ids: Vec<String> = raw_songs
            .iter()
            .filter_map(|s| s.get("id").map(|v| v.to_string()))
            .collect();
        let tracks = self.song_details(&song_ids)?;

        // 2. Search artists (type = 100)
        let mut artist_params = HashMap::new();
        artist_params.insert("s", json!(text));
        artist_params.insert("type", json!(100));
        artist_params.insert("limit", json!(8));
        let artist_res = self.request("/api/cloudsearch/pc", &artist_params, false)?;
        let artists: Vec<Item> = artist_res
            .get("result")
            .and_then(|r| r.get("artists"))
            .and_then(|s| s.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|a| {
                        let id = a.get("id")?.to_string();
                        let name = a.get("name")?.as_str()?.to_string();
                        let image = artwork_url(a.get("picUrl").or_else(|| a.get("img1v1Url")).and_then(|u| u.as_str()));
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

        // 3. Search albums (type = 10)
        let mut album_params = HashMap::new();
        album_params.insert("s", json!(text));
        album_params.insert("type", json!(10));
        album_params.insert("limit", json!(8));
        let album_res = self.request("/api/cloudsearch/pc", &album_params, false)?;
        let albums: Vec<Item> = album_res
            .get("result")
            .and_then(|r| r.get("albums"))
            .and_then(|s| s.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|a| {
                        let id = a.get("id")?.to_string();
                        let name = a.get("name")?.as_str()?.to_string();
                        let artist_name = a
                            .get("artist")
                            .and_then(|ar| ar.get("name").and_then(|n| n.as_str()))
                            .unwrap_or("未知歌手");
                        let image = artwork_url(a.get("picUrl").and_then(|u| u.as_str()));
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

        // 4. Search playlists (type = 1000)
        let mut playlist_params = HashMap::new();
        playlist_params.insert("s", json!(text));
        playlist_params.insert("type", json!(1000));
        playlist_params.insert("limit", json!(8));
        let playlist_res = self.request("/api/cloudsearch/pc", &playlist_params, false)?;
        let playlists: Vec<Item> = playlist_res
            .get("result")
            .and_then(|r| r.get("playlists"))
            .and_then(|s| s.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|p| {
                        let id = p.get("id")?.to_string();
                        let name = p.get("name")?.as_str()?.to_string();
                        let owner = p
                            .get("creator")
                            .and_then(|c| c.get("nickname").and_then(|n| n.as_str()))
                            .unwrap_or("网易云歌单");
                        let image = artwork_url(p.get("coverImgUrl").and_then(|u| u.as_str()));
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
            if !self.is_signed_in() {
                return Err(EngineError::new(ErrorCode::SignedOut, "请先扫码登录网易云音乐"));
            }
            let res = self.request("/api/v3/discovery/recommend/songs", &HashMap::new(), false)?;
            let songs = res
                .get("data")
                .and_then(|d| d.get("dailySongs"))
                .and_then(|s| s.as_array())
                .cloned()
                .unwrap_or_default();
            let ids: Vec<String> = songs
                .iter()
                .filter_map(|s| s.get("id").map(|v| v.to_string()))
                .collect();
            let tracks = self.song_details(&ids)?;

            return Ok(Collection {
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
            });
        }

        let raw_id = uri.strip_prefix("netease:playlist:").unwrap_or(&uri);
        let mut params = HashMap::new();
        params.insert("id", json!(raw_id));
        params.insert("n", json!(1000));
        let res = self.request("/api/v6/playlist/detail", &params, false)?;
        let p = res
            .get("playlist")
            .ok_or_else(|| EngineError::new(ErrorCode::NotFound, "歌单不存在或无权访问"))?;

        let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("歌单").to_string();
        let subtitle = p
            .get("creator")
            .and_then(|c| c.get("nickname").and_then(|v| v.as_str()))
            .unwrap_or("网易云歌单")
            .to_string();
        let image = artwork_url(p.get("coverImgUrl").and_then(|v| v.as_str()));
        let description = p.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();

        let track_ids: Vec<String> = p
            .get("trackIds")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|t| t.get("id").map(|i| i.to_string())).collect())
            .unwrap_or_default();

        let tracks = self.song_details(&track_ids)?;

        Ok(Collection {
            header: Header {
                uri,
                kind: "playlist".to_string(),
                name,
                subtitle,
                image,
                description,
            },
            total: tracks.len() as f64,
            tracks,
            releases: Vec::new(),
        })
    }

    /// Fetches an album by id.
    pub fn album(&self, id_or_uri: &str) -> Result<Collection, EngineError> {
        let raw_id = id_or_uri.strip_prefix("netease:album:").unwrap_or(id_or_uri);
        let path = format!("/api/v1/album/{raw_id}");
        let res = self.request(&path, &HashMap::new(), false)?;
        let a = res
            .get("album")
            .ok_or_else(|| EngineError::new(ErrorCode::NotFound, "专辑不存在或不可用"))?;

        let name = a.get("name").and_then(|v| v.as_str()).unwrap_or("专辑").to_string();
        let subtitle = a
            .get("artists")
            .and_then(|arr| arr.as_array())
            .map(|artists| {
                artists
                    .iter()
                    .filter_map(|ar| ar.get("name").and_then(|v| v.as_str()))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        let image = artwork_url(a.get("picUrl").and_then(|v| v.as_str()));
        let description = a.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();

        let song_ids: Vec<String> = res
            .get("songs")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|s| s.get("id").map(|v| v.to_string())).collect())
            .unwrap_or_default();

        let tracks = self.song_details(&song_ids)?;

        Ok(Collection {
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
        })
    }

    /// Fetches an artist by id.
    pub fn artist(&self, id_or_uri: &str) -> Result<Collection, EngineError> {
        let raw_id = id_or_uri.strip_prefix("netease:artist:").unwrap_or(id_or_uri);
        let artist_path = format!("/api/v1/artist/{raw_id}");
        let albums_path = format!("/api/artist/albums/{raw_id}");

        let res = self.request(&artist_path, &HashMap::new(), false)?;
        let ar = res
            .get("artist")
            .ok_or_else(|| EngineError::new(ErrorCode::NotFound, "歌手不存在或不可用"))?;

        let name = ar.get("name").and_then(|v| v.as_str()).unwrap_or("歌手").to_string();
        let image = artwork_url(ar.get("picUrl").or_else(|| ar.get("img1v1Url")).and_then(|v| v.as_str()));
        let description = ar.get("briefDesc").and_then(|v| v.as_str()).unwrap_or("").to_string();

        let hot_ids: Vec<String> = res
            .get("hotSongs")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|s| s.get("id").map(|v| v.to_string())).collect())
            .unwrap_or_default();
        let tracks = self.song_details(&hot_ids)?;

        // Fetch albums
        let mut album_params = HashMap::new();
        album_params.insert("limit", json!(30));
        let mut releases = Vec::new();
        if let Ok(alb_res) = self.request(&albums_path, &album_params, false) {
            if let Some(arr) = alb_res.get("hotAlbums").and_then(|v| v.as_array()) {
                releases = arr
                    .iter()
                    .filter_map(|a| {
                        let id = a.get("id")?.to_string();
                        let a_name = a.get("name")?.as_str()?.to_string();
                        let a_img = artwork_url(a.get("picUrl").and_then(|u| u.as_str()));
                        Some(Item {
                            uri: format!("netease:album:{id}"),
                            kind: "album".to_string(),
                            name: a_name,
                            subtitle: "专辑".to_string(),
                            image: a_img,
                        })
                    })
                    .collect();
            }
        }

        Ok(Collection {
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
        })
    }

    /// Fetches the signed-in user's Liked Songs.
    pub fn liked_songs(&self) -> Result<Collection, EngineError> {
        let profile = self.profile().ok_or_else(|| {
            EngineError::new(ErrorCode::SignedOut, "请先扫码登录网易云音乐")
        })?;

        let mut params = HashMap::new();
        params.insert("uid", json!(profile.id));
        let res = self.request("/api/song/like/get", &params, true)?;
        let ids: Vec<String> = res
            .get("ids")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().map(|id| id.to_string()).collect())
            .unwrap_or_default();

        let tracks = self.song_details(&ids.iter().take(300).cloned().collect::<Vec<_>>())?;
        let first_image = tracks.first().map(|t| t.image.clone()).unwrap_or_default();

        Ok(Collection {
            header: Header {
                uri: "netease:collection:tracks".to_string(),
                kind: "collection".to_string(),
                name: "我喜欢的音乐".to_string(),
                subtitle: profile.name,
                image: first_image,
                description: format!("网易云音乐 · 共 {} 首喜欢的音乐", ids.len()),
            },
            total: ids.len() as f64,
            tracks,
            releases: Vec::new(),
        })
    }

    /// Fetches the user's library playlists.
    pub fn library(&self) -> Result<Library, EngineError> {
        let profile = self
            .profile()
            .ok_or_else(|| EngineError::new(ErrorCode::SignedOut, "请先扫码登录网易云音乐"))?;

        let mut items = vec![
            // 1. Liked Songs
            Item {
                uri: "netease:collection:tracks".to_string(),
                kind: "collection".to_string(),
                name: "我喜欢的音乐".to_string(),
                subtitle: "心动精选".to_string(),
                image: "https://p1.music.126.net/vC_Hn86qfS-L_eS1y9k81g==/109951167878895057.jpg?param=300y300".to_string(),
            },
            // 2. Daily Recommend
            Item {
                uri: "netease:playlist:daily".to_string(),
                kind: "playlist".to_string(),
                name: "每日推荐".to_string(),
                subtitle: "每天更新".to_string(),
                image: "https://p1.music.126.net/19b5c3bfd5cb8d2db82c5f1566a7b3db/109951163351984240.jpg?param=300y300".to_string(),
            },
        ];

        // 3. User's playlists
        let mut params = HashMap::new();
        params.insert("uid", json!(profile.id));
        params.insert("limit", json!(100));
        let res = self.request("/api/user/playlist", &params, false)?;
        if let Some(arr) = res.get("playlist").and_then(|v| v.as_array()) {
            for p in arr {
                // Skip the first special "I like" playlist if present
                if p.get("specialType").and_then(|v| v.as_i64()) == Some(5) {
                    continue;
                }
                if let Some(id) = p.get("id").map(|v| v.to_string()) {
                    let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let count = p.get("trackCount").and_then(|v| v.as_i64()).unwrap_or(0);
                    let owner = p
                        .get("creator")
                        .and_then(|c| c.get("nickname").and_then(|v| v.as_str()))
                        .unwrap_or("");
                    let image = artwork_url(p.get("coverImgUrl").and_then(|v| v.as_str()));
                    items.push(Item {
                        uri: format!("netease:playlist:{id}"),
                        kind: "playlist".to_string(),
                        name,
                        subtitle: format!("{count} 首 · by {owner}"),
                        image,
                    });
                }
            }
        }

        Ok(Library { items })
    }

    /// Gets the signed-in user info (`Me`).
    pub fn me(&self) -> Result<Me, EngineError> {
        let profile = self
            .profile()
            .ok_or_else(|| {
                EngineError::new(ErrorCode::SignedOut, "请先扫码登录网易云音乐")
            })?;

        Ok(Me {
            id: profile.id,
            name: profile.name.clone(),
            username: profile.name,
            image: profile.avatar_url,
        })
    }

    /// Fetches lyrics for a track ID.
    pub fn lyrics(&self, id_or_uri: &str) -> Result<Lyrics, EngineError> {
        let raw_id = id_or_uri.strip_prefix("netease:track:").unwrap_or(id_or_uri);
        let mut params = HashMap::new();
        params.insert("id", json!(raw_id));
        params.insert("lv", json!(1));
        params.insert("tv", json!(-1));

        let res = self.request("/api/song/lyric", &params, false)?;
        let mut lrc = res
            .get("lrc")
            .and_then(|l| l.get("lyric"))
            .and_then(|v| v.as_str())
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
                    .and_then(|v| v.as_str())
                {
                    lrc = d_lrc.to_string();
                }
            }
        }

        let mut lines = Vec::new();
        for line in lrc.lines() {
            let line = line.trim();
            if !line.starts_with('[') {
                continue;
            }
            if let Some(close_idx) = line.find(']') {
                let time_str = &line[1..close_idx];
                let words = line[close_idx + 1..].trim();
                if let Some((min_str, sec_str)) = time_str.split_once(':') {
                    if let (Ok(m), Ok(s)) = (min_str.parse::<f64>(), sec_str.parse::<f64>()) {
                        let start_ms = m * 60_000.0 + s * 1000.0;
                        lines.push(LyricLine {
                            start_ms,
                            words: if words.is_empty() { "♪".to_string() } else { words.to_string() },
                        });
                    }
                }
            }
        }

        lines.sort_by(|a, b| a.start_ms.partial_cmp(&b.start_ms).unwrap_or(std::cmp::Ordering::Equal));

        let synced = !lines.is_empty();
        Ok(Lyrics {
            synced,
            lines,
            provider: "网易云音乐".to_string(),
        })
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
        .and_then(|d| d.as_array())
        .and_then(|arr| arr.iter().find(|item| item.get("id").and_then(|v| v.as_u64()) == Some(id_num)));

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

    let raw_url = audio.get("url").and_then(|v| v.as_str()).unwrap_or("");
    if raw_url.is_empty() {
        return Ok(StreamLookup::TryFallback);
    }

    let secure_url = if raw_url.starts_with("http://") {
        raw_url.replacen("http://", "https://", 1)
    } else {
        raw_url.to_string()
    };

    if !secure_url.starts_with("https://") || !secure_url.contains("music.126.net") {
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

        for &(level, encode_type) in tiers {
            match self.query_player_url(id_num, level, encode_type, true) {
                Ok(StreamLookup::Url(url)) => return Ok(Some(url)),
                Ok(StreamLookup::Unavailable) => return Ok(None),
                Ok(StreamLookup::TryFallback) => continue,
                Err(err) if err.message.starts_with("460:") => {
                    // When EAPI hits risk control 460, try Web API endpoint first (which also refreshes cookies).
                    match self.query_player_url(id_num, level, encode_type, false) {
                        Ok(StreamLookup::Url(url)) => return Ok(Some(url)),
                        Ok(StreamLookup::Unavailable) => return Ok(None),
                        Ok(StreamLookup::TryFallback) => return Err(err),
                        Err(web_err) if web_err.message.starts_with("460:") => return Err(web_err),
                        Err(_) => return Err(err),
                    }
                }
                Err(err) => {
                    log::warn!("player url query failed for {id_num} at {level}: {err}");
                    continue;
                }
            }
        }
        Ok(None)
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
                if at.elapsed() < Duration::from_secs(600) {
                    return Ok(url.clone());
                }
            }
            cache.retain(|_, (_, at)| at.elapsed() < Duration::from_secs(600));
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
        let raw_id = id_or_uri.strip_prefix("netease:track:").unwrap_or(id_or_uri);
        let mut params = HashMap::new();
        params.insert("alg", json!("itembased"));
        params.insert("trackId", json!(raw_id));
        params.insert("like", json!(like.to_string()));
        params.insert("time", json!("3"));

        self.request("/api/radio/like", &params, true)?;
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
    fn stream_cache_hits_and_clears_with_session() {
        let data = std::env::temp_dir().join(format!("nsh-music-netease-cache-{}", std::process::id()));
        fs::create_dir_all(&data).unwrap();
        let client = NetEaseClient::new(&data);

        // Prepopulate stream cache
        let fake_url = "https://m701.music.126.net/fixture.mp3".to_string();
        client.stream_cache.lock().unwrap().insert(123456, (fake_url.clone(), Instant::now()));

        // Stream url hits cache directly without hitting network
        assert_eq!(client.stream_url("netease:track:123456").unwrap(), fake_url);

        // Clearing session clears the stream cache
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
                "id": 1001,
                "code": 200,
                "url": "http://m701.music.126.net/full.mp3",
                "freeTrialInfo": null
            }]
        });
        assert_eq!(
            parse_player_url_response(&ok_res, 1001),
            Ok(StreamLookup::Url("https://m701.music.126.net/full.mp3".to_string()))
        );
    }
}
