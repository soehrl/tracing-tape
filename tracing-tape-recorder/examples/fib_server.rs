use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
};

use tracing::{info_span, subscriber::set_default};
use tracing_subscriber::{fmt, layer::SubscriberExt, Registry};
use tracing_tape_recorder::TapeRecorder;

#[tracing::instrument]
fn fib(n: u64) -> u64 {
    if n == 0 || n == 1 {
        n
    } else {
        fib(n - 1) + fib(n - 2)
    }
}

#[tracing::instrument]
fn handle_connection(mut stream: TcpStream, addr: SocketAddr) {
    let mut buffer = [0u8; 8];
    loop {
        if let Err(err) = info_span!("read request").in_scope(|| stream.read_exact(&mut buffer)) {
            tracing::error!(?err, "failed to read from stream");
            break;
        }

        let n = u64::from_be_bytes(buffer);
        tracing::info!(n, "received request");
        let result = fib(n);

        if let Err(err) =
            info_span!("write response").in_scope(|| stream.write_all(&result.to_be_bytes()))
        {
            tracing::error!(?err, "failed to write to stream");
            break;
        }
    }
}

fn main() {
    let recorder = TapeRecorder::default();
    let subscriber = Registry::default()
        .with(recorder.clone())
        .with(fmt::Layer::default());
    let guard = set_default(subscriber);

    let address = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:8080".to_string());

    let tcp_listener = std::net::TcpListener::bind(address).unwrap();
    while let Ok((stream, addr)) = info_span!("accept").in_scope(|| tcp_listener.accept()) {
        let recorder = recorder.clone();
        std::thread::spawn(move || {
            let subscriber = Registry::default()
                .with(recorder)
                .with(fmt::Layer::default());
            let guard = set_default(subscriber);
            handle_connection(stream, addr);
            drop(guard);
        });
    }

    drop(guard);
}
