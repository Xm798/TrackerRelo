pub mod qbittorrent;
pub mod transmission;

use serde::{Deserialize, Serialize};

use self::qbittorrent::QBittorrent;
use self::transmission::Transmission;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentInfo {
    pub hash: String,
    pub name: String,
    pub trackers: Vec<TrackerInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackerInfo {
    pub url: String,
    pub status: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaceResult {
    pub torrent_name: String,
    pub old_url: String,
    pub new_url: String,
    pub success: bool,
    pub error: Option<String>,
}

pub enum DownloaderKind {
    QBittorrent(QBittorrent),
    Transmission(Transmission),
}

impl DownloaderKind {
    pub async fn test_connection(&self) -> Result<String, String> {
        match self {
            Self::QBittorrent(d) => d.test_connection().await,
            Self::Transmission(d) => d.test_connection().await,
        }
    }

    pub async fn list_torrents(&self) -> Result<Vec<TorrentInfo>, String> {
        match self {
            Self::QBittorrent(d) => d.list_torrents().await,
            Self::Transmission(d) => d.list_torrents().await,
        }
    }

    pub async fn replace_tracker(
        &self,
        hash: &str,
        old_url: &str,
        new_url: &str,
    ) -> Result<(), String> {
        match self {
            Self::QBittorrent(d) => d.replace_tracker(hash, old_url, new_url).await,
            Self::Transmission(d) => d.replace_tracker(hash, old_url, new_url).await,
        }
    }
}
