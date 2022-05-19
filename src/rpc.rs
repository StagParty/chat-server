use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::{Room, Rooms, TokenUser, Tokens};
use chat_server_rpc::{
    chat_server_server::ChatServer, CreateRoomRequest, CreateRoomResponse, JoinTokenRequest,
    JoinTokenResponse,
};
use tonic::{Request, Response, Status};
use uuid::Uuid;

pub mod chat_server_rpc {
    tonic::include_proto!("chatserver");
}

pub struct ChatServerService {
    rooms: Rooms,
    tokens: Tokens,
}

impl ChatServerService {
    pub fn new(rooms: Rooms, tokens: Tokens) -> Self {
        Self { rooms, tokens }
    }
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
                let user = TokenUser {
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
