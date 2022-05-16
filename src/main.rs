use actix_web::{get, web, App, HttpRequest, HttpServer};
use actix_web_actors::ws;

mod server;

// TODO - Maybe use gRPC to create rooms?
#[get("/createroom")]
async fn create_room(req: HttpRequest, stream: web::Payload) -> String {
    if let Some(event_code) = req.headers().get("EVENT_CODE") {
        let _ = ws::start(server::WSRoom::new("sdfdfs".to_owned()), &req, stream);
        println!("{}", event_code.to_str().unwrap());

        "Created Event Room!".to_owned()
    } else {
        "EVENT_CODE Header not found!".to_owned()
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| App::new().service(create_room))
        .bind(("0.0.0.0", 8060))?
        .run()
        .await
}
