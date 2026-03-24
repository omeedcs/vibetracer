use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::event::AgentInfo;

/// The operating mode for a session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SessionMode {
    Enriched,
    Passive,
}

/// Metadata stored in a session's `meta.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub id: String,
    pub project_path: String,
    pub started_at: i64,
    pub mode: SessionMode,
    #[serde(default)]
    pub agents: Vec<AgentInfo>,
}

/// A handle to an active or created session on disk.
pub struct Session {
    pub id: String,
    pub dir: PathBuf,
}

impl Session {
    /// Generate a session ID in the format `YYYYMMDD-HHMMSS-xxxx`
    /// where `xxxx` is 4 random lowercase hex characters.
    pub fn generate_id() -> String {
        let now = Utc::now();
        let timestamp = now.format("%Y%m%d-%H%M%S").to_string();
        let micros = now.timestamp_micros();
        let hex_suffix = format!("{:04x}", (micros & 0xFFFF) as u16);
        format!("{}-{}", timestamp, hex_suffix)
    }
}

/// Manages sessions stored under a root `sessions_dir` directory.
pub struct SessionManager {
    pub sessions_dir: PathBuf,
}

impl SessionManager {
    pub fn new(sessions_dir: PathBuf) -> Self {
        Self { sessions_dir }
    }

    /// Create a new session: generates an ID, creates the directory layout,
    /// and writes `meta.json`.
    pub fn create(&self) -> Result<Session> {
        let id = Session::generate_id();
        let dir = self.sessions_dir.join(&id);

        fs::create_dir_all(dir.join("snapshots"))?;
        fs::create_dir_all(dir.join("checkpoints"))?;

        let meta = SessionMeta {
            id: id.clone(),
            project_path: String::new(),
            started_at: Utc::now().timestamp(),
            mode: SessionMode::Enriched,
            agents: Vec::new(),
        };

        let meta_path = dir.join("meta.json");
        let meta_json = serde_json::to_string_pretty(&meta)?;
        fs::write(meta_path, meta_json)?;

        Ok(Session { id, dir })
    }

    /// List all sessions sorted by `started_at` ascending.
    pub fn list(&self) -> Result<Vec<SessionMeta>> {
        if !self.sessions_dir.exists() {
            return Ok(Vec::new());
        }

        let mut metas = Vec::new();

        for entry in fs::read_dir(&self.sessions_dir)? {
            let entry = entry?;
            let meta_path = entry.path().join("meta.json");
            if meta_path.exists() {
                let content = fs::read_to_string(&meta_path)?;
                let meta: SessionMeta = serde_json::from_str(&content)?;
                metas.push(meta);
            }
        }

        metas.sort_by_key(|m| m.started_at);
        Ok(metas)
    }

    /// Load the metadata for a specific session by ID.
    pub fn load_meta(&self, id: &str) -> Result<SessionMeta> {
        let meta_path = self.sessions_dir.join(id).join("meta.json");
        let content = fs::read_to_string(&meta_path)?;
        let meta: SessionMeta = serde_json::from_str(&content)?;
        Ok(meta)
    }
}

// ─── unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_v1_meta_deserializes_with_empty_agents() {
        let v1_json = r#"{"id":"test-123","project_path":"/tmp","started_at":0,"mode":"passive"}"#;
        let meta: SessionMeta = serde_json::from_str(v1_json).unwrap();
        assert!(meta.agents.is_empty());
    }
}
