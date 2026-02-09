use reqwest::Client;
use serde::Deserialize;

use super::{TorrentInfo, TrackerInfo};

pub struct QBittorrent {
    base_url: String,
    client: Client,
}

#[derive(Deserialize)]
struct QBTorrent {
    hash: String,
    name: String,
}

#[derive(Deserialize)]
struct QBTracker {
    url: String,
    status: i32,
}

impl QBittorrent {
    pub async fn new(
        host: &str,
        port: u16,
        username: &str,
        password: &str,
        use_https: bool,
    ) -> Result<Self, String> {
        let scheme = if use_https { "https" } else { "http" };
        let base_url = format!("{}://{}:{}", scheme, host, port);
        let client = Client::builder()
            .cookie_store(true)
            .danger_accept_invalid_certs(use_https)
            .timeout(std::time::Duration::from_secs(10))
            .connect_timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(|e| e.to_string())?;

        let resp = client
            .post(format!("{}/api/v2/auth/login", base_url))
            .form(&[("username", username), ("password", password)])
            .send()
            .await
            .map_err(|e| format!("Connection failed: {}", e))?;

        let text = resp.text().await.map_err(|e| e.to_string())?;
        if text.trim() != "Ok." {
            return Err(format!("Login failed: {}", text));
        }

        Ok(Self { base_url, client })
    }
}

impl QBittorrent {
    pub async fn test_connection(&self) -> Result<String, String> {
        let resp = self
            .client
            .get(format!("{}/api/v2/app/version", self.base_url))
            .send()
            .await
            .map_err(|e| format!("Connection failed: {}", e))?;
        let version = resp.text().await.map_err(|e| e.to_string())?;
        Ok(format!("qBittorrent {}", version))
    }

    pub async fn list_torrents(&self) -> Result<Vec<TorrentInfo>, String> {
        let torrents: Vec<QBTorrent> = self
            .client
            .get(format!("{}/api/v2/torrents/info", self.base_url))
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json()
            .await
            .map_err(|e| e.to_string())?;

        let mut result = Vec::new();
        for t in torrents {
            let trackers: Vec<QBTracker> = self
                .client
                .get(format!("{}/api/v2/torrents/trackers", self.base_url))
                .query(&[("hash", &t.hash)])
                .send()
                .await
                .map_err(|e| e.to_string())?
                .json()
                .await
                .map_err(|e| e.to_string())?;

            let tracker_infos: Vec<TrackerInfo> = trackers
                .into_iter()
                .filter(|tr| tr.url.starts_with("http") || tr.url.starts_with("udp"))
                .map(|tr| TrackerInfo {
                    url: tr.url,
                    status: tr.status,
                })
                .collect();

            result.push(TorrentInfo {
                hash: t.hash,
                name: t.name,
                trackers: tracker_infos,
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
        let resp = self
            .client
            .post(format!("{}/api/v2/torrents/editTracker", self.base_url))
            .form(&[("hash", hash), ("origUrl", old_url), ("newUrl", new_url)])
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Edit tracker failed: {}", text));
        }

        Ok(())
    }
}
