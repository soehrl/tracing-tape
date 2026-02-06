//! # Tracing Tape Parser
//! This craet provides parsing functionality for the `tracing-tape` format.
//! It is mainly used in the trace-deck crate to parse the tape files.

use std::{fmt::Display, sync::Arc};

use ahash::HashMap;
use petgraph::{graph::NodeIndex, Graph};
use tracing_tape::{
    intro::Intro,
    record::{
        field_type, record_kind, CallsiteFieldRecord, CallsiteRecord, EventRecord,
        EventValueRecord, RecordHeader, SpanCloseRecord, SpanEnterRecord, SpanExitRecord,
        SpanOpenRecord, SpanOpenRecord2, SpanValueRecord,
    },
};
use zerocopy::FromBytes;

#[derive(Debug)]
pub enum Value {
    Bool(bool),
    I64(i64),
    U64(u64),
    I128(i128),
    U128(u128),
    F64(f64),
    String(Arc<str>),
    Error(Arc<str>),
}

impl Value {
    fn parse(kind: u8, data: &[u8]) -> Self {
        match kind {
            field_type::BOOL => Value::Bool(data[0] != 0),
            field_type::I64 => Value::I64(i64::from_le_bytes(data.try_into().unwrap())),
            field_type::U64 => Value::U64(u64::from_le_bytes(data.try_into().unwrap())),
            field_type::I128 => Value::I128(i128::from_le_bytes(data.try_into().unwrap())),
            field_type::U128 => Value::U128(u128::from_le_bytes(data.try_into().unwrap())),
            field_type::F64 => Value::F64(f64::from_le_bytes(data.try_into().unwrap())),
            field_type::STR => {
                let value = Arc::from(String::from_utf8_lossy(data));
                Value::String(value)
            }
            field_type::ERROR => {
                let value = Arc::from(String::from_utf8_lossy(data));
                Value::Error(value)
            }
            _ => {
                panic!("unknown field type: {}", kind);
            }
        }
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Bool(value) => value.fmt(f),
            Value::I64(value) => value.fmt(f),
            Value::U64(value) => value.fmt(f),
            Value::I128(value) => value.fmt(f),
            Value::U128(value) => value.fmt(f),
            Value::F64(value) => value.fmt(f),
            Value::String(value) => value.fmt(f),
            Value::Error(value) => value.fmt(f),
        }
    }
}

#[derive(Debug, Default)]
struct IntermediateThread {
    name: Option<String>,
    entrances: Graph<SpanEntrance, (), petgraph::Directed, usize>,
    root_entrances: Vec<NodeIndex<usize>>,
    context: Vec<NodeIndex<usize>>,
}

impl IntermediateThread {
    fn finish(self, max_timestamp: i64) -> Thread {
        let mut entrances = self.entrances;
        for entrance_index in self.context {
            entrances[entrance_index].exited = max_timestamp;
        }

        Thread {
            name: self.name,
            entrances,
            root_entrances: self.root_entrances,
        }
    }
}

#[derive(Debug)]
pub struct Thread {
    name: Option<String>,
    entrances: Graph<SpanEntrance, (), petgraph::Directed, usize>,
    root_entrances: Vec<NodeIndex<usize>>,
}

impl Thread {
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn root_entrances(&self) -> &[NodeIndex<usize>] {
        &self.root_entrances
    }

    pub fn entrances(&self) -> &Graph<SpanEntrance, (), petgraph::Directed, usize> {
        &self.entrances
    }
}

#[derive(Debug)]
struct Intermediate {
    min_timestamp: i64,
    max_timestamp: i64,

    /// Callsites where not all fields have been parsed yet.
    intermediate_callsites: HashMap<u64, IntermediateCallsite>,

    /// Fully parsed callsites.
    callsites: Vec<IntermediateCallsite>,

    /// Events where not all values have been parsed yet.
    ///
    /// The key is the thread_id.
    intermediate_events: HashMap<u64, IntermediateEvent>,

    /// Complete events.
    events: Vec<IntermediateEvent>,

    /// Closed spans.
    spans: Vec<IntermediateSpan>,

    /// Open spans.
    open_spans: HashMap<u64, usize>,

    /// All discovered threads.
    threads: HashMap<u64, IntermediateThread>,
}

impl Default for Intermediate {
    fn default() -> Self {
        Self {
            min_timestamp: i64::MAX,
            max_timestamp: i64::MIN,

            intermediate_callsites: HashMap::default(),
            callsites: Vec::new(),

            intermediate_events: HashMap::default(),
            events: Vec::new(),

            spans: Vec::new(),
            open_spans: HashMap::default(),
            threads: HashMap::default(),
        }
    }
}

impl Intermediate {
    fn callsite<'a>(&mut self, slice: &'a [u8]) -> &'a [u8] {
        let (callsite, remaining) = IntermediateCallsite::parse(slice);

        if callsite.fields.capacity() == 0 {
            self.callsites.push(callsite);
        } else {
            self.intermediate_callsites.insert(callsite.id, callsite);
        }
        remaining
    }

    fn callsite_field<'a>(&mut self, slice: &'a [u8]) -> &'a [u8] {
        let callsite_field_record = CallsiteFieldRecord::ref_from_prefix(slice).unwrap();

        let name_len = callsite_field_record.field_name_len.get() as usize;
        let offset = std::mem::size_of::<CallsiteFieldRecord>();
        let name = &slice[offset..offset + name_len];
        let name = Arc::from(String::from_utf8_lossy(name));

        let callsite_id = callsite_field_record.callsite_id.get();
        let mut callsite = self.intermediate_callsites.remove(&callsite_id).unwrap();
        callsite.fields.push(Field {
            name,
            id: callsite_field_record.field_id.get(),
        });
        if callsite.fields.len() == callsite.fields.capacity() {
            self.callsites.push(callsite);
        } else {
            self.intermediate_callsites.insert(callsite_id, callsite);
        }

        &slice[callsite_field_record.header.len.get() as usize..]
    }

    fn event<'a>(&mut self, slice: &'a [u8]) -> &'a [u8] {
        let event_record = EventRecord::ref_from_prefix(slice).unwrap();

        // TODO: change once try_insert is stable
        self.threads
            .entry(event_record.thread_id.get())
            .or_default();

        let event = IntermediateEvent {
            timestamp: event_record.timestamp.get(),
            callsite_id: event_record.callsite_id.get(),
            values: Vec::with_capacity(event_record.value_count.get() as usize),
        };

        let thread_id = event_record.thread_id.get();
        assert!(!self.intermediate_events.contains_key(&thread_id));

        if event.values.capacity() == 0 {
            self.events.push(event);
        } else {
            self.intermediate_events.insert(thread_id, event);
        }

        self.min_timestamp = self.min_timestamp.min(event_record.timestamp.get());
        self.max_timestamp = self.max_timestamp.max(event_record.timestamp.get());

        &slice[event_record.header.len.get() as usize..]
    }

    fn event_value<'a>(&mut self, slice: &'a [u8]) -> &'a [u8] {
        let event_value_record = EventValueRecord::ref_from_prefix(slice).unwrap();

        let value_len =
            event_value_record.header.len.get() as usize - std::mem::size_of::<EventValueRecord>();
        let value = &slice[std::mem::size_of::<EventValueRecord>()..][..value_len];

        let kind = event_value_record.kind;
        let value = Value::parse(kind, value);

        let thread_id = event_value_record.thread_id.get();
        let mut event = self.intermediate_events.remove(&thread_id).unwrap();

        // TODO: use push_within_capacity once it's stable
        event.values.push(IntermediateValue {
            value,
            field_id: event_value_record.field_id.get(),
        });

        if event.values.len() == event.values.capacity() {
            self.events.push(event);
        } else {
            self.intermediate_events.insert(thread_id, event);
        }

        &slice[event_value_record.header.len.get() as usize..]
    }

    fn open_span<'a>(&mut self, slice: &'a [u8]) -> &'a [u8] {
        let span_record = if slice.len() >= std::mem::size_of::<SpanOpenRecord2>() {
            SpanOpenRecord2::ref_from_prefix(slice).unwrap().clone()
        } else if slice.len() >= std::mem::size_of::<SpanOpenRecord>() {
            SpanOpenRecord::ref_from_prefix(slice)
                .unwrap()
                .clone()
                .into()
        } else {
            panic!("invalid span record");
        };

        println!("{:?}", span_record);

        let span = IntermediateSpan {
            id: span_record.span_open_record.id.get(),
            opened: span_record.span_open_record.timestamp.get(),
            closed: 0,
            callsite_id: span_record.span_open_record.callsite_id.get(),
            parent: Parent::Contextual,
            values: HashMap::default(),
            entered_thread: None,
        };

        self.min_timestamp = self
            .min_timestamp
            .min(span_record.span_open_record.timestamp.get());
        self.max_timestamp = self
            .max_timestamp
            .max(span_record.span_open_record.timestamp.get());

        let span_id = span.id;
        let span_index = self.spans.len();
        self.spans.push(span);
        self.open_spans.insert(span_id, span_index);

        &slice[span_record.span_open_record.header.len.get() as usize..]
    }

    fn enter_span<'a>(&mut self, slice: &'a [u8]) -> &'a [u8] {
        let span_enter_record = SpanEnterRecord::ref_from_prefix(slice).unwrap();

        println!("{:?}", span_enter_record);

        let thread_id = span_enter_record.thread_id.get();
        let thread = self.threads.entry(thread_id).or_default();
        let span_index = self.open_spans[&span_enter_record.id.get()];
        self.spans[span_index].entered_thread = Some(thread_id);

        let entrance = SpanEntrance {
            entered: span_enter_record.timestamp.get(),
            exited: span_enter_record.timestamp.get(),
            span_index,
        };

        let entrance_index = thread.entrances.add_node(entrance);
        if let Some(containing) = thread.context.last() {
            thread.entrances.add_edge(*containing, entrance_index, ());
        } else {
            thread.root_entrances.push(entrance_index);
        }
        thread.context.push(entrance_index);

        &slice[span_enter_record.header.len.get() as usize..]
    }

    fn exit_span<'a>(&mut self, slice: &'a [u8]) -> &'a [u8] {
        let span_exit_record = SpanExitRecord::ref_from_prefix(slice).unwrap();

        println!("{:?}", span_exit_record);

        let span_index = self.open_spans[&span_exit_record.id.get()];
        if let Some(thread_id) = self.spans[span_index].entered_thread.take() {
            let thread = self.threads.get_mut(&thread_id).unwrap();
            let entrance_index = thread.context.pop().unwrap();
            let entrance = thread.entrances.node_weight_mut(entrance_index).unwrap();
            entrance.exited = span_exit_record.timestamp.get();
        } else {
            panic!("span exited without being entered");
        }

        &slice[span_exit_record.header.len.get() as usize..]
    }

    fn close_span<'a>(&mut self, slice: &'a [u8]) -> &'a [u8] {
        let span_record = SpanCloseRecord::ref_from_prefix(slice).unwrap();

        println!("{:?}", span_record);

        self.min_timestamp = self.min_timestamp.min(span_record.timestamp.get());
        self.max_timestamp = self.max_timestamp.max(span_record.timestamp.get());

        let span_index = self.open_spans.remove(&span_record.id.get()).unwrap();
        let span = &mut self.spans[span_index];
        span.closed = span_record.timestamp.get();

        &slice[span_record.header.len.get() as usize..]
    }

    fn span_value<'a>(&mut self, slice: &'a [u8]) -> &'a [u8] {
        let span_value_record = SpanValueRecord::ref_from_prefix(slice).unwrap();

        let value_len =
            span_value_record.header.len.get() as usize - std::mem::size_of::<SpanValueRecord>();
        let value = &slice[std::mem::size_of::<SpanValueRecord>()..][..value_len];

        let kind = span_value_record.kind;
        let value = Value::parse(kind, value);

        let span_id = span_value_record.span_id.get();
        let index = self.open_spans[&span_id];
        let span = &mut self.spans[index];
        span.values.insert(span_value_record.field_id.get(), value);

        &slice[span_value_record.header.len.get() as usize..]
    }

    fn parse(&mut self, mut data: &[u8]) -> Result<(), u8> {
        while !data.is_empty() {
            let record_kind = data[0];

            match record_kind {
                record_kind::NOOP => {
                    data = &data[1..];
                }
                record_kind::CALLSITE => {
                    data = self.callsite(data);
                }
                record_kind::CALLSITE_FIELD => {
                    data = self.callsite_field(data);
                }
                record_kind::SPAN_OPEN => {
                    data = self.open_span(data);
                }
                record_kind::SPAN_ENTER => {
                    data = self.enter_span(data);
                }
                record_kind::SPAN_EXIT => {
                    data = self.exit_span(data);
                }
                record_kind::SPAN_CLOSE => {
                    data = self.close_span(data);
                }
                record_kind::SPAN_VALUE => {
                    data = self.span_value(data);
                }
                record_kind::EVENT => {
                    data = self.event(data);
                }
                record_kind::EVENT_VALUE => {
                    data = self.event_value(data);
                }
                _ => {
                    let header = RecordHeader::ref_from_prefix(data).unwrap();
                    data = &data[header.len.get() as usize..];
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
struct IntermediateValue {
    value: Value,
    field_id: u64,
}

#[derive(Debug)]
struct IntermediateEvent {
    timestamp: i64,
    callsite_id: u64,
    values: Vec<IntermediateValue>,
}

#[derive(Debug)]
enum Parent {
    #[allow(dead_code)]
    Root,
    #[allow(dead_code)]
    Explicit(u64),
    Contextual,
}

#[derive(Debug)]
struct IntermediateSpan {
    id: u64,
    opened: i64,
    closed: i64,
    callsite_id: u64,
    parent: Parent,
    values: HashMap<u64, Value>,
    entered_thread: Option<u64>,
}

#[derive(Debug)]
pub struct SpanEntrance {
    pub entered: i64,
    pub exited: i64,
    pub span_index: usize,
}

#[derive(Debug)]
pub struct Span {
    pub opened: i64,
    pub closed: i64,
    pub callsite_index: usize,
    pub values: Arc<[Value]>,
}

#[derive(Debug)]
pub struct Event {
    pub timestamp: i64,
    pub callsite_index: usize,
    pub values: Arc<[Value]>,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct Field {
    name: Arc<str>,
    id: u64,
}

#[derive(Debug)]
struct IntermediateCallsite {
    id: u64,
    kind: tracing::metadata::Kind,
    level: tracing::Level,
    name: Arc<str>,
    target: Arc<str>,
    module_path: Arc<str>,
    file: Option<Arc<str>>,
    line: Option<u32>,
    fields: Vec<Field>,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct Metadata {
    pub level: tracing::Level,
    pub name: Arc<str>,
    pub target: Arc<str>,
    pub module_path: Arc<str>,
    pub file: Option<Arc<str>>,
    pub line: Option<u32>,
    pub fields: Arc<[Arc<str>]>,
}

impl From<IntermediateCallsite> for Metadata {
    fn from(value: IntermediateCallsite) -> Self {
        Self {
            level: value.level,
            name: value.name,
            target: value.target,
            module_path: value.module_path,
            file: value.file,
            line: value.line,
            fields: value
                .fields
                .into_iter()
                .map(|field| field.name)
                .collect::<Vec<_>>()
                .into(),
        }
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub enum Callsite {
    Event(Metadata),
    Span(Metadata),
}

impl Callsite {
    pub fn metadata(&self) -> &Metadata {
        match self {
            Self::Event(metadata) => metadata,
            Self::Span(metadata) => metadata,
        }
    }

    pub fn kind(&self) -> tracing::metadata::Kind {
        match self {
            Self::Event(_) => tracing::metadata::Kind::EVENT,
            Self::Span(_) => tracing::metadata::Kind::SPAN,
        }
    }

    pub fn level(&self) -> tracing::Level {
        self.metadata().level
    }

    pub fn name(&self) -> &str {
        &self.metadata().name
    }

    pub fn target(&self) -> &str {
        &self.metadata().target
    }

    pub fn module_path(&self) -> &str {
        &self.metadata().module_path
    }

    pub fn file(&self) -> Option<&str> {
        self.metadata().file.as_deref()
    }

    pub fn line(&self) -> Option<u32> {
        self.metadata().line
    }

    pub fn fields(&self) -> &[Arc<str>] {
        &self.metadata().fields
    }
}

impl From<IntermediateCallsite> for Callsite {
    fn from(value: IntermediateCallsite) -> Self {
        if value.kind == tracing::metadata::Kind::SPAN {
            Self::Span(value.into())
        } else {
            Self::Event(value.into())
        }
    }
}

impl IntermediateCallsite {
    fn parse(slice: &[u8]) -> (Self, &[u8]) {
        let callsite_record = CallsiteRecord::ref_from_prefix(slice).unwrap();

        let remaining = &slice[callsite_record.header.len.get() as usize..];

        let slice = &slice[std::mem::size_of::<CallsiteRecord>()..];
        let (name, slice) = slice.split_at(callsite_record.name_len.get() as usize);
        let (target, slice) = slice.split_at(callsite_record.target_len.get() as usize);
        let (module_path, slice) = slice.split_at(callsite_record.module_path_len.get() as usize);
        let (file, _) = slice.split_at(callsite_record.file_len.get() as usize);

        let name = Arc::from(String::from_utf8_lossy(name));
        let target = Arc::from(String::from_utf8_lossy(target));
        let module_path = Arc::from(String::from_utf8_lossy(module_path));
        let file = if file.is_empty() {
            None
        } else {
            Some(Arc::from(String::from_utf8_lossy(file)))
        };
        let line = if callsite_record.line.get() == 0 {
            None
        } else {
            Some(callsite_record.line.get())
        };

        let callsite = Self {
            id: callsite_record.id.get(),
            kind: callsite_record.info.kind().expect("invalid kind"),
            level: callsite_record.info.level().expect("invalid level"),
            name,
            target,
            module_path,
            file,
            line,
            fields: Vec::with_capacity(callsite_record.field_count.get() as usize),
        };

        (callsite, remaining)
    }
}

#[derive(Debug)]
pub struct TapeData {
    min_timestamp: i64,
    max_timestamp: i64,
    callsites: Vec<Callsite>,
    events: Vec<Event>,
    spans: Vec<Span>,
    threads: HashMap<u64, Thread>,
}

impl TapeData {
    fn new(intermediate: Intermediate) -> Self {
        let mut callsite_map = HashMap::default();
        let mut callsite_field_map = HashMap::default();
        let callsites = intermediate
            .callsites
            .into_iter()
            .enumerate()
            .map(|(index, callsite)| {
                callsite_map.insert(callsite.id, index);

                for (index, field) in callsite.fields.iter().enumerate() {
                    callsite_field_map.insert((callsite.id, field.id), index);
                }

                callsite.into()
            })
            .collect::<Vec<_>>();

        let mut events = intermediate.events;
        events.sort_by_key(|event| event.timestamp);
        let events = events
            .into_iter()
            .map(|event| {
                let mut values = event.values;
                values.sort_by_cached_key(|value| {
                    callsite_field_map[&(event.callsite_id, value.field_id)]
                });
                let values = values
                    .into_iter()
                    .map(|value| value.value)
                    .collect::<Vec<_>>();

                Event {
                    timestamp: event.timestamp,
                    callsite_index: callsite_map[&event.callsite_id],
                    values: Arc::from(values.into_boxed_slice()),
                }
            })
            .collect();

        let spans = intermediate
            .spans
            .into_iter()
            .map(|span| {
                let mut values = span.values.into_iter().collect::<Vec<_>>();
                values.sort_by_cached_key(|(field_id, _)| {
                    callsite_field_map[&(span.callsite_id, *field_id)]
                });
                let value = values
                    .into_iter()
                    .map(|(_, value)| value)
                    .collect::<Vec<_>>();

                Span {
                    callsite_index: callsite_map[&span.callsite_id],
                    opened: span.opened,
                    closed: span.closed,
                    values: Arc::from(value.into_boxed_slice()),
                }
            })
            .collect();

        let threads = intermediate
            .threads
            .into_iter()
            .map(|(thread_id, thread)| (thread_id, thread.finish(intermediate.max_timestamp)))
            .collect();

        Self {
            min_timestamp: intermediate.min_timestamp,
            max_timestamp: intermediate.max_timestamp,
            callsites,
            events,
            spans,
            threads,
        }
    }
}

#[derive(Debug)]
pub struct Tape {
    intro: Intro,
    data: TapeData,
}

impl Tape {
    pub fn parse(data: &[u8]) -> Self {
        let intro = Intro::read_from_prefix(data).unwrap();

        let mut intermediate = Intermediate::default();
        intermediate
            .parse(&data[std::mem::size_of::<Intro>()..])
            .unwrap();

        let data = TapeData::new(intermediate);

        Self { intro, data }
    }

    pub fn time_range(&self) -> std::ops::RangeInclusive<i128> {
        let start = self.intro.timestamp_base.get();
        let end = start + self.data.max_timestamp as i128;
        start..=end
    }

    pub fn timestamp_range(&self) -> std::ops::RangeInclusive<i64> {
        self.data.min_timestamp..=self.data.max_timestamp
    }

    pub fn events(&self) -> &[Event] {
        &self.data.events
    }

    pub fn callsites(&self) -> &[Callsite] {
        &self.data.callsites
    }

    pub fn spans(&self) -> &[Span] {
        &self.data.spans
    }

    pub fn threads(&self) -> &HashMap<u64, Thread> {
        &self.data.threads
    }
}
