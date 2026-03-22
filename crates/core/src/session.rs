use dashmap::DashMap;
use std::time::{Duration, Instant};

/// 会话信息
#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub source: String,
    pub target: String,
    pub proxy: String,
    pub created_at: Instant,
    pub upload: u64,
    pub download: u64,
}

impl Session {
    pub fn new(source: String, target: String, proxy: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            source,
            target,
            proxy,
            created_at: Instant::now(),
            upload: 0,
            download: 0,
        }
    }

    pub fn duration(&self) -> Duration {
        self.created_at.elapsed()
    }

    pub fn total_traffic(&self) -> u64 {
        self.upload + self.download
    }
}

/// 会话管理器
pub struct SessionManager {
    sessions: DashMap<String, Session>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: DashMap::new(),
        }
    }

    pub fn create(&self, source: String, target: String, proxy: String) -> Session {
        let session = Session::new(source, target, proxy);
        self.sessions.insert(session.id.clone(), session.clone());
        session
    }

    pub fn get(&self, id: &str) -> Option<Session> {
        self.sessions.get(id).map(|s| s.clone())
    }

    pub fn update_traffic(&self, id: &str, upload: u64, download: u64) {
        if let Some(mut session) = self.sessions.get_mut(id) {
            session.upload += upload;
            session.download += download;
        }
    }

    pub fn close(&self, id: &str) {
        self.sessions.remove(id);
    }

    pub async fn close_all(&self) {
        self.sessions.clear();
    }

    pub fn get_all(&self) -> Vec<Session> {
        self.sessions.iter().map(|e| e.value().clone()).collect()
    }

    pub fn count(&self) -> usize {
        self.sessions.len()
    }

    pub fn total_upload(&self) -> u64 {
        self.sessions.iter().map(|e| e.value().upload).sum()
    }

    pub fn total_download(&self) -> u64 {
        self.sessions.iter().map(|e| e.value().download).sum()
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}
