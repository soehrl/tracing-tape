use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;

fn trace_event(c: &mut Criterion) {
    use tracing_subscriber::layer::SubscriberExt;
    let subscriber =
        tracing_subscriber::Registry::default().with(tracing_tape::TapeRecorder::default());

    tracing::subscriber::with_default(subscriber, || {
        c.bench_function("event", |b| {
            b.iter(|| {
                tracing::info!("some event");
            });
        });
    });
}

fn trace_span(c: &mut Criterion) {
    use tracing_subscriber::layer::SubscriberExt;
    let subscriber =
        tracing_subscriber::Registry::default().with(tracing_tape::TapeRecorder::default());

    tracing::subscriber::with_default(subscriber, || {
        c.bench_function("span", |b| {
            b.iter(|| {
                let _ = tracing::info_span!("some span").enter();
            });
        });
    });
}

criterion_group!(benches, trace_event, trace_span);
criterion_main!(benches);
