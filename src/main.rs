use actix_web::{App, HttpServer};

use chat_server_rpc::chat_server_server::{ChatServer, ChatServerServer};
use chat_server_rpc::{CreateRoomRequest, CreateRoomResponse};
use tonic::transport::Server;
use tonic::{Request, Response, Status};

mod server;

pub mod chat_server_rpc {
    tonic::include_proto!("chatserver");
}

#[derive(Debug, Default)]
pub struct ChatServerService {}

#[tonic::async_trait]
impl ChatServer for ChatServerService {
    async fn create_room(&self, request: Request<CreateRoomRequest>) -> Result<Response<CreateRoomResponse>, Status> {
        let req = request.into_inner();
        println!("Creating new room for event {}", req.event_code);

        // TODO - Create actual room

        let reply = CreateRoomResponse { successful: true };
        Ok(Response::new(reply))
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let chat_server_service = ChatServerService::default();
    Server::builder()
        .add_service(ChatServerServer::new(chat_server_service))
        .serve("[::1]:50051".parse().unwrap())
        .await.unwrap();

    HttpServer::new(|| App::new())
        .bind(("0.0.0.0", 8060))?
        .run()
        .await
}
