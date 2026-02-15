use std::sync::Arc;
use tokio::sync::Mutex;

use crate::types::{RoomId, UserId};

/// Thread-safe room using tokio's async Mutex.
pub struct Room {
    pub id: RoomId,
    pub name: String,
    pub members: Arc<Mutex<Vec<UserId>>>,
}

impl Room {
    pub fn new(id: RoomId, name: String) -> Self {
        Self {
            id,
            name,
            members: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn add_member(&self, user_id: UserId) {
        let mut members = self.members.lock().await;
        if !members.contains(&user_id) {
            members.push(user_id);
        }
    }

    pub async fn remove_member(&self, user_id: UserId) {
        self.members.lock().await.retain(|&id| id != user_id);
    }

    pub async fn member_ids(&self) -> Vec<UserId> {
        self.members.lock().await.clone()
    }
}
