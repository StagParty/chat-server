FROM rust:1.60 as builder
RUN apt-get update && apt-get install -y cmake && rm -rf /var/lib/apt/lists/*
COPY . /chat-server
WORKDIR /chat-server
RUN cargo install --path .

FROM debian:buster-slim
RUN rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/stagparty_chat_server /usr/local/bin/stagparty_chat_server
CMD ["stagparty_chat_server"]
