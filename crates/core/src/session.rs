use dashmap::DashMap;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

const DEFAULT_CLOSED_HISTORY_LIMIT: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Active,
    Closed,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub source: String,
    pub target: String,
    pub proxy: String,
    pub policy: Option<String>,
    pub matched_rule: Option<String>,
    pub created_at: Instant,
    pub closed_at: Option<Instant>,
    pub state: SessionState,
    pub upload: u64,
    pub download: u64,
}

impl Session {
    pub fn new(
        source: String,
        target: String,
        proxy: String,
        policy: Option<String>,
        matched_rule: Option<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            source,
            target,
            proxy,
            policy,
            matched_rule,
            created_at: Instant::now(),
            closed_at: None,
            state: SessionState::Active,
            upload: 0,
            download: 0,
        }
    }

    pub fn close(&mut self) {
        self.state = SessionState::Closed;
        self.closed_at = Some(Instant::now());
    }

    pub fn duration(&self) -> Duration {
        match self.closed_at {
            Some(closed_at) => closed_at.saturating_duration_since(self.created_at),
            None => self.created_at.elapsed(),
        }
    }

    pub fn total_traffic(&self) -> u64 {
        self.upload + self.download
    }
}

pub struct SessionManager {
    sessions: DashMap<String, Session>,
    closed_sessions: Mutex<VecDeque<Session>>,
    closed_history_limit: usize,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: DashMap::new(),
            closed_sessions: Mutex::new(VecDeque::new()),
            closed_history_limit: DEFAULT_CLOSED_HISTORY_LIMIT,
        }
    }

    pub fn create(
        &self,
        source: String,
        target: String,
        proxy: String,
        policy: Option<String>,
        matched_rule: Option<String>,
    ) -> Session {
        let session = Session::new(source, target, proxy, policy, matched_rule);
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
        if let Some((_, mut session)) = self.sessions.remove(id) {
            session.close();
            self.push_closed(session);
        }
    }

    pub async fn close_all(&self) {
        let ids: Vec<String> = self.sessions.iter().map(|entry| entry.key().clone()).collect();
        for id in ids {
            self.close(&id);
        }
    }

    pub fn clear_history(&self) {
        self.closed_sessions.lock().clear();
    }

    pub fn get_active(&self) -> Vec<Session> {
        self.sessions.iter().map(|entry| entry.value().clone()).collect()
    }

    pub fn get_recent_closed(&self, limit: usize) -> Vec<Session> {
        let limit = limit.min(self.closed_history_limit);
        self.closed_sessions
            .lock()
            .iter()
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn get_all(&self) -> Vec<Session> {
        self.get_active()
    }

    pub fn count(&self) -> usize {
        self.sessions.len()
    }

    pub fn total_upload(&self) -> u64 {
        self.sessions.iter().map(|entry| entry.value().upload).sum()
    }

    pub fn total_download(&self) -> u64 {
        self.sessions.iter().map(|entry| entry.value().download).sum()
    }

    fn push_closed(&self, session: Session) {
        let mut history = self.closed_sessions.lock();
        history.push_front(session);
        while history.len() > self.closed_history_limit {
            history.pop_back();
        }
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}
