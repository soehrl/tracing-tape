//! # Tracing Tape Recorder
//! This crate provides a [tracing subscriber layer](tracing_subscriber::Layer)
//! that records [tracing] events to a file in the [tracing-tape](tracing_tape)
//! format.
//!
//! ## Setup
//! First, add the the [tracing], [tracing_subscriber], and
//! [tracing-tape-recorder](self) dependencies to your application:`cargo add
//! tracing tracing_subscriber tracing-tape-recorder`.
//! Then, you can use the [TapeRecorder] layer in your application:
//!
//! ```rust
//! use tracing_subscriber::layer::SubscriberExt;
//! use tracing_tape_recorder::TapeRecorder;
//! let subscriber = tracing_subscriber::Registry::default().with(TapeRecorder::default());
//! tracing::subscriber::set_global_default(subscriber).expect("failed to set default subscriber");
//! ```
//! This will create a new *.tape file in the current working directory with the
//! name based on the executable name and the current time. This file can be
//! viewed using the trace-deck.

use std::{
    borrow::Cow,
    fs::File,
    hint,
    io::Write,
    path::Path,
    sync::atomic::{AtomicPtr, AtomicU64, Ordering},
    time::Instant,
};

use tracing::{
    span::{Attributes, Id},
    Subscriber,
};
use tracing_subscriber::registry::LookupSpan;
use tracing_tape::{
    intro::Intro,
    record::{
        field_type, CallsiteFieldRecord, CallsiteRecord, EventRecord, EventValueRecord,
        SpanCloseRecord, SpanEnterRecord, SpanExitRecord, SpanFollowsRecord, SpanOpenRecord,
        SpanValueRecord,
    },
};
use zerocopy::AsBytes;

#[derive(Debug)]
struct Chapter {
    chapter_size: usize,
    chapter_index: AtomicU64,
    data_offset: AtomicU64,
    bytes_written: AtomicU64,
    buffer: AtomicPtr<u8>,
}

impl Chapter {
    fn new(chapter_size: usize, chapter_index: u64) -> Self {
        let buffer = unsafe {
            std::alloc::alloc(std::alloc::Layout::from_size_align(chapter_size, 16).unwrap())
        };
        Self {
            chapter_size,
            chapter_index: AtomicU64::new(chapter_index),
            buffer: AtomicPtr::new(buffer),
            bytes_written: AtomicU64::new(0),
            data_offset: AtomicU64::new(0),
            // finished: AtomicBool::new(false),
            // finished_cond_var: Condvar::new(),
        }
    }

    unsafe fn as_bytes(&self) -> &[u8] {
        std::slice::from_raw_parts(self.buffer.load(Ordering::Relaxed), self.chapter_size)
    }

    unsafe fn byte_range_mut(&self, offset: usize, len: usize) -> &mut [u8] {
        std::slice::from_raw_parts_mut(self.buffer.load(Ordering::Relaxed).add(offset), len)
    }

    #[cold]
    fn finish(&self, file: &File, end_offset: u64, next_chapter_index: u64) {
        unsafe {
            self.byte_range_mut(end_offset as usize, self.chapter_size - end_offset as usize)
        }
        .fill(0);
        // TODO: do we need syncrhonization here?
        let data_offset = self.data_offset.load(Ordering::Relaxed);
        let expected_bytes_written = end_offset - data_offset;

        loop {
            // Acquire ordering because previuous writes to the buffer must be visible to
            // this thread.
            let bytes_written = self.bytes_written.load(Ordering::Acquire);
            if bytes_written == expected_bytes_written {
                break;
            }
            println!("waiting for written bytes {bytes_written} {expected_bytes_written}");
        }

        let offset = self.offset();
        let data = unsafe { self.as_bytes() };

        use std::os::unix::fs::FileExt;
        file.write_all_at(data, offset).unwrap();

        self.bytes_written.store(0, Ordering::Relaxed);
        self.data_offset.store(u64::max_value(), Ordering::Relaxed);
        self.chapter_index
            .store(next_chapter_index, Ordering::Release);
    }

    fn offset(&self) -> u64 {
        INTRO_SIZE as u64 + self.chapter_index.load(Ordering::Relaxed) * self.chapter_size as u64
    }
}

impl Drop for Chapter {
    fn drop(&mut self) {
        unsafe {
            std::alloc::dealloc(
                self.buffer.load(Ordering::Relaxed),
                std::alloc::Layout::from_size_align(self.chapter_size, 16).unwrap(),
            );
        }
    }
}

#[derive(Debug)]
pub struct TapeRecorder {
    file: File,
    offset: AtomicU64,
    init_instant: Instant,

    chapter_size: u32,
    chapter_size_pot: u8,
    chapter_offset_mask: u64,
    chapters: [Chapter; 2],
    random_state: ahash::RandomState,
}

const INTRO_SIZE: usize = std::mem::size_of::<Intro>();

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
    fn with_file<P: AsRef<Path>>(path: P) -> std::io::Result<Self> {
        let mut file = File::create_new(path)?;

        let now_system = time::OffsetDateTime::now_local()
            .ok()
            .unwrap_or_else(time::OffsetDateTime::now_utc);
        let now_instant = Instant::now();

        let chapter_size: u32 = 1024 * 1024;
        let chapter_size_pot = chapter_size.ilog2() as u8;

        let intro = Intro::new(chapter_size_pot, now_system.unix_timestamp_nanos());
        file.write_all(intro.as_bytes())?;

        Ok(Self {
            file,
            offset: AtomicU64::new(0),
            init_instant: now_instant,

            chapter_size,
            chapter_size_pot: chapter_size.ilog2() as u8,
            chapter_offset_mask: chapter_size as u64 - 1,
            chapters: [
                Chapter::new(chapter_size as usize, 0),
                Chapter::new(chapter_size as usize, 1),
            ],
            random_state: Default::default(),
        })
    }

    #[inline]
    fn elapsed_nanos(&self) -> i64 {
        self.init_instant.elapsed().as_nanos() as i64
    }

    #[inline]
    fn chapter_index(&self, offset: u64) -> u64 {
        offset >> self.chapter_size_pot
    }

    #[inline]
    fn chapter(&self, chapter_index: u64) -> &Chapter {
        let chapter = &self.chapters[(chapter_index & 1) as usize];
        while chapter.chapter_index.load(Ordering::Acquire) != chapter_index {
            println!("waiting for {chapter_index}");
            hint::spin_loop();
        }
        chapter
    }

    #[inline]
    fn write<F: Fn(&mut [u8])>(&self, size: usize, f: F) {
        if size > self.chapter_size as usize >> 2 {
            panic!("record too large");
        }

        let data_start = self.offset.fetch_add(size as u64, Ordering::Relaxed);
        let data_end = data_start + size as u64;

        let data_start_chapter = self.chapter_index(data_start);
        let data_end_chapter = self.chapter_index(data_end - 1);
        let chapter = self.chapter(data_start_chapter);
        let chapter_offset = data_start & self.chapter_offset_mask;

        if data_start_chapter == data_end_chapter {
            f(unsafe { chapter.byte_range_mut(chapter_offset as usize, size) });

            chapter
                .bytes_written
                .fetch_add(size as u64, Ordering::Release);

            if data_end & self.chapter_offset_mask == 0 {
                chapter.finish(&self.file, self.chapter_size as u64, data_start_chapter + 2);

                let next_chapter = self.chapter(data_start_chapter + 1);
                next_chapter.data_offset.store(0, Ordering::Relaxed);
            }
        } else {
            chapter.finish(&self.file, chapter_offset, data_start_chapter + 2);
            let next_chapter = self.chapter(data_start_chapter + 1);
            let next_chapter_offset = data_end & self.chapter_offset_mask;
            next_chapter
                .data_offset
                .store(next_chapter_offset, Ordering::Relaxed);
            unsafe { next_chapter.byte_range_mut(0, next_chapter_offset as usize) }.fill(0);
            self.write(size, f);
        }
    }
}

struct EventValueRecorder<'a> {
    recorder: &'a TapeRecorder,
    thread_id: u64,
}

impl EventValueRecorder<'_> {
    fn record_value(&self, field: &tracing::field::Field, kind: u8, value: &[u8]) {
        let field_id = self.recorder.random_state.hash_one(field.name());
        let record = EventValueRecord::new(field_id, kind, value.len(), self.thread_id);
        self.recorder
            .write(std::mem::size_of_val(&record) + value.len(), |slice| {
                let mut cursor = std::io::Cursor::new(slice);
                cursor.write_all(record.as_bytes()).unwrap();
                cursor.write_all(value).unwrap();
            });
    }
}

impl tracing::field::Visit for EventValueRecorder<'_> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let string = format!("{value:?}");
        self.record_value(field, field_type::STR, string.as_bytes());
    }

    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.record_value(field, field_type::F64, &value.to_le_bytes());
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.record_value(field, field_type::I64, &value.to_le_bytes());
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.record_value(field, field_type::U64, &value.to_le_bytes());
    }

    fn record_i128(&mut self, field: &tracing::field::Field, value: i128) {
        self.record_value(field, field_type::I128, &value.to_le_bytes());
    }

    fn record_u128(&mut self, field: &tracing::field::Field, value: u128) {
        self.record_value(field, field_type::U128, &value.to_le_bytes());
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.record_value(field, field_type::BOOL, &[value as u8]);
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.record_value(field, field_type::STR, value.as_bytes());
    }

    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        let string = value.to_string();
        self.record_value(field, field_type::ERROR, string.as_bytes());
    }
}

struct SpanValueRecorder<'a> {
    recorder: &'a TapeRecorder,
    span_id: u64,
}

impl SpanValueRecorder<'_> {
    fn record_value(&self, field: &tracing::field::Field, kind: u8, value: &[u8]) {
        let field_id = self.recorder.random_state.hash_one(field.name());
        let record = SpanValueRecord::new(field_id, kind, value.len(), self.span_id);
        self.recorder
            .write(std::mem::size_of_val(&record) + value.len(), |slice| {
                let mut cursor = std::io::Cursor::new(slice);
                cursor.write_all(record.as_bytes()).unwrap();
                cursor.write_all(value).unwrap();
            });
    }
}

impl tracing::field::Visit for SpanValueRecorder<'_> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let string = format!("{value:?}");
        self.record_value(field, field_type::STR, string.as_bytes());
    }

    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.record_value(field, field_type::F64, &value.to_le_bytes());
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        self.record_value(field, field_type::I64, &value.to_le_bytes());
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        self.record_value(field, field_type::U64, &value.to_le_bytes());
    }

    fn record_i128(&mut self, field: &tracing::field::Field, value: i128) {
        self.record_value(field, field_type::I128, &value.to_le_bytes());
    }

    fn record_u128(&mut self, field: &tracing::field::Field, value: u128) {
        self.record_value(field, field_type::U128, &value.to_le_bytes());
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.record_value(field, field_type::BOOL, &[value as u8]);
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.record_value(field, field_type::STR, value.as_bytes());
    }

    fn record_error(
        &mut self,
        field: &tracing::field::Field,
        value: &(dyn std::error::Error + 'static),
    ) {
        let string = value.to_string();
        self.record_value(field, field_type::ERROR, string.as_bytes());
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
        let id = self.random_state.hash_one(metadata.callsite());

        let module_path = metadata.module_path().unwrap_or("");
        let file = metadata.file().unwrap_or("");

        let name_len = metadata.name().len();
        let target_len = metadata.target().len();
        let module_path_len = module_path.len();
        let file_len = file.len();

        let record_len = std::mem::size_of::<CallsiteRecord>()
            + name_len
            + target_len
            + module_path_len
            + file_len;

        let callsite_record = CallsiteRecord::new(
            record_len as u16,
            if metadata.is_span() {
                tracing::metadata::Kind::SPAN
            } else {
                tracing::metadata::Kind::EVENT
            },
            *metadata.level(),
            metadata.fields().len() as u16,
            name_len as u16,
            target_len as u16,
            module_path_len as u16,
            file_len as u16,
            metadata.line().unwrap_or(0),
            id,
        );

        self.write(record_len, |slice| {
            let mut cursor = std::io::Cursor::new(slice);
            cursor.write_all(callsite_record.as_bytes()).unwrap();
            cursor.write_all(metadata.name().as_bytes()).unwrap();
            cursor.write_all(metadata.target().as_bytes()).unwrap();
            cursor.write_all(module_path.as_bytes()).unwrap();
            cursor.write_all(file.as_bytes()).unwrap();
        });

        for field in metadata.fields() {
            let field_record = CallsiteFieldRecord::new(
                field.name().len() as u16,
                id,
                self.random_state.hash_one(field.name()),
            );
            self.write(field_record.header.len.get() as usize, |slice| {
                let mut cursor = std::io::Cursor::new(slice);
                cursor.write_all(field_record.as_bytes()).unwrap();
                cursor.write_all(field.name().as_bytes()).unwrap();
            });
        }

        tracing::subscriber::Interest::sometimes()
    }

    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let timestamp = self.elapsed_nanos();
        let callsite_id = self.random_state.hash_one(event.metadata().callsite());
        let thread_id = self.random_state.hash_one(std::thread::current().id());
        let event_record = EventRecord::new(
            event.metadata().fields().len() as u16,
            timestamp,
            callsite_id,
            thread_id,
        );

        self.write(std::mem::size_of::<EventRecord>(), |slice| {
            slice.copy_from_slice(event_record.as_bytes());
        });
        let mut recorder = EventValueRecorder {
            recorder: self,
            thread_id,
        };
        event.record(&mut recorder);
    }

    fn on_new_span(
        &self,
        attrs: &Attributes<'_>,
        id: &Id,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let timestamp = self.elapsed_nanos();
        let id = self.random_state.hash_one(id);
        let callsite_id = self.random_state.hash_one(attrs.metadata().callsite());
        let parent = if let Some(parent) = attrs.parent() {
            Some(self.random_state.hash_one(parent))
        } else if attrs.is_contextual() {
            tracing::Span::current()
                .id()
                .map(|id| self.random_state.hash_one(&id))
        } else {
            None
        };
        let record = SpanOpenRecord::new(id, parent, callsite_id, timestamp);
        self.write(std::mem::size_of_val(&record), |slice| {
            slice.copy_from_slice(record.as_bytes());
        });
        let mut recorder = SpanValueRecorder {
            recorder: self,
            span_id: id,
        };
        attrs.record(&mut recorder);
    }

    fn on_enter(&self, id: &Id, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        let timestamp = self.elapsed_nanos();
        let id = self.random_state.hash_one(id);
        let thread_id = self.random_state.hash_one(std::thread::current().id());

        let record = SpanEnterRecord::new(id, timestamp, thread_id);
        self.write(std::mem::size_of_val(&record), |slice| {
            slice.copy_from_slice(record.as_bytes());
        });
    }

    fn on_exit(&self, id: &Id, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        let timestamp = self.elapsed_nanos();
        let id = self.random_state.hash_one(id);

        let record = SpanExitRecord::new(id, timestamp);
        self.write(std::mem::size_of_val(&record), |slice| {
            slice.copy_from_slice(record.as_bytes());
        });
    }

    fn on_close(&self, id: Id, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        let timestamp = self.elapsed_nanos();
        let id = self.random_state.hash_one(id);

        let record = SpanCloseRecord::new(id, timestamp);
        self.write(std::mem::size_of_val(&record), |slice| {
            slice.copy_from_slice(record.as_bytes());
        });
    }

    fn on_record(
        &self,
        id: &Id,
        values: &tracing::span::Record<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let id = self.random_state.hash_one(id);
        let mut recorder = SpanValueRecorder {
            recorder: self,
            span_id: id,
        };
        values.record(&mut recorder);
    }

    fn on_follows_from(
        &self,
        id: &Id,
        follows: &Id,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let id = self.random_state.hash_one(id);
        let follows = self.random_state.hash_one(follows);

        let record = SpanFollowsRecord::new(id, follows);
        self.write(std::mem::size_of_val(&record), |slice| {
            slice.copy_from_slice(record.as_bytes());
        });
    }
}
