use std::time::{Duration, Instant};

use actix::{fut, Actor, ActorContext, Addr, AsyncContext, Running, StreamHandler};
use actix_web_actors::ws;
use uuid::Uuid;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(15);

pub struct WSRoom {
    conn_id: Uuid,
    event_code: String,
    hb: Instant,
}

impl WSRoom {
    pub fn new(event_code: String) -> Self {
        Self {
            conn_id: Uuid::new_v4(),
            event_code,
            hb: Instant::now(),
        }
    }

    fn hb(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                println!(
                    "Disconnecting client {} due to failed heartbeat!",
                    act.conn_id
                );
                ctx.stop();
                return;
            }

            ctx.ping(b"");
        });
    }
}

impl Actor for WSRoom {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);

        // let addr = ctx.address();
        // self.room_addr.send(Connect {
        //     addr: addr.recipient(),
        //     room_id: self.room,
        //     conn_id: self.id,
        // })
        // .into_actor(self)
        // .then(|res, _, ctx| {
        //     match res {
        //         Ok(res) => (),
        //         _ => ctx.stop()
        //     }
        //     fut::ready(())
        // })
        // .wait(ctx);
    }

    fn stopping(&mut self, ctx: &mut Self::Context) -> Running {
        // self.room_addr.do_send(Disconnect {
        //     conn_id: self.id,
        //     room_id: self.room,
        // });
        Running::Stop
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WSRoom {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.hb = Instant::now();
            }
            Ok(ws::Message::Text(text)) => ctx.text(text),
            Ok(ws::Message::Binary(bin)) => ctx.binary(bin),
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            _ => ctx.stop(),
        }
    }
}
