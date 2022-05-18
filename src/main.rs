use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use chat_server_rpc::{
    chat_server_server::{ChatServer, ChatServerServer},
    CreateRoomRequest, CreateRoomResponse, JoinTokenRequest, JoinTokenResponse,
};
use tokio::sync::RwLock;
use tonic::{transport::Server, Request, Response, Status};
use uuid::Uuid;
use warp::{ws::WebSocket, Filter};

pub mod chat_server_rpc {
    tonic::include_proto!("chatserver");
}

type Tokens = Arc<RwLock<HashSet<String>>>;

const JOIN_TOKEN_VALIDITY: Duration = Duration::from_secs(20);

#[derive(Debug, Default)]
pub struct ChatServerService {
    tokens: Tokens,
}

#[tonic::async_trait]
impl ChatServer for ChatServerService {
    async fn create_room(
        &self,
        request: Request<CreateRoomRequest>,
    ) -> Result<Response<CreateRoomResponse>, Status> {
        let req = request.into_inner();
        println!("Creating new room for event {}", req.event_code);

        // TODO - Create actual room

        let reply = CreateRoomResponse { successful: true };
        Ok(Response::new(reply))
    }

    async fn generate_join_token(
        &self,
        request: Request<JoinTokenRequest>,
    ) -> Result<Response<JoinTokenResponse>, Status> {
        let req = request.into_inner();
        let event_code = req.event_code;
        let hash = sha256::digest(Uuid::new_v4().to_string() + event_code.as_ref());

        let reply = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(now) => {
                let token = format!("{}:{}", hash, now.as_secs());
                self.tokens.write().await.insert(token.clone());
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
    let tokens = Arc::new(RwLock::new(HashSet::default()));

    let chat_server_service = ChatServerService { tokens };

    Server::builder()
        .add_service(ChatServerServer::new(chat_server_service))
        .serve("[::1]:50051".parse()?)
        .await?;

    let chat = warp::path("chat")
        .and(warp::ws())
        .map(|ws: warp::ws::Ws| ws.on_upgrade(move |sock| user_connected(sock)));

    warp::serve(chat).run(([0, 0, 0, 0], 8060)).await;
    Ok(())
}

async fn user_connected(ws: WebSocket) {
    // TODO
    println!("New WS connection");
}
