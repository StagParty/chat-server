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
use types::{Room, Rooms, Tokens, User};

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
