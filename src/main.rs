use chat_server_rpc::{
    chat_server_server::{ChatServer, ChatServerServer},
    CreateRoomRequest, CreateRoomResponse,
};
use tonic::{transport::Server, Request, Response, Status};
use warp::{ws::WebSocket, Filter};

pub mod chat_server_rpc {
    tonic::include_proto!("chatserver");
}

#[derive(Debug, Default)]
pub struct ChatServerService {}

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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chat_server_service = ChatServerService::default();
    Server::builder()
        .add_service(ChatServerServer::new(chat_server_service))
        .serve("[::1]:50051".parse()?)
        .await?;

    let chat = warp::path("chat")
        .and(warp::ws())
        .map(|ws: warp::ws::Ws| ws.on_upgrade(|sock| user_connected(sock)));

    warp::serve(chat).run(([0, 0, 0, 0], 8060)).await;
    Ok(())
}

async fn user_connected(ws: WebSocket) {
    // TODO
    println!("New WS connection");
}
