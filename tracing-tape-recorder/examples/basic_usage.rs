use tracing::trace_span;
use tracing_subscriber::{fmt, layer::SubscriberExt, Registry};
use tracing_tape_recorder::TapeRecorder;

#[tracing::instrument]
fn fib(n: u64) -> u64 {
    if n == 0 || n == 1 {
        trace_span!("base case").in_scope(|| n)
    } else {
        trace_span!("recursion").in_scope(|| fib(n - 1) + fib(n - 2))
    }
}

fn main() {
    let subscriber = Registry::default()
        .with(TapeRecorder::default())
        .with(fmt::Layer::default());
    let guard = tracing::subscriber::set_default(subscriber);

    let result = fib(20);
    tracing::info!(result, fib = 20, "calculated fib");

    drop(guard);
}
