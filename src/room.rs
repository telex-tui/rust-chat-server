use std::sync::{Arc, Mutex};

use crate::types::{RoomId, UserId};

/// A chat room. Members stored behind Arc<Mutex> for thread-safe access.
///
/// Stage 2 used Rc<RefCell> — single-threaded shared mutable state.
/// Now that we have threads, Rc isn't Send (can't cross thread boundaries)
/// and RefCell isn't Sync (can't be shared between threads). Arc<Mutex>
/// is the thread-safe equivalent: Arc for shared ownership across threads,
/// Mutex for exclusive access.
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

    pub fn add_member(&self, user_id: UserId) {
        let mut members = self.members.lock().unwrap();
        if !members.contains(&user_id) {
            members.push(user_id);
        }
    }

    pub fn remove_member(&self, user_id: UserId) {
        self.members.lock().unwrap().retain(|&id| id != user_id);
    }

    pub fn member_ids(&self) -> Vec<UserId> {
        self.members.lock().unwrap().clone()
    }
}
