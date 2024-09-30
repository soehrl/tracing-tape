use tracing_tape_parser::Tape;

#[derive(Debug)]
pub enum CallsiteStatistics {
    Span(SpanCallsiteStatistics),
    Event(EventCallsiteStatistics),
}

#[derive(Debug)]
pub struct SpanCallsiteStatistics {
    pub q1: i64,
    pub q2: i64,
    pub q3: i64,
    pub iqr: i64,
    pub min: i64,
    pub max: i64,
    pub mean: i64,
    pub span_indices: Vec<usize>,
    pub outliers_slow: Vec<usize>,
    pub outliers_fast: Vec<usize>,
}

fn calculate_span_statistics(tape: &Tape, callsite_index: usize) -> SpanCallsiteStatistics {
    let mut min = i64::MAX;
    let mut max = i64::MIN;
    let mut sum = 0;
    let mut spans: Vec<(usize, i64)> = tape
        .spans()
        .node_weights()
        .enumerate()
        .filter_map(|(index, span)| {
            if span.callsite_index != callsite_index {
                return None;
            }

            let duration = span.closed - span.opened;
            sum += duration;
            min = min.min(duration);
            max = max.max(duration);

            Some((index, duration))
        })
        .collect();

    let q2_index = spans.len() / 2;
    let (lower_half, q2, upper_half) =
        spans.select_nth_unstable_by_key(q2_index, |(_, duration)| *duration);

    let q1_index = lower_half.len() / 2;
    let (smaller_q1, q1, _) =
        lower_half.select_nth_unstable_by_key(q1_index, |(_, duration)| *duration);

    let q3_index = upper_half.len() / 2;
    let (_, q3, greater_q3) =
        upper_half.select_nth_unstable_by_key(q3_index, |(_, duration)| *duration);

    let iqr = q3.1 - q1.1;
    let iqr_1_5 = iqr + iqr / 2;
    let lower_bound = q1.1 - iqr_1_5;
    let upper_bound = q3.1 + iqr_1_5;

    let mut outliers_fast = vec![];
    for value in smaller_q1 {
        if value.1 < lower_bound {
            outliers_fast.push(value.0);
        }
    }

    let mut outliers_slow = vec![];
    for value in greater_q3 {
        if value.1 > upper_bound {
            outliers_slow.push(value.0);
        }
    }

    SpanCallsiteStatistics {
        q1: q1.1,
        q2: q2.1,
        q3: q3.1,
        iqr,
        min,
        max,
        mean: sum / spans.len() as i64,
        outliers_slow,
        outliers_fast,
        span_indices: spans.into_iter().map(|(index, _)| index).collect(),
    }
}

#[derive(Debug)]
pub struct EventCallsiteStatistics {
}

fn calculate_event_statistics(tape: &Tape, callsite_index: usize) -> EventCallsiteStatistics {
    EventCallsiteStatistics {}
}

pub fn calculate_statistics(tape: &Tape, callsite_index: usize) -> CallsiteStatistics {
    let callsite = &tape.callsites()[callsite_index];
    if callsite.kind().is_span() {
        CallsiteStatistics::Span(calculate_span_statistics(tape, callsite_index))
    } else {
        CallsiteStatistics::Event(calculate_event_statistics(tape, callsite_index))
    }
}
