use std::{
    fs::File,
    ops::Range,
    os::unix::fs::FileExt,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, OnceLock,
    },
    time::Instant,
};

use ahash::HashMap;
use crossbeam_channel::Receiver;
use tracing_tape::{
    file_format::{ChapterSummary, Intro, SERIALIZED_CHAPTER_SUMMARY_LEN, SERIALIZED_INTRO_LEN},
    Event, Metadata, Record, Span,
};

#[derive(Debug, Default)]
struct ParsedRecords<'a> {
    threads: HashMap<u64, tracing_tape::Thread<'a>>,
    callsites: Vec<(u64, Metadata<'a>)>,
    events: Vec<Event<'a>>,
    spans: Vec<Span<'a>>,
}

impl<'a> ParsedRecords<'a> {
    fn parse(bytes: &'a [u8], bytes_offset: u64) -> postcard::Result<Self> {
        let mut records = ParsedRecords::default();

        let mut slice = bytes;
        while !slice.is_empty() {
            let offset = bytes_offset + (bytes.len() - slice.len()) as u64;
            let record: Record;
            (record, slice) = postcard::take_from_bytes(&slice)?;
            match record {
                Record::Callsite(meta) => records.callsites.push((offset, meta)),
                Record::Thread(thread) => {
                    records.threads.insert(thread.id, thread);
                }
                Record::Event(event) => records.events.push(event),
                Record::Span(span) => records.spans.push(span),
            }
        }

        Ok(records)
    }
}

#[derive(Debug)]
pub struct ChapterData {
    // The bytes of the chapter. This data is not
    _slice: Box<[u8]>,

    // The lifetime is not actually 'static, but we can't express that in the type system. The
    // lifetime is actually tied to the slice field. The TapeSlice ensures that the memory of the
    // slice field is never deallocated, moved or mutaded while the TapeSlice is alive.
    records: ParsedRecords<'static>,
}

impl ChapterData {
    pub fn new(slice: Box<[u8]>, chapter_offset: u64, data_range: std::ops::Range<usize>) -> Self {
        let bytes_offset = chapter_offset + data_range.start as u64;
        let records =
            unsafe { std::mem::transmute(ParsedRecords::parse(&slice[data_range], bytes_offset)) };
        Self {
            // byte_offset,
            _slice: slice,
            records,
        }
    }

    pub fn spans(&self) -> &[Span] {
        &self.records.spans
    }
}

pub struct ChapterDataset<'a>(pub Vec<&'a ChapterData>);

#[derive(Debug)]
pub struct Chapter {
    file: Arc<File>,
    summary: ChapterSummary,
    location: Range<u64>,
    data: OnceLock<ChapterData>,
    data_receiver: OnceLock<Receiver<ChapterData>>,

    last_accessed_base: Instant,
    last_accessed_offset_nanos: AtomicU64,
}

impl Chapter {
    fn event_count(&self) -> usize {
        self.summary.event_counts.iter().map(|c| **c as usize).sum()
    }

    // Returns the data if it is loaded, otherwise it will load the data in the background and
    // return None.
    fn data(&self) -> Option<&ChapterData> {
        self.last_accessed_offset_nanos.fetch_max(
            self.last_accessed_base.elapsed().as_nanos() as u64,
            Ordering::Relaxed,
        );

        if let Some(data) = self.data.get() {
            Some(data)
        } else {
            let recv = self.data_receiver.get_or_init(|| {
                let (sender, receiver) = crossbeam_channel::bounded(1);
                let file = self.file.clone();
                let bytes = self.location.clone();
                let data_start = *self.summary.data_offset as usize;
                let data_end = data_start + *self.summary.data_len as usize;
                let data_range = data_start..data_end;
                std::thread::spawn(move || {
                    let mut slice = vec![0; (bytes.end - bytes.start) as usize];
                    file.read_exact_at(&mut slice, bytes.start).unwrap();

                    let data =
                        ChapterData::new(slice.into_boxed_slice(), bytes.start as u64, data_range);
                    sender.send(data).unwrap();
                });

                receiver
            });

            if let Ok(data) = recv.try_recv() {
                Some(self.data.get_or_init(|| data))
            } else {
                None
            }
        }
    }

    // Waits for the data to be loaded and returns it.
    //
    // When the data failed to load for some reason, this function will return Err(()).
    fn wait_for_data(&self) -> Result<&ChapterData, ()> {
        if let Some(data) = self.data() {
            Ok(data)
        } else {
            let receiver = self
                .data_receiver
                .get()
                .expect("data receiver must be present");
            let data = receiver.recv().map_err(|_| ())?;
            Ok(self.data.get_or_init(|| data))
        }
    }

    fn time_span(&self, base_time: time::OffsetDateTime) -> std::ops::Range<time::OffsetDateTime> {
        let start = base_time + time::Duration::nanoseconds(*self.summary.min_timestamp as i64);
        let end = base_time + time::Duration::nanoseconds(*self.summary.max_timestamp as i64);
        start..end
    }
}

pub struct Tape {
    path: PathBuf,
    file: Arc<File>,
    intro: Intro,
    chapters: Vec<Chapter>,
    callsites: HashMap<u64, Metadata<'static>>,
    threads: HashMap<String, u64>,
    time_span: std::ops::Range<time::OffsetDateTime>,
}

impl Tape {
    pub fn from_path<P: Into<PathBuf>>(path: P) -> std::io::Result<Self> {
        let path = path.into();
        let file = Arc::new(File::open(&path)?);

        let mut slice = [0; SERIALIZED_INTRO_LEN];
        file.read_exact_at(&mut slice, 0)?;
        let intro: Intro = postcard::from_bytes(&slice).unwrap();
        println!("intro: {:?}", intro);

        let metadata = file.metadata()?;
        let chapter_count =
            (metadata.len() - SERIALIZED_INTRO_LEN as u64) / *intro.chapter_len as u64;

        let mut chapters = Vec::with_capacity(chapter_count as usize);

        for i in 0..chapter_count {
            let chapter_start = SERIALIZED_INTRO_LEN as u64 + i * *intro.chapter_len as u64;
            let chapter_end = chapter_start + *intro.chapter_len as u64;
            let summary_start = chapter_end - SERIALIZED_CHAPTER_SUMMARY_LEN as u64;

            let mut slice = [0; SERIALIZED_CHAPTER_SUMMARY_LEN];
            file.read_exact_at(&mut slice, summary_start)?;
            let chapter_summary: ChapterSummary = postcard::from_bytes(&slice).unwrap();

            println!("chapter_summary: {:?}", chapter_summary);

            let chapter = Chapter {
                file: file.clone(),
                summary: chapter_summary,
                location: chapter_start..chapter_end,
                data: OnceLock::new(),
                data_receiver: OnceLock::new(),
                last_accessed_base: Instant::now(),
                last_accessed_offset_nanos: AtomicU64::new(0),
            };

            if *chapter.summary.metadata_count > 0 {
                // Already scheduled to load the data in the background to extract the callsites.
                chapter.data();
            }

            chapters.push(chapter);
        }

        let mut callsites: HashMap<u64, Metadata<'static>> = HashMap::default();
        let mut threads: HashMap<String, u64> = HashMap::default();

        for chapter in &chapters {
            if *chapter.summary.metadata_count > 0 {
                let data = chapter
                    .wait_for_data()
                    .map_err(|_| std::io::ErrorKind::InvalidData)?;

                for (offset, callsite) in &data.records.callsites {
                    callsites.insert(*offset, callsite.to_static());
                }

                for t in &data.records.threads {
                    if let Some(name) = &t.1.name {
                        threads.insert(name.to_string(), *t.0);
                    } else {
                        threads.insert(t.1.id.to_string(), *t.0);
                    }
                }
            }
        }

        let timestamp_base = time::OffsetDateTime::from_unix_timestamp_nanos(*intro.timestamp_base)
            .map_err(|err| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("invalid timestamp base: {}", err,),
                )
            })?;

        // Technically it is possible that the latest timestamp is not in the last chapter.
        let timespan_end = chapters
            .iter()
            .map(|c| *c.summary.max_timestamp)
            .max()
            .map(|max_timestamp| timestamp_base + time::Duration::nanoseconds(max_timestamp as i64))
            .unwrap_or(timestamp_base);
        let time_span = timestamp_base..timespan_end;

        Ok(Self {
            file,
            chapters,
            intro,
            path,
            callsites,
            time_span,
            threads,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn callsite(&self, offset: u64) -> Option<&Metadata> {
        self.callsites.get(&offset)
    }

    pub fn time_span(&self) -> &std::ops::Range<time::OffsetDateTime> {
        &self.time_span
    }

    pub fn timestamp_date_time(&self, timestamp: u64) -> time::OffsetDateTime {
        self.time_span.start + time::Duration::nanoseconds(timestamp as i64)
    }

    pub fn threads(&self) -> impl Iterator<Item = (&String, &u64)> {
        self.threads.iter()
    }

    pub fn events(&self) -> RecordedEvents {
        RecordedEvents {
            chapters: &self.chapters,
            event_index: 0,
        }
    }

    pub fn data_for_time_span(
        &self,
        timespan: std::ops::Range<time::OffsetDateTime>,
    ) -> ChapterDataset {
        let mut dataset = Vec::new();

        for c in &self.chapters {
            fn ranges_overlap(a: &std::ops::Range<u64>, b: &std::ops::Range<u64>) -> bool {
                a.start < b.end && b.start < a.end
            }

            let chapter_timespan = c.time_span(self.time_span.start);
            if timespan.start < chapter_timespan.end && chapter_timespan.start < timespan.end {
                if let Some(data) = c.data() {
                    dataset.push(data);
                }
            }
        }

        ChapterDataset(dataset)
    }

    pub fn find_event_offset(&self, time: time::OffsetDateTime) -> u64 {
        if time < self.time_span.start {
            return 0;
        }
        let mut offset: u64 = 0;
        for c in &self.chapters {
            let chapter_timespan = c.time_span(self.time_span.start);
            if !chapter_timespan.contains(&time) {
                offset += c.event_count() as u64;
                continue;
            }

            if let Some(data) = c.data() {
                let target_timestampe = (time - chapter_timespan.start).whole_nanoseconds() as u64;

                for e in &data.records.events {
                    if e.timestamp < target_timestampe {
                        offset += 1;
                    } else {
                        return offset;
                    }
                }
            } else {
                // Estimate the offset based on the time.
                let chapter_duration = chapter_timespan.end - chapter_timespan.start;
                let chapter_ns = chapter_duration.whole_nanoseconds() as u64;

                let time_offset = time - chapter_timespan.start;
                let time_offset_ns = time_offset.whole_nanoseconds() as u64;

                let events = c.event_count() as u64;

                let est_offset = time_offset_ns * events / chapter_ns;
                return offset + est_offset;
            }
        }
        return offset;
    }
}

/// An iterator over the events in a tape.
///
/// # Note
/// Calling `next` on this iterator will load the data for the next chapter into memory. Because
/// the data is loaded in a background thread, the iterator yields Option<&Event> instead of
/// &Event. If the data for the next chapter is not yet loaded, `next` will return Some(None).
/// To avoid loading the data for a chapter, use the `skip_n` method.
pub struct RecordedEvents<'a> {
    chapters: &'a [Chapter],
    event_index: usize,
}

impl RecordedEvents<'_> {
    // pub fn current_chapter(&self) -> Option<&Chapter> {
    //     // self.chapters.get(self.chapter_index)
    // }
    pub fn remaining_len(&self) -> usize {
        let len: usize = self.chapters.iter().map(|c| c.event_count()).sum();
        len - self.event_index
    }

    pub fn skip_n(&mut self, n: usize) {
        let mut remaining = n;
        while remaining > 0 {
            if self.event_index + remaining < self.chapters[0].event_count() {
                self.event_index += remaining;
                remaining = 0;
            } else {
                remaining -= self.chapters[0].event_count() - self.event_index;
                self.chapters = &self.chapters[1..];
                self.event_index = 0;
            }
        }
    }
}

impl<'a> Iterator for RecordedEvents<'a> {
    type Item = Option<&'a Event<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(chapter) = self.chapters.first() {
            if self.event_index >= chapter.event_count() {
                self.chapters = &self.chapters[1..];
                self.event_index = 0;
            } else {
                if let Some(data) = chapter.data() {
                    let event = &data.records.events[self.event_index];
                    self.event_index += 1;
                    return Some(Some(event));
                } else {
                    self.event_index += 1;
                    return Some(None);
                }
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.remaining_len();
        (len, Some(len))
    }
}
