use std::{collections::HashMap, sync::Arc};

use tokio::sync::{mpsc, RwLock};
use warp::ws::Message;

pub type Tokens = Arc<RwLock<HashMap<String, TokenUser>>>; // key = token, value = user info
pub type Room = Arc<RwLock<Vec<User>>>;
pub type Rooms = Arc<RwLock<HashMap<String, Room>>>; // key = event code, value = HashSet of users

#[derive(Eq, Hash, PartialEq)]
pub struct TokenUser {
    pub id: i32,
    pub username: String,
    pub event_code: String,
}

#[derive(Clone)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub event_code: String,
    pub tx: mpsc::UnboundedSender<Message>,
}
