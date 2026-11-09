//! NetEase Cloud Music HTTP API client, EAPI requests, session storage, and QR code login.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::crypto::eapi_encrypt;
use crate::spotify::model::{
    Collection, ErrorCode, EngineError, Header, Home, Item, Library, LyricLine, Lyrics, Me,
    SearchResults, Section, Track,
};

const SITE: &str = "https://music.163.com";
const EAPI: &str = "https://interfacepc.music.163.com";
const UA: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15";
const EAPI_UA: &str = "NeteaseMusic 9.0.90/5038 (iPhone; iOS 16.2; zh_CN)";

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

impl NetEaseClient {
    /// Creates a client and loads existing session cookies from disk if present.
    pub fn new(data_dir: &Path) -> Self {
        let session_file = data_dir.join("netease-session.json");
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
        }
    }

    /// Whether an active login session cookie (`MUSIC_U`) exists.
    pub fn is_signed_in(&self) -> bool {
        let lock = self.cookies.lock().unwrap();
        lock.get("MUSIC_U").is_some_and(|v| !v.is_empty())
    }

    /// The signed-in user's profile, if available.
    pub fn profile(&self) -> Option<NetEaseProfile> {
        self.profile.lock().unwrap().clone()
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
            let _ = fs::write(&self.session_file, bytes);
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

    fn accept_response_cookies(&self, resp: &ureq::http::Response<ureq::Body>) {
        for (header_name, header_value) in resp.headers() {
            if header_name.as_str().eq_ignore_ascii_case("set-cookie") {
                if let Ok(cookie_str) = header_value.to_str() {
                    self.save_cookie_str(cookie_str);
                }
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
        let cookies_snapshot = self.cookies.lock().unwrap().clone();

        let resp = if is_eapi {
            let now_m = now_millis();
            let now_s = now_secs();
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
            header_obj.insert("requestId".into(), json!(format!("{}_{:04}", now_m, now_m % 10000)));
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
                        percent_encoding::utf8_percent_encode(k.as_str(), percent_encoding::NON_ALPHANUMERIC),
                        percent_encoding::utf8_percent_encode(s, percent_encoding::NON_ALPHANUMERIC)
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

        self.accept_response_cookies(&resp);

        let body_bytes = resp
            .into_body()
            .read_to_vec()
            .map_err(|e| EngineError::new(ErrorCode::Shape, format!("failed to read response: {e}")))?;

        let json_body: Value = serde_json::from_slice(&body_bytes)
            .map_err(|e| EngineError::new(ErrorCode::Shape, format!("invalid JSON from NetEase: {e}")))?;

        let code = json_body.get("code").and_then(|v| v.as_i64()).unwrap_or(200);
        if code == 301 || code == 302 || code == 401 {
            self.clear_session();
            return Err(EngineError::new(ErrorCode::SignedOut, "网易云登录已失效，请重新登录"));
        }

        Ok(json_body)
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
            803 => {
                // NetEase returns session credentials in the `cookie` field of the 803 JSON response
                if let Some(cookie_str) = res.get("cookie").and_then(|v| v.as_str()) {
                    self.save_cookie_str(cookie_str);
                } else if let Some(cookie_str) = res.get("cookies").and_then(|v| v.as_str()) {
                    self.save_cookie_str(cookie_str);
                }
                // If profile is already included in 803 response, save it directly
                if let Some(profile_obj) = res.get("profile").and_then(|v| v.as_object()) {
                    let id = profile_obj.get("userId").map(|v| v.to_string()).unwrap_or_default();
                    let name = profile_obj
                        .get("nickname")
                        .and_then(|v| v.as_str())
                        .unwrap_or("NetEase User")
                        .to_string();
                    let avatar_url = artwork_url(profile_obj.get("avatarUrl").and_then(|v| v.as_str()));
                    let p = NetEaseProfile { id, name, avatar_url };
                    let mut lock = self.profile.lock().unwrap();
                    *lock = Some(p);
                    drop(lock);
                    self.persist_session();
                } else {
                    let _ = self.refresh_profile();
                }
                Ok(QrPollResult::Success)
            }
            _ => Ok(QrPollResult::WaitingScan),
        }
    }

    /// Refreshes the user profile from `/api/nuser/account/get`.
    pub fn refresh_profile(&self) -> Result<Option<NetEaseProfile>, EngineError> {
        let res = self.request("/api/nuser/account/get", &HashMap::new(), true)?;
        if let Some(profile_obj) = res.get("profile").and_then(|v| v.as_object()) {
            let id = profile_obj.get("userId").map(|v| v.to_string()).unwrap_or_default();
            let name = profile_obj
                .get("nickname")
                .and_then(|v| v.as_str())
                .unwrap_or("NetEase User")
                .to_string();
            let avatar_url = artwork_url(profile_obj.get("avatarUrl").and_then(|v| v.as_str()));
            let p = NetEaseProfile { id, name, avatar_url };
            let mut lock = self.profile.lock().unwrap();
            *lock = Some(p.clone());
            drop(lock);
            self.persist_session();
            return Ok(Some(p));
        }
        Ok(None)
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
                        format!("spotify:album:{album_id}")
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
                        uri: format!("spotify:track:{id_str}"),
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
                uri: "spotify:playlist:daily".to_string(),
                kind: "playlist".to_string(),
                name: "每日推荐".to_string(),
                subtitle: "每天 6:00 更新 · 专属推荐".to_string(),
                image: "https://p1.music.126.net/19b5c3bfd5cb8d2db82c5f1566a7b3db/109951163351984240.jpg?param=300y300".to_string(),
            });
            items.push(Item {
                uri: "spotify:collection:tracks".to_string(),
                kind: "collection".to_string(),
                name: "我喜欢的音乐".to_string(),
                subtitle: "红心歌单".to_string(),
                image: "https://p1.music.126.net/vC_Hn86qfS-L_eS1y9k81g==/109951167878895057.jpg?param=300y300".to_string(),
            });
            items.push(Item {
                uri: "spotify:playlist:3136952023".to_string(),
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
                            uri: format!("spotify:playlist:{id}"),
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
                            uri: format!("spotify:playlist:{id}"),
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
                            uri: format!("spotify:artist:{id}"),
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
                            uri: format!("spotify:album:{id}"),
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
                            uri: format!("spotify:playlist:{id}"),
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
        let uri = if id_or_uri.starts_with("spotify:") {
            id_or_uri.to_string()
        } else {
            format!("spotify:playlist:{id_or_uri}")
        };

        if uri == "spotify:playlist:daily" {
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
                    uri: "spotify:playlist:daily".to_string(),
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

        let raw_id = uri.strip_prefix("spotify:playlist:").unwrap_or(&uri);
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
        let raw_id = id_or_uri.strip_prefix("spotify:album:").unwrap_or(id_or_uri);
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
                uri: format!("spotify:album:{raw_id}"),
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
        let raw_id = id_or_uri.strip_prefix("spotify:artist:").unwrap_or(id_or_uri);
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
                            uri: format!("spotify:album:{id}"),
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
                uri: format!("spotify:artist:{raw_id}"),
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
                uri: "spotify:collection:tracks".to_string(),
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
        let mut items = Vec::new();

        if let Some(profile) = self.profile() {
            // 1. Liked Songs
            items.push(Item {
                uri: "spotify:collection:tracks".to_string(),
                kind: "collection".to_string(),
                name: "我喜欢的音乐".to_string(),
                subtitle: "心动精选".to_string(),
                image: "https://p1.music.126.net/vC_Hn86qfS-L_eS1y9k81g==/109951167878895057.jpg?param=300y300".to_string(),
            });

            // 2. Daily Recommend
            items.push(Item {
                uri: "spotify:playlist:daily".to_string(),
                kind: "playlist".to_string(),
                name: "每日推荐".to_string(),
                subtitle: "每天更新".to_string(),
                image: "https://p1.music.126.net/19b5c3bfd5cb8d2db82c5f1566a7b3db/109951163351984240.jpg?param=300y300".to_string(),
            });

            // 3. User's playlists
            let mut params = HashMap::new();
            params.insert("uid", json!(profile.id));
            params.insert("limit", json!(100));
            if let Ok(res) = self.request("/api/user/playlist", &params, false) {
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
                                uri: format!("spotify:playlist:{id}"),
                                kind: "playlist".to_string(),
                                name,
                                subtitle: format!("{count} 首 · by {owner}"),
                                image,
                            });
                        }
                    }
                }
            }
        }

        Ok(Library { items })
    }

    /// Gets the signed-in user info (`Me`).
    pub fn me(&self) -> Result<Me, EngineError> {
        let profile = self
            .profile()
            .or_else(|| self.refresh_profile().ok().flatten())
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
        let raw_id = id_or_uri.strip_prefix("spotify:track:").unwrap_or(id_or_uri);
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

    /// Fetches audio stream URL (MP3/FLAC) for a track.
    pub fn stream_url(&self, id_or_uri: &str) -> Result<String, EngineError> {
        let raw_id = id_or_uri.strip_prefix("spotify:track:").unwrap_or(id_or_uri);
        let id_num = raw_id.parse::<u64>().map_err(|_| {
            EngineError::new(ErrorCode::NotFound, format!("无效的歌曲 ID: {raw_id}"))
        })?;

        let mut params = HashMap::new();
        params.insert("ids", json!(format!("[{id_num}]")));
        params.insert("level", json!("exhigh"));
        params.insert("encodeType", json!("mp3"));

        let res = self.request("/api/song/enhance/player/url/v1", &params, true)?;
        let audio_obj = res
            .get("data")
            .and_then(|d| d.as_array())
            .and_then(|arr| arr.iter().find(|item| item.get("id").and_then(|v| v.as_u64()) == Some(id_num)));

        let mut url = audio_obj
            .and_then(|a| a.get("url"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // If exhigh returned null/empty, fallback to standard level
        if url.is_empty() {
            let mut fallback_params = HashMap::new();
            fallback_params.insert("ids", json!(format!("[{id_num}]")));
            fallback_params.insert("level", json!("standard"));
            fallback_params.insert("encodeType", json!("mp3"));
            if let Ok(res2) = self.request("/api/song/enhance/player/url/v1", &fallback_params, true) {
                if let Some(audio_obj2) = res2
                    .get("data")
                    .and_then(|d| d.as_array())
                    .and_then(|arr| arr.iter().find(|item| item.get("id").and_then(|v| v.as_u64()) == Some(id_num)))
                {
                    if let Some(u) = audio_obj2.get("url").and_then(|v| v.as_str()) {
                        url = u.to_string();
                    }
                }
            }
        }

        if url.is_empty() {
            let reason = if !self.is_signed_in() {
                "该歌曲需要登录或开通 VIP 才能播放，请先扫码登录"
            } else {
                "该歌曲暂无可用播放地址或需要黑胶 VIP"
            };
            return Err(EngineError::new(ErrorCode::NotFound, reason));
        }

        let secure_url = if url.starts_with("http://") {
            url.replacen("http://", "https://", 1)
        } else {
            url
        };

        Ok(secure_url)
    }

    /// Adds or removes a track from Liked Songs.
    pub fn set_liked(&self, id_or_uri: &str, like: bool) -> Result<(), EngineError> {
        if !self.is_signed_in() {
            return Err(EngineError::new(ErrorCode::SignedOut, "请先扫码登录网易云音乐"));
        }
        let raw_id = id_or_uri.strip_prefix("spotify:track:").unwrap_or(id_or_uri);
        let mut params = HashMap::new();
        params.insert("alg", json!("itembased"));
        params.insert("trackId", json!(raw_id));
        params.insert("like", json!(like.to_string()));
        params.insert("time", json!("3"));

        self.request("/api/radio/like", &params, true)?;
        Ok(())
    }
}
