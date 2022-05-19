use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt, TryFutureExt};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tonic::transport::Server;
use warp::{
    ws::{Message, WebSocket},
    Filter,
};

use rpc::chat_server_rpc::chat_server_server::ChatServerServer;
use types::{Room, Rooms, TokenUser, Tokens, User};

mod rpc;
mod types;

const JOIN_TOKEN_VALIDITY: Duration = Duration::from_secs(20);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rooms = Rooms::default();
    let tokens = Tokens::default();

    let chat_server_service = rpc::ChatServerService::new(rooms.clone(), tokens.clone());

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

async fn verify_token(msg: Message, rooms: Rooms, tokens: Tokens) -> Option<(TokenUser, Room)> {
    let token = msg.to_str().ok()?;
    let token_user = tokens.write().await.remove(token)?; // Check whether token exists

    let token_split: Vec<_> = token.split(":").collect();
    let token_ts_utc: u64 = token_split.get(1).unwrap().parse().unwrap();
    let since_epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

    // Checks whether token has expired
    if since_epoch.as_secs() - token_ts_utc > JOIN_TOKEN_VALIDITY.as_secs() {
        return None;
    }

    let rooms_read = rooms.read().await;
    let room = rooms_read.get(&token_user.event_code)?; // Checks whether the room exists

    Some((token_user, room.clone()))
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

    // Unbound channels for buffering messages
    let (tx, rx) = mpsc::unbounded_channel();
    let mut rx = UnboundedReceiverStream::new(rx);

    // Convert TokenUser to User
    let (me, my_room) = verify_result.unwrap();
    let me = User {
        id: me.id,
        username: me.username,
        event_code: me.event_code,
        tx,
    };
    println!(
        "Token verified successfully for uid={} username={}",
        me.id, me.username
    );

    // Push new user to my_room...
    my_room.write().await.push(me.clone());

    // ... and send welcome message to everyone
    let new_user_msg = format!("[SERVER] {} has joined!", me.username);
    my_room.read().await.iter().for_each(|u| {
        let _ = u.tx.send(Message::text(&new_user_msg));
    });

    // Spawn task for message buffering
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
                eprintln!("WebSocket receive error(uid={}): {}", me.id, e);
                break;
            }
        };
        user_message(&me, msg, &my_room).await;
    }

    user_disconnected(&me, &my_room).await;
}

async fn user_message(me: &User, msg: Message, my_room: &Room) {
    if let Ok(m) = msg.to_str() {
        println!("Received message from {}: {}", me.id, m);
        let new_msg = format!("[{}] {}", me.username, m);

        for user in my_room.read().await.iter() {
            if user.id != me.id {
                let _ = user.tx.send(Message::text(&new_msg));
            }
        }
    }
}

async fn user_disconnected(user: &User, room: &Room) {
    println!(
        "User {} has disconnected from room {}!",
        user.id, user.event_code
    );

    // Remove user from room
    let mut room_write = room.write().await;
    if let Some(idx) = room_write.iter().position(|u| u.id == user.id) {
        room_write.swap_remove(idx);
    }
}
