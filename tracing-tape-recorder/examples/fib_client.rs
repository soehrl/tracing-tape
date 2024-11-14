use std::{
    io::{Read, Write},
    net::TcpStream,
};

use tracing::{error, info, info_span, subscriber::set_default};
use tracing_subscriber::{fmt, layer::SubscriberExt, Registry};
use tracing_tape_recorder::TapeRecorder;

fn main() {
    let subscriber = Registry::default()
        .with(TapeRecorder::default())
        .with(fmt::Layer::default());
    let guard = set_default(subscriber);

    let address = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());
    let mut stream = info_span!("connect").in_scope(|| TcpStream::connect(address).unwrap());

    let mut buffer = [0u8; 8];
    for i in 0..2000 {
        if let Err(err) = info_span!("request", i).in_scope(|| -> std::io::Result<()> {
            let n = i as u64 % 10;
            info!(n, "sending request");

            info_span!("write request", i).in_scope(|| stream.write_all(&n.to_be_bytes()))?;

            info_span!("read response", i).in_scope(|| stream.read_exact(&mut buffer))?;

            let result = u64::from_be_bytes(buffer);
            info!(result, "received response");

            info_span!("sleep", i)
                .in_scope(|| std::thread::sleep(std::time::Duration::from_millis(1)));

            Ok(())
        }) {
            error!(?err, "error");
            break;
        }
    }

    drop(guard);
}
