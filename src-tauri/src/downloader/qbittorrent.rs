use qbit_rs::model::{Credential, GetTorrentListArg};
use qbit_rs::Qbit;
use reqwest::Client;
use url::Url;

use super::{TorrentInfo, TrackerInfo};

pub struct QBittorrent {
    api: Qbit,
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
        log::debug!("qBittorrent base URL: {}", base_url);

        let client = Client::builder()
            .cookie_store(true)
            .danger_accept_invalid_certs(use_https)
            .timeout(std::time::Duration::from_secs(10))
            .connect_timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(|e| {
                log::error!("Failed to build HTTP client: {}", e);
                e.to_string()
            })?;

        let credential = Credential::new(username, password);
        let api = Qbit::new_with_client(base_url.as_str(), credential, client);

        api.login(false).await.map_err(|e| {
            log::error!("qBittorrent login failed: {}", e);
            format!("Login failed: {}", e)
        })?;

        log::info!("qBittorrent auth successful");
        Ok(Self { api })
    }
}

impl QBittorrent {
    pub async fn test_connection(&self) -> Result<String, String> {
        let version = self.api.get_version().await.map_err(|e| {
            log::error!("Failed to get qBittorrent version: {}", e);
            format!("Connection failed: {}", e)
        })?;
        log::info!("qBittorrent version: {}", version);
        Ok(format!("qBittorrent {}", version))
    }

    pub async fn list_torrents(&self) -> Result<Vec<TorrentInfo>, String> {
        let torrents = self
            .api
            .get_torrent_list(GetTorrentListArg::default())
            .await
            .map_err(|e| e.to_string())?;

        log::debug!("qBittorrent returned {} torrents", torrents.len());

        let mut result = Vec::new();
        for t in torrents {
            let hash = match &t.hash {
                Some(h) => h.clone(),
                None => {
                    log::warn!("Skipping torrent with no hash");
                    continue;
                }
            };
            let name = t.name.clone().unwrap_or_default();

            let trackers = self
                .api
                .get_torrent_trackers(&hash)
                .await
                .map_err(|e| e.to_string())?;

            let tracker_infos: Vec<TrackerInfo> = trackers
                .into_iter()
                .filter(|tr| tr.url.starts_with("http") || tr.url.starts_with("udp"))
                .map(|tr| TrackerInfo {
                    url: tr.url,
                    status: tr.status as i32,
                })
                .collect();

            result.push(TorrentInfo {
                hash,
                name,
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
        log::debug!("Replacing tracker for hash={}", hash);

        let orig_url: Url = old_url.parse().map_err(|e| {
            log::error!("Invalid original tracker URL '{}': {}", old_url, e);
            format!("Invalid original URL: {}", e)
        })?;
        let new_url: Url = new_url.parse().map_err(|e| {
            log::error!("Invalid new tracker URL '{}': {}", new_url, e);
            format!("Invalid new URL: {}", e)
        })?;

        self.api
            .edit_trackers(hash, orig_url, new_url)
            .await
            .map_err(|e| {
                log::error!("Edit tracker failed for hash={}: {}", hash, e);
                e.to_string()
            })?;

        Ok(())
    }
}
