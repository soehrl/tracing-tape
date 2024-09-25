#![feature(thread_id_value)]

use std::{
    borrow::Cow,
    fmt::Display,
    fs::File,
    os::unix::fs::FileExt,
    path::Path,
    sync::atomic::{AtomicU32, AtomicU64, Ordering},
    time::Instant,
};

use dashmap::{DashMap, DashSet};
use file_format::{ChapterSummary, Intro, SERIALIZED_CHAPTER_SUMMARY_LEN, SERIALIZED_INTRO_LEN};
use tracing::{
    span::{Attributes, Id},
    Subscriber,
};

pub mod file_format;
use serde::{Deserialize, Serialize};
use tracing_subscriber::registry::LookupSpan;
#[derive(Debug)]
struct ChapterInfo {
    data_start: AtomicU64,
    bytes_written: AtomicU32,
    min_timestamp: AtomicU64,
    max_timestamp: AtomicU64,
    callsite_count: AtomicU32,
    event_counts: [AtomicU32; 5],
}

impl ChapterInfo {
    fn reset(&self) {
        self.data_start.store(0, Ordering::Relaxed);
        self.min_timestamp.store(u64::MAX, Ordering::Relaxed);
        self.max_timestamp.store(0, Ordering::Relaxed);
        self.callsite_count.store(0, Ordering::Relaxed);
        for count in &self.event_counts {
            count.store(0, Ordering::Relaxed);
        }
        self.bytes_written.store(0, Ordering::Relaxed);
    }
}

impl Default for ChapterInfo {
    fn default() -> Self {
        Self {
            data_start: AtomicU64::new(0),
            min_timestamp: AtomicU64::new(u64::MAX),
            max_timestamp: AtomicU64::new(0),
            callsite_count: AtomicU32::new(0),
            event_counts: Default::default(),
            bytes_written: AtomicU32::new(0),
        }
    }
}

#[derive(Debug)]
pub struct TapeRecorder {
    file: File,
    offset: AtomicU64,
    callsites: DashMap<tracing::callsite::Identifier, u64>,
    recorded_threads: DashSet<u64>,
    init_instant: Instant,

    section_size: u32,
    section_infos: [ChapterInfo; 2],
    next_chapter_summary: AtomicU64,
}

impl Default for TapeRecorder {
    fn default() -> Self {
        let exe = std::env::current_exe().ok();
        let name = exe
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy())
            .unwrap_or(Cow::Borrowed("trace"));

        let time = time::OffsetDateTime::now_local()
            .ok()
            .unwrap_or_else(time::OffsetDateTime::now_utc);
        let format = time::macros::format_description!(
            "[year]-[month]-[day]_[weekday repr:short]_[hour]-[minute]-[second]"
        );

        let time_format = time
            .format(&format)
            .ok()
            .unwrap_or_else(|| time.unix_timestamp().to_string());

        let file_path = format!("{}_{}.tape", name, time_format);
        return Self::with_file(file_path).unwrap();
    }
}

impl TapeRecorder {
    pub fn with_file<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let file = File::create_new(path)?;

        let now_system = time::OffsetDateTime::now_local()
            .ok()
            .unwrap_or_else(time::OffsetDateTime::now_utc);
        let now_instant = Instant::now();

        let section_size = 1024 * 1024 * 16;

        let file_header = Intro::new(section_size, now_system);
        let vec = postcard::to_allocvec(&file_header).unwrap();
        debug_assert_eq!(vec.len(), SERIALIZED_INTRO_LEN);
        file.write_all_at(&vec, 0).unwrap();

        let layer = Self {
            file,
            offset: AtomicU64::new(SERIALIZED_INTRO_LEN as u64),
            callsites: DashMap::new(),
            init_instant: now_instant,
            section_infos: Default::default(),
            section_size,
            next_chapter_summary: AtomicU64::new(0),
            recorded_threads: DashSet::new(),
        };

        layer
            .chapter_info(0)
            .data_start
            .store(SERIALIZED_INTRO_LEN as u64, Ordering::Release);

        Ok(layer)
    }

    #[inline]
    fn elapsed_nanos(&self) -> u64 {
        self.init_instant.elapsed().as_nanos() as u64
    }

    #[inline]
    fn chapter_index(&self, offset: u64) -> u64 {
        ((offset - SERIALIZED_INTRO_LEN as u64) / self.section_size as u64) as u64
    }

    #[inline]
    fn chapter_offset(&self, chapter_index: u64) -> u64 {
        SERIALIZED_INTRO_LEN as u64 + chapter_index * self.section_size as u64
    }

    #[inline]
    fn chapter_ranges(&self, chapter_index: u64) -> (std::ops::Range<u64>, std::ops::Range<u64>) {
        let chapter_start = self.chapter_offset(chapter_index);
        let chapter_end = chapter_start + self.section_size as u64;
        let summary_start = chapter_end - SERIALIZED_CHAPTER_SUMMARY_LEN as u64;
        (chapter_start..chapter_end, summary_start..chapter_end)
    }

    fn chapter_info(&self, chapter_index: u64) -> &ChapterInfo {
        #[cfg(debug_assertions)]
        {
            let next_chapter_summary = self.next_chapter_summary.load(Ordering::Relaxed);
            debug_assert!((next_chapter_summary..next_chapter_summary + 2).contains(&chapter_index));
        }
        while self.next_chapter_summary.load(Ordering::Relaxed) < chapter_index {
            println!("spinning {chapter_index}");
            std::hint::spin_loop();
        }
        &self.section_infos[(chapter_index % 2) as usize]
    }

    #[cfg(unix)]
    #[inline]
    fn write(&self, data: &[u8], offset: u64) {
        use std::os::unix::fs::FileExt;
        self.file.write_all_at(&data, offset).unwrap();
    }

    fn write_chapter_summary(&self, chapter_index: u64, data_end: u64) {
        let chapter_info = self.chapter_info(chapter_index);
        let (chapter_range, chapter_summary_range) = self.chapter_ranges(chapter_index);

        // Wait until data start is written
        let mut data_start;
        while {
            // Relaxed ordering is fine here, we ensure with the bytes_written load that all
            // previous writes are visible.
            data_start = chapter_info.data_start.load(Ordering::Relaxed);
            data_start == 0
        } {
            println!("spinning data start {data_start}");
            std::hint::spin_loop();
        }

        // Wait until all bytes have been written, whch also indicates that the data in the chapter
        // summary is final.
        let data_len = data_end - data_start;
        while data_len != chapter_info.bytes_written.load(Ordering::Acquire) as u64 {
            println!("spinning data_len {data_len}");
            std::hint::spin_loop();
        }

        let summary = ChapterSummary {
            min_timestamp: chapter_info.min_timestamp.load(Ordering::Relaxed).into(),
            max_timestamp: chapter_info.max_timestamp.load(Ordering::Relaxed).into(),
            data_offset: ((data_start - chapter_range.start) as u32).into(),
            data_len: (data_len as u32).into(),
            metadata_count: chapter_info.callsite_count.load(Ordering::Relaxed).into(),
            event_counts: [
                chapter_info.event_counts[0].load(Ordering::Relaxed).into(),
                chapter_info.event_counts[1].load(Ordering::Relaxed).into(),
                chapter_info.event_counts[2].load(Ordering::Relaxed).into(),
                chapter_info.event_counts[3].load(Ordering::Relaxed).into(),
                chapter_info.event_counts[4].load(Ordering::Relaxed).into(),
            ],
        };

        let mut chapter_summary = [0; SERIALIZED_CHAPTER_SUMMARY_LEN];
        postcard::to_slice(&summary, &mut chapter_summary).unwrap();
        self.write(&chapter_summary, chapter_summary_range.start);

        chapter_info.reset();

        let exc = self.next_chapter_summary.compare_exchange(
            chapter_index,
            chapter_index + 1,
            Ordering::Release,
            Ordering::Relaxed,
        );
        debug_assert!(exc.is_ok());
    }

    fn write_record_data<F: Fn(&ChapterInfo)>(&self, data: &[u8], f: F) -> u64 {
        if data.len() > (self.section_size as usize - SERIALIZED_CHAPTER_SUMMARY_LEN) / 2 {
            panic!("record too large");
        }

        fn ranges_overlap(a: &std::ops::Range<u64>, b: &std::ops::Range<u64>) -> bool {
            a.start < b.end && b.start < a.end
        }

        let data_start = self.offset.fetch_add(data.len() as u64, Ordering::Relaxed);
        let data_end = data_start + data.len() as u64;

        // The data range describes the range that the record_data should be written to.
        let data_range = data_start..data_end;

        // `first_chapter` defindes the chapter that includes the first byte of the data_range, and
        // `second_chapter` defines the chapter that includes the last byte of the data_range.
        let first_chapter_index = self.chapter_index(data_start);
        let second_chapter_index = self.chapter_index(data_end - 1);
        let (_, first_chapter_summary_range) = self.chapter_ranges(first_chapter_index);

        if first_chapter_index == second_chapter_index
            && !ranges_overlap(&first_chapter_summary_range, &data_range)
        {
            // Hot path

            // Write first, as chapter_info() may block if called too early, this might cause
            // write_chapter_summary to wait until bytes_written is updated, but we optimise for
            // the case where the chapter summary does not need to be written.
            self.write(data, data_start);

            // Update chapter info
            let chapter_summary = &self.chapter_info(first_chapter_index);
            f(chapter_summary);

            // This must be the last operation on the chapter info with a release ordering to
            // ensure all previous writes are visible to other threads.
            chapter_summary
                .bytes_written
                .fetch_add(data.len() as u32, Ordering::Release);

            data_start
        } else {
            // Cold path

            if data_range.contains(&first_chapter_summary_range.start) {
                self.write_chapter_summary(first_chapter_index, data_start);
            }
            if data_range.contains(&(first_chapter_summary_range.end - 1)) {
                self.chapter_info(first_chapter_index + 1)
                    .data_start
                    .store(data_range.end, Ordering::Relaxed);
            }

            self.write_record_data(data, f)
        }
    }

    /// Checks whether the current thread has already been recorded, and does so, if not.
    #[inline]
    fn check_thread(&self) {
        #[cold]
        fn insert_thread(rec: &TapeRecorder) {
            let thread = std::thread::current();
            let thread_id = thread.id().as_u64().into();

            let thread_record = Record::Thread(Thread {
                id: thread_id,
                name: thread.name().map(|name| name.into()),
            });
            rec.write_record_data(&thread_record.serialize_to_vec(), |info| {
                info.callsite_count.fetch_add(1, Ordering::Relaxed);
            });
        }

        let thread_id = std::thread::current().id().as_u64().into();

        if self.recorded_threads.insert(thread_id) {
            insert_thread(self);
        }
    }
}


impl<S> tracing_subscriber::Layer<S> for TapeRecorder
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn register_callsite(
        &self,
        metadata: &'static tracing::Metadata<'static>,
    ) -> tracing::subscriber::Interest {
        let level = Level::from(*metadata.level());
        let callsite_record = Record::Callsite(Metadata {
            name: metadata.name().into(),
            target: metadata.target().into(),
            level,
            module_path: metadata.module_path().map(|p| p.into()),
            file: metadata.file().map(|f| f.into()),
            line: metadata.line(),
            field_names: metadata.fields().iter().map(|f| f.name().into()).collect(),
            kind: 0,
        });

        let data = callsite_record.serialize_to_vec();
        let offset = self.write_record_data(&data, |info| {
            info.callsite_count.fetch_add(1, Ordering::Relaxed);
        });
        self.callsites.insert(metadata.callsite(), offset);

        // println!("register_callsite: {:?}", metadata);
        tracing::subscriber::Interest::sometimes()
    }

    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let timestamp = self.elapsed_nanos();
        let level = Level::from(*event.metadata().level());
        let mut event_record = Event {
            timestamp,
            callsite: *self
                .callsites
                .get(&event.metadata().callsite())
                .expect("callsite not registered"),
            values: vec![], // TODO: pre-allocate vec
        };
        event.record(&mut event_record);
        let data = Record::Event(event_record).serialize_to_vec();
        self.write_record_data(&data, |chapter_info| {
            chapter_info.event_counts[level as usize].fetch_add(1, Ordering::Relaxed);
            chapter_info
                .min_timestamp
                .fetch_min(timestamp, Ordering::Relaxed);
            chapter_info
                .max_timestamp
                .fetch_max(timestamp, Ordering::Relaxed);
        });
    }

    fn on_new_span(
        &self,
        attrs: &Attributes<'_>,
        id: &Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let timestamp = self.elapsed_nanos();

        let mut intermediate = SpanIntermediate::new(timestamp);
        attrs.record(&mut intermediate);

        let span = ctx.span(id).expect("span not found");
        span.extensions_mut().insert(intermediate);

        //         ctx.span(id).unwrap().metadata

        //         let parent_id = if let Some(parent) = attrs.parent() {
        //             Some(parent.into_u64())
        //         } else if attrs.is_contextual() {
        //             ctx.current_span().id().map(|id| id.into_u64())
        //         } else {
        //             None
        //         };

        //         let mut new_span = Span {
        //             timestamp,
        //             callsite: *self
        //                 .callsites
        //                 .get(&attrs.metadata().callsite())
        //                 .expect("callsite not registered"),
        //             id: id.into_u64(),
        //             parent_id,
        //             values: vec![], // TODO: pre-allocate vec
        //         };
        //         // attrs.record(&mut new_span);

        //         let data = Record::SpanOpened(new_span).serialize_to_vec();
        //         self.write_record_data(&data, |chapter_info| {
        //             chapter_info
        //                 .min_timestamp
        //                 .fetch_min(timestamp, Ordering::Relaxed);
        //             chapter_info
        //                 .max_timestamp
        //                 .fetch_max(timestamp, Ordering::Relaxed);
        //         });
    }

    fn on_enter(&self, id: &Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        let timestamp = self.elapsed_nanos();
        self.check_thread();
        let span = ctx.span(id).expect("span not found");
        let mut extensions = span.extensions_mut();
        let intermediate = extensions
            .get_mut::<SpanIntermediate>()
            .expect("span times not found");
        intermediate.entrances.push(SpanEntrance::new(timestamp));

        //         let _block_time = BlockTime::new("enter");
        //         let timestamp = self.elapsed_nanos();
        //         let span_event = SpanEvent {
        //             timestamp,
        //             id: id.into_u64(),
        //         };
        //         let data = Record::SpanEntered(span_event).serialize_to_vec();
        //         self.write_record_data_with_timestamp(&data, timestamp);
    }

    fn on_exit(&self, id: &Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        let timestamp = self.elapsed_nanos();
        let span = ctx.span(id).expect("span not found");
        let mut extensions = span.extensions_mut();
        let intermediate = extensions
            .get_mut::<SpanIntermediate>()
            .expect("span times not found");
        let last_entrance = intermediate.entrances.last_mut().expect("no entrance");
        last_entrance.exit = timestamp;
        debug_assert_eq!(
            last_entrance.thread_id,
            std::thread::current().id().as_u64().into()
        );
        // let _block_time = BlockTime::new("exit");
        // let timestamp = self.elapsed_nanos();
        // let span_event = SpanEvent {
        //     timestamp,
        //     id: id.into_u64(),
        // };
        // let data = Record::SpanExited(span_event).serialize_to_vec();
        // self.write_record_data_with_timestamp(&data, timestamp);
    }

    fn on_close(&self, id: Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        let timestamp = self.elapsed_nanos();
        let mut intermediate = {
            let span = ctx.span(&id).expect("span not found");
            let mut extensions = span.extensions_mut();
            extensions
                .remove::<SpanIntermediate>()
                .expect("span times not found")
        };
        intermediate.closed = timestamp;

        let span_record = Record::Span(Span {
            opened: intermediate.opened,
            entrances: intermediate.entrances,
            closed: intermediate.closed,
            callsite: *self
                .callsites
                .get(&ctx.metadata(&id).expect("metadata not found").callsite())
                .expect("callsite not registered"),
            id: id.into_u64(),
            parent_id: None,
            values: intermediate.values,
        });
        self.write_record_data(&span_record.serialize_to_vec(), |chapter_info| {
            chapter_info
                .min_timestamp
                .fetch_min(intermediate.opened, Ordering::Relaxed);
            chapter_info
                .max_timestamp
                .fetch_max(timestamp, Ordering::Relaxed);
        });

        // let _block_time = BlockTime::new("close");
        // let span_event = SpanEvent {
        //     timestamp,
        //     id: id.into_u64(),
        // };
        // let data = Record::SpanClosed(span_event).serialize_to_vec();
        // self.write_record_data_with_timestamp(&data, timestamp);
    }

    fn on_record(
        &self,
        _span: &Id,
        _values: &tracing::span::Record<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        println!("on_record: {:?}", _span);
    }

    fn on_follows_from(
        &self,
        _span: &Id,
        _follows: &Id,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        println!("on_follows_from: {:?}, {:?}", _span, _follows);
    }
}

struct Flavor<'a> {
    file: &'a File,
    file_offset: u64,
    bytes_read: u64,
}

impl<'de> postcard::de_flavors::Flavor<'de> for &'de mut Flavor<'de> {
    type Remainder = u64;
    type Source = ();

    fn pop(&mut self) -> postcard::Result<u8> {
        use std::os::unix::fs::FileExt;
        let mut byte = [0];
        self.file
            .read_exact_at(&mut byte, self.file_offset)
            .map_err(|_| postcard::Error::DeserializeUnexpectedEnd)?;
        self.file_offset += 1;
        self.bytes_read += 1;
        Ok(byte[0])
    }

    fn try_take_n(&mut self, _ct: usize) -> postcard::Result<&'de [u8]> {
        todo!();
    }

    fn finalize(self) -> postcard::Result<Self::Remainder> {
        Ok(self.bytes_read)
    }
}
