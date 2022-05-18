use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use chat_server_rpc::{
    chat_server_server::{ChatServer, ChatServerServer},
    CreateRoomRequest, CreateRoomResponse, JoinTokenRequest, JoinTokenResponse,
};
use futures_util::{SinkExt, StreamExt, TryFutureExt};
use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tonic::{transport::Server, Request, Response, Status};
use uuid::Uuid;
use warp::{
    ws::{Message, WebSocket},
    Filter,
};

pub mod chat_server_rpc {
    tonic::include_proto!("chatserver");
}

type Tokens = Arc<RwLock<HashMap<String, User>>>; // key = token, value = user info
type Room = Arc<RwLock<HashSet<User>>>;
type Rooms = Arc<RwLock<HashMap<String, Room>>>; // key = event code, value = HashSet of users

const JOIN_TOKEN_VALIDITY: Duration = Duration::from_secs(20);

#[derive(Eq, Hash, PartialEq, Clone)]
struct User {
    id: i32,
    username: String,
    event_code: String,
}

struct ChatServerService {
    rooms: Rooms,
    tokens: Tokens,
}

#[tonic::async_trait]
impl ChatServer for ChatServerService {
    async fn create_room(
        &self,
        request: Request<CreateRoomRequest>,
    ) -> Result<Response<CreateRoomResponse>, Status> {
        let req = request.into_inner();
        let event_code = req.event_code;

        let reply = if self.rooms.read().await.contains_key(&event_code) {
            println!("Room with code {} already exists", event_code);
            CreateRoomResponse { successful: false }
        } else {
            let users = Room::default();
            self.rooms.write().await.insert(event_code.clone(), users);

            println!("Created new room for event {}", event_code);
            CreateRoomResponse { successful: true }
        };
        Ok(Response::new(reply))
    }

    async fn generate_join_token(
        &self,
        request: Request<JoinTokenRequest>,
    ) -> Result<Response<JoinTokenResponse>, Status> {
        let req = request.into_inner();
        let event_code = req.event_code;

        // Check whether the requested room exists
        if !self.rooms.read().await.contains_key(&event_code) {
            println!("Room {} for token request does not exist!", event_code);
            return Ok(Response::new(JoinTokenResponse {
                successful: false,
                token: "".to_owned(),
            }));
        }

        let hash = sha256::digest(Uuid::new_v4().to_string() + event_code.as_ref());

        let reply = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(now) => {
                let token = format!("{}:{}", hash, now.as_secs());
                let user = User {
                    id: req.user_id,
                    username: req.username,
                    event_code,
                };

                self.tokens.write().await.insert(token.clone(), user);
                println!("Generated new token: {}", token);

                JoinTokenResponse {
                    successful: true,
                    token,
                }
            }
            Err(_) => {
                eprintln!("Error generating join token: System Time earlier than UNIX Epoch!");
                JoinTokenResponse {
                    successful: false,
                    token: "".to_owned(),
                }
            }
        };
        Ok(Response::new(reply))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rooms = Rooms::default();
    let tokens = Tokens::default();

    let chat_server_service = ChatServerService {
        rooms: rooms.clone(),
        tokens: tokens.clone(),
    };

    let rpc_server = Server::builder()
        .add_service(ChatServerServer::new(chat_server_service))
        .serve("[::1]:50051".parse()?);

    // Turn the collections into Filters
    let rooms_filter = warp::any().map(move || rooms.clone());
    let tokens_filter = warp::any().map(move || tokens.clone());

    let chat = warp::path("chat")
        .and(warp::ws())
        .and(rooms_filter)
        .and(tokens_filter)
        .map(|ws: warp::ws::Ws, rooms: Rooms, tokens: Tokens| {
            ws.on_upgrade(move |sock| user_connected(sock, rooms, tokens))
        });

    let ws_server = warp::serve(chat).run(([127, 0, 0, 1], 8060));

    let ws_handle = tokio::spawn(ws_server);
    let rpc_handle = tokio::spawn(rpc_server);

    println!("WS and RPC Servers Started!");
    ws_handle.await?;
    rpc_handle.await??;
    Ok(())
}

async fn verify_token(msg: Message, rooms: Rooms, tokens: Tokens) -> Option<(User, Room)> {
    let token = msg.to_str().ok()?;
    let user = tokens.write().await.remove(token)?;

    let token_split: Vec<_> = token.split(":").collect();
    let token_ts_utc: u64 = token_split.get(1).unwrap().parse().unwrap();
    let since_epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

    if since_epoch.as_secs() - token_ts_utc > JOIN_TOKEN_VALIDITY.as_secs() {
        return None;
    }

    let rooms_read = rooms.read().await;
    let room = rooms_read.get(&user.event_code)?;
    room.write().await.insert(user.clone());

    Some((user, room.clone()))
}

async fn user_connected(ws: WebSocket, rooms: Rooms, tokens: Tokens) {
    println!("New WS connection");

    let (mut user_ws_tx, mut user_ws_rx) = ws.split();

    // Verify token
    let verify_result = if let Some(res) = user_ws_rx.next().await {
        match res {
            Ok(msg) => verify_token(msg, rooms, tokens).await,
            Err(e) => {
                eprintln!("Token verification error: {e}");
                None
            }
        }
    } else {
        None
    };

    if !verify_result.is_some() {
        println!("New user failed to verify token!");
        return;
    }

    let (me, my_room) = verify_result.unwrap();

    println!(
        "Token verified successfully for user {} joining event {}",
        me.id, me.event_code
    );

    // Unbound channels for buffering messages
    let (tx, rx) = mpsc::unbounded_channel();
    let mut rx = UnboundedReceiverStream::new(rx);

    tokio::task::spawn(async move {
        while let Some(message) = rx.next().await {
            user_ws_tx
                .send(message)
                .unwrap_or_else(|e| {
                    eprintln!("WebSocket send error: {}", e);
                })
                .await;
        }
    });

    while let Some(result) = user_ws_rx.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("websocket error(uid={}): {}", me.id, e);
                break;
            }
        };
        // user_message(my_id, msg, &users).await;
    }

    // user_disconnected(my_id, &users).await;
}
