use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HistoryItem {
    pub id: String,
    pub bvid: String,
    pub title: String,
    pub quality: String,
    pub format: String,
    pub output_path: String,
    pub timestamp: String,
    pub status: String,
}
