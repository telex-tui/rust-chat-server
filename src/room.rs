use std::cell::RefCell;
use std::rc::Rc;

use crate::types::{RoomId, UserId};

/// A chat room. Members are stored as a shared, mutable list of user IDs.
///
/// Rc<RefCell<Vec<UserId>>> lets multiple parts of the code hold a handle
/// to the membership list. Rc provides shared ownership; RefCell moves
/// borrow checking to runtime so we can mutate through a shared reference.
pub struct Room {
    #[allow(dead_code)]
    pub id: RoomId,
    pub name: String,
    pub members: Rc<RefCell<Vec<UserId>>>,
}

impl Room {
    pub fn new(id: RoomId, name: String) -> Self {
        Self {
            id,
            name,
            members: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn add_member(&self, user_id: UserId) {
        let mut members = self.members.borrow_mut();
        if !members.contains(&user_id) {
            members.push(user_id);
        }
    }

    pub fn remove_member(&self, user_id: UserId) {
        self.members.borrow_mut().retain(|&id| id != user_id);
    }

    pub fn member_ids(&self) -> Vec<UserId> {
        self.members.borrow().clone()
    }
}
