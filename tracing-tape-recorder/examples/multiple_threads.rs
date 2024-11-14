use std::sync::mpsc::{Receiver, Sender};

use tracing::{subscriber::set_default, trace_span};
use tracing_subscriber::{layer::SubscriberExt, Registry};
use tracing_tape_recorder::TapeRecorder;

#[tracing::instrument]
fn fib(n: u64) -> u64 {
    if n == 0 || n == 1 {
        n
    } else {
        fib(n - 1) + fib(n - 2)
    }
}

fn fib_calculator(recv: Receiver<u64>, send: Sender<u64>) {
    while let Ok(n) = recv.recv() {
        trace_span!("handle request").in_scope(|| {
            let result = fib(n);
            send.send(result).unwrap();
        });
    }
}

fn main() {
    let recorder = TapeRecorder::default();
    let guard = set_default(Registry::default().with(recorder.clone()));

    let (send_input, recv_input) = std::sync::mpsc::channel();
    let (send_output, recv_output) = std::sync::mpsc::channel();

    let handle = std::thread::spawn(move || {
        let guard = set_default(Registry::default().with(recorder.clone()));
        fib_calculator(recv_input, send_output);
        drop(guard);
    });

    for i in 0..20 {
        trace_span!("sending input", input = i).in_scope(|| {
            send_input.send(i).unwrap();
        });
    }
    drop(send_input);

    for i in 0..20 {
        trace_span!("receiving result", input = i).in_scope(|| {
            let result = recv_output.recv().unwrap();
            tracing::info!(result, fib = i, "calculated fib");
        });
    }
    drop(recv_output);

    handle.join().unwrap();

    drop(guard);
}
