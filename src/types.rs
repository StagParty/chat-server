use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use tokio::sync::RwLock;

pub type Tokens = Arc<RwLock<HashMap<String, User>>>; // key = token, value = user info
pub type Room = Arc<RwLock<HashSet<User>>>;
pub type Rooms = Arc<RwLock<HashMap<String, Room>>>; // key = event code, value = HashSet of users

#[derive(Eq, Hash, PartialEq, Clone)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub event_code: String,
}
