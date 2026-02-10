use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::Mutex;

use super::{TorrentInfo, TrackerInfo};

pub struct Transmission {
    url: String,
    client: Client,
    session_id: Mutex<String>,
    auth_header: Option<String>,
    rpc_version: Mutex<i64>,
}

#[derive(Serialize)]
struct RpcRequest {
    method: String,
    arguments: Value,
}

#[derive(Deserialize)]
struct RpcResponse {
    result: String,
    arguments: Option<Value>,
}

impl Transmission {
    pub async fn new(
        host: &str,
        port: u16,
        username: &str,
        password: &str,
        use_https: bool,
    ) -> Result<Self, String> {
        let scheme = if use_https { "https" } else { "http" };
        let url = format!("{}://{}:{}/transmission/rpc", scheme, host, port);
        log::debug!("Transmission RPC URL: {}", url);
        let client = Client::builder()
            .danger_accept_invalid_certs(use_https)
            .timeout(std::time::Duration::from_secs(10))
            .connect_timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(|e| e.to_string())?;

        let auth_header = if !username.is_empty() {
            use base64::Engine;
            let encoded = base64::engine::general_purpose::STANDARD
                .encode(format!("{}:{}", username, password));
            log::debug!("Transmission using Basic auth");
            Some(format!("Basic {}", encoded))
        } else {
            log::debug!("Transmission using no auth");
            None
        };

        let tr = Self {
            url,
            client,
            session_id: Mutex::new(String::new()),
            auth_header,
            rpc_version: Mutex::new(0),
        };

        tr.refresh_session_id().await?;

        // Fetch RPC version to select the right tracker API later
        let args = tr.rpc_call("session-get", json!({})).await?;
        let ver = args
            .get("rpc-version")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        *tr.rpc_version.lock().unwrap() = ver;
        log::info!("Transmission session established (RPC v{})", ver);

        Ok(tr)
    }

    async fn refresh_session_id(&self) -> Result<(), String> {
        log::debug!("Refreshing Transmission session ID");
        let mut req = self
            .client
            .post(&self.url)
            .header("Content-Type", "application/json");
        if let Some(ref auth) = self.auth_header {
            req = req.header("Authorization", auth);
        }

        let resp = req
            .json(&RpcRequest {
                method: "session-get".to_string(),
                arguments: json!({}),
            })
            .send()
            .await;

        match resp {
            Ok(r) if r.status().as_u16() == 409 => {
                if let Some(sid) = r.headers().get("x-transmission-session-id") {
                    *self.session_id.lock().unwrap() = sid.to_str().unwrap_or("").to_string();
                    log::debug!("Session ID obtained from 409 response");
                    Ok(())
                } else {
                    log::error!("No session ID in 409 response");
                    Err("No session ID in 409 response".to_string())
                }
            }
            Ok(r) if r.status().is_success() => {
                if let Some(sid) = r.headers().get("x-transmission-session-id") {
                    *self.session_id.lock().unwrap() = sid.to_str().unwrap_or("").to_string();
                }
                Ok(())
            }
            Ok(r) => {
                log::error!("Unexpected status during session refresh: {}", r.status());
                Err(format!("Unexpected status: {}", r.status()))
            }
            Err(e) => {
                log::error!("Connection failed during session refresh: {}", e);
                Err(format!("Connection failed: {}", e))
            }
        }
    }

    async fn rpc_call(&self, method: &str, arguments: Value) -> Result<Value, String> {
        log::debug!("Transmission RPC call: {}", method);
        let request = RpcRequest {
            method: method.to_string(),
            arguments,
        };

        // Transmission uses HTTP 409 to indicate the session id is missing/expired.
        // When that happens, we update the session id and retry once.
        for attempt in 0..2 {
            let sid = self.session_id.lock().unwrap().clone();
            let mut req = self
                .client
                .post(&self.url)
                .header("Content-Type", "application/json")
                .header("X-Transmission-Session-Id", &sid);

            if let Some(ref auth) = self.auth_header {
                req = req.header("Authorization", auth);
            }

            let resp = req.json(&request).send().await.map_err(|e| e.to_string())?;

            if resp.status().as_u16() == 409 {
                log::debug!("Got 409, refreshing session ID (attempt {})", attempt + 1);
                if let Some(sid) = resp.headers().get("x-transmission-session-id") {
                    *self.session_id.lock().unwrap() = sid.to_str().unwrap_or("").to_string();
                    if attempt == 0 {
                        continue;
                    }
                }
                // Fall back to an explicit refresh for weird responses (or if we already retried).
                self.refresh_session_id().await?;
                if attempt == 0 {
                    continue;
                }
            }

            if !resp.status().is_success() {
                log::error!("RPC '{}' failed with status {}", method, resp.status());
                return Err(format!("Unexpected status: {}", resp.status()));
            }

            let resp: RpcResponse = resp.json().await.map_err(|e| e.to_string())?;

            if resp.result != "success" {
                log::error!("RPC '{}' error: {}", method, resp.result);
                return Err(format!("RPC error: {}", resp.result));
            }

            return Ok(resp.arguments.unwrap_or(json!({})));
        }

        Err("RPC failed after session refresh".to_string())
    }
}

impl Transmission {
    pub async fn test_connection(&self) -> Result<String, String> {
        let args = self.rpc_call("session-get", json!({})).await?;
        let version = args
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        log::info!("Transmission version: {}", version);
        Ok(format!("Transmission {}", version))
    }

    pub async fn list_torrents(&self) -> Result<Vec<TorrentInfo>, String> {
        let args = self
            .rpc_call(
                "torrent-get",
                json!({
                    "fields": ["hashString", "name", "trackers"]
                }),
            )
            .await?;

        let torrents = args
            .get("torrents")
            .and_then(|t| t.as_array())
            .ok_or("No torrents field")?;

        log::debug!("Transmission returned {} torrents", torrents.len());

        let mut result = Vec::new();
        for t in torrents {
            let hash = t
                .get("hashString")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let name = t
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let trackers = t
                .get("trackers")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|tr| {
                            let announce = tr.get("announce")?.as_str()?;
                            Some(TrackerInfo {
                                url: announce.to_string(),
                                status: 0,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();

            result.push(TorrentInfo {
                hash,
                name,
                trackers,
            });
        }

        Ok(result)
    }

    pub async fn replace_tracker(
        &self,
        hash: &str,
        old_url: &str,
        new_url: &str,
    ) -> Result<(), String> {
        log::debug!("Replacing tracker for hash={}", hash);

        let rpc_ver = *self.rpc_version.lock().unwrap();

        if rpc_ver >= 17 {
            // Transmission 4.0+ (RPC v17): use trackerList
            self.replace_tracker_list(hash, old_url, new_url).await
        } else {
            // Transmission < 4.0: use trackerReplace
            self.replace_tracker_legacy(hash, old_url, new_url).await
        }
    }

    async fn replace_tracker_list(
        &self,
        hash: &str,
        old_url: &str,
        new_url: &str,
    ) -> Result<(), String> {
        let args = self
            .rpc_call(
                "torrent-get",
                json!({
                    "ids": [hash],
                    "fields": ["trackerList"]
                }),
            )
            .await?;

        let torrents = args
            .get("torrents")
            .and_then(|t| t.as_array())
            .ok_or("No torrents")?;
        let torrent = torrents.first().ok_or("Torrent not found")?;
        let tracker_list = torrent
            .get("trackerList")
            .and_then(|v| v.as_str())
            .ok_or("No trackerList field")?;

        if !tracker_list.contains(old_url) {
            return Err("Tracker not found in torrent".to_string());
        }

        // Replace then deduplicate to handle cases where new_url already exists
        let replaced = tracker_list.replace(old_url, new_url);
        let mut seen = HashSet::new();
        let new_tracker_list: String = replaced
            .split("\n\n")
            .map(|tier| {
                tier.lines()
                    .filter(|url| !url.is_empty() && seen.insert(url.to_string()))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .filter(|tier| !tier.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");
        log::debug!("Using trackerList approach (RPC v17+)");

        self.rpc_call(
            "torrent-set",
            json!({
                "ids": [hash],
                "trackerList": new_tracker_list
            }),
        )
        .await?;

        Ok(())
    }

    async fn replace_tracker_legacy(
        &self,
        hash: &str,
        old_url: &str,
        new_url: &str,
    ) -> Result<(), String> {
        let args = self
            .rpc_call(
                "torrent-get",
                json!({
                    "ids": [hash],
                    "fields": ["trackers"]
                }),
            )
            .await?;

        let torrents = args
            .get("torrents")
            .and_then(|t| t.as_array())
            .ok_or("No torrents")?;
        let torrent = torrents.first().ok_or("Torrent not found")?;
        let trackers = torrent
            .get("trackers")
            .and_then(|v| v.as_array())
            .ok_or("No trackers")?;

        let tracker_id = trackers
            .iter()
            .find_map(|tr| {
                let announce = tr.get("announce")?.as_str()?;
                if announce == old_url {
                    tr.get("id")?.as_i64()
                } else {
                    None
                }
            })
            .ok_or("Tracker not found in torrent")?;

        log::debug!("Found tracker ID {} for replacement (legacy)", tracker_id);

        self.rpc_call(
            "torrent-set",
            json!({
                "ids": [hash],
                "trackerReplace": [tracker_id, new_url]
            }),
        )
        .await?;

        Ok(())
    }
}
