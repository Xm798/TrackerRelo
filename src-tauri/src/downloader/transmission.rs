use std::collections::HashSet;

use tokio::sync::Mutex;
use transmission_rpc::types::{BasicAuth, Id, TorrentGetField, TorrentSetArgs, TrackerList};
use transmission_rpc::TransClient;

use super::{TorrentInfo, TrackerInfo};

pub struct Transmission {
    client: Mutex<TransClient>,
    rpc_version: i32,
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
        let url_str = format!("{}://{}:{}/transmission/rpc", scheme, host, port);
        log::debug!("Transmission RPC URL: {}", url_str);

        let reqwest_client = reqwest::Client::builder()
            .danger_accept_invalid_certs(use_https)
            .timeout(std::time::Duration::from_secs(10))
            .connect_timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(|e| e.to_string())?;

        let url = url_str.parse::<url::Url>().map_err(|e| e.to_string())?;
        let mut client = TransClient::new_with_client(url, reqwest_client);

        if !username.is_empty() {
            log::debug!("Transmission using Basic auth");
            client.set_auth(BasicAuth {
                user: username.to_string(),
                password: password.to_string(),
            });
        } else {
            log::debug!("Transmission using no auth");
        }

        let resp = client.session_get().await.map_err(|e| e.to_string())?;
        if !resp.is_ok() {
            log::error!("session-get failed: {}", resp.result);
            return Err(format!("session-get failed: {}", resp.result));
        }

        let rpc_version = resp.arguments.rpc_version;
        log::info!("Transmission session established (RPC v{})", rpc_version);

        Ok(Self {
            client: Mutex::new(client),
            rpc_version,
        })
    }

    pub async fn test_connection(&self) -> Result<String, String> {
        let mut client = self.client.lock().await;
        let resp = client.session_get().await.map_err(|e| e.to_string())?;
        if !resp.is_ok() {
            return Err(format!("session-get failed: {}", resp.result));
        }
        let version = resp.arguments.version;
        log::info!("Transmission version: {}", version);
        Ok(format!("Transmission {}", version))
    }

    pub async fn list_torrents(&self) -> Result<Vec<TorrentInfo>, String> {
        let mut client = self.client.lock().await;
        let resp = client
            .torrent_get(
                Some(vec![
                    TorrentGetField::HashString,
                    TorrentGetField::Name,
                    TorrentGetField::Trackers,
                ]),
                None,
            )
            .await
            .map_err(|e| e.to_string())?;

        if !resp.is_ok() {
            return Err(format!("torrent-get failed: {}", resp.result));
        }

        let torrents = resp.arguments.torrents;
        log::debug!("Transmission returned {} torrents", torrents.len());

        let result = torrents
            .into_iter()
            .map(|t| {
                let hash = t.hash_string.unwrap_or_default();
                let name = t.name.unwrap_or_default();
                let trackers = t
                    .trackers
                    .unwrap_or_default()
                    .into_iter()
                    .map(|tr| TrackerInfo {
                        url: tr.announce,
                        status: 0,
                    })
                    .collect();
                TorrentInfo {
                    hash,
                    name,
                    trackers,
                }
            })
            .collect();

        Ok(result)
    }

    pub async fn replace_tracker(
        &self,
        hash: &str,
        old_url: &str,
        new_url: &str,
    ) -> Result<(), String> {
        log::debug!("Replacing tracker for hash={}", hash);

        if self.rpc_version >= 17 {
            self.replace_tracker_list(hash, old_url, new_url).await
        } else {
            self.replace_tracker_legacy(hash, old_url, new_url).await
        }
    }

    async fn replace_tracker_list(
        &self,
        hash: &str,
        old_url: &str,
        new_url: &str,
    ) -> Result<(), String> {
        let ids = Some(vec![Id::Hash(hash.to_string())]);

        let mut client = self.client.lock().await;

        let resp = client
            .torrent_get(Some(vec![TorrentGetField::TrackerList]), ids.clone())
            .await
            .map_err(|e| e.to_string())?;

        if !resp.is_ok() {
            return Err(format!("torrent-get failed: {}", resp.result));
        }

        let torrent = resp.arguments.torrents.first().ok_or("Torrent not found")?;

        let tracker_list = torrent
            .tracker_list
            .as_deref()
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

        let mut args = TorrentSetArgs::default();
        args.tracker_list = Some(TrackerList(vec![new_tracker_list]));

        let resp = client
            .torrent_set(args, ids)
            .await
            .map_err(|e| e.to_string())?;

        if !resp.is_ok() {
            return Err(format!("torrent-set failed: {}", resp.result));
        }

        Ok(())
    }

    async fn replace_tracker_legacy(
        &self,
        hash: &str,
        old_url: &str,
        new_url: &str,
    ) -> Result<(), String> {
        let ids = Some(vec![Id::Hash(hash.to_string())]);

        let mut client = self.client.lock().await;

        let resp = client
            .torrent_get(Some(vec![TorrentGetField::Trackers]), ids.clone())
            .await
            .map_err(|e| e.to_string())?;

        if !resp.is_ok() {
            return Err(format!("torrent-get failed: {}", resp.result));
        }

        let torrent = resp.arguments.torrents.first().ok_or("Torrent not found")?;

        let trackers = torrent.trackers.as_ref().ok_or("No trackers")?;

        let tracker_id = trackers
            .iter()
            .find_map(|tr| {
                if tr.announce == old_url {
                    Some(tr.id)
                } else {
                    None
                }
            })
            .ok_or("Tracker not found in torrent")?;

        log::debug!("Found tracker ID {} for replacement (legacy)", tracker_id);

        let mut args = TorrentSetArgs::default();
        args.tracker_replace = Some(vec![tracker_id.to_string(), new_url.to_string()]);

        let resp = client
            .torrent_set(args, ids)
            .await
            .map_err(|e| e.to_string())?;

        if !resp.is_ok() {
            return Err(format!("torrent-set failed: {}", resp.result));
        }

        Ok(())
    }
}
