use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;

fn elapsed(c: &mut Criterion) {
    c.bench_function("elapsed", |b| {
        let base = std::time::Instant::now();
        b.iter(|| base.elapsed());
    });
}

fn trace_event(c: &mut Criterion) {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    std::env::set_current_dir(&dir).expect("failed to set current dir");
    println!("writing to {}", dir.path().display());

    use tracing_subscriber::layer::SubscriberExt;
    let subscriber = tracing_subscriber::Registry::default()
        .with(tracing_tape_recorder::TapeRecorder::default());

    tracing::subscriber::with_default(subscriber, || {
        c.bench_function("event", |b| {
            b.iter(|| {
                tracing::info!("some event");
            });
        });
    });

    dir.close().expect("failed to close tempdir");
}

fn trace_span(c: &mut Criterion) {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    std::env::set_current_dir(&dir).expect("failed to set current dir");
    println!("writing to {}", dir.path().display());

    use tracing_subscriber::layer::SubscriberExt;
    let subscriber = tracing_subscriber::Registry::default()
        .with(tracing_tape_recorder::TapeRecorder::default());

    tracing::subscriber::with_default(subscriber, || {
        c.bench_function("span", |b| {
            b.iter(|| {
                let _ = tracing::info_span!("some span").entered();
            });
        });
    });

    dir.close().expect("failed to close tempdir");
}

criterion_group!(benches, elapsed, trace_event, trace_span);
criterion_main!(benches);
