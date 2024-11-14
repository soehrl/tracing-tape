# Tracing Tape
Dead-simple debugging and profiling of (distributed) Rust applications using the [tracing](https://docs.rs/tracing) crate.
Record trace files and view them within within seconds without complex setup or configuration.

[![Trace Deck Screenshot](https://github.com/soehrl/tracing-tape/blob/main/trace-deck.png)](https://github.com/soehrl/tracing-tape/blob/main/trace-deck.png)

## Setup
1. Add the following dependencies to your application:
```
cargo add tracing tracing-subscriber tracing-tape-recorder
```
2. Add the following code to your application:
```rust
use tracing::trace_span;
use tracing_subscriber::{fmt, layer::SubscriberExt, Registry};
use tracing_tape_recorder::TapeRecorder;

let subscriber = Registry::default().with(TapeRecorder::default())
let guard = tracing::subscriber::set_default(subscriber);

// ...

drop(guard);
```
Running your application will now generate a `{name}-{timestamp}.tape` file in the current working directory.

**Note:** it is preferred to use `set_default` instead of `set_global_default` to ensure the subsriber is dropped when the guard goes out of scope.
See [#7](https://github.com/soehrl/tracing-tape/issues/7) for more information.

## Viewing Tape Files
You can use the `trace-deck` application to view the recorded tape files either by running `trace-deck filename.tape` or by dragging the files into the window.
You can load multiple files simultaneously which can be useful for analyzing workflows across multiple applications (e.g., client-server interactions).
Have a look at the [getting started guide](https://github.com/soehrl/tracing-tape/wiki/Getting-Started).

## Crates
- tracing-tape: defines the format of the tape files.
- tracing-tape-recorder: records trace events to tape files.
- tracing-tape-parser: parses recorded tape files.
- trace-deck: GUI application for viewing tape files.

## Known Issues
- Currently there is no way, to configure the tape recorder ([#6](https://github.com/soehrl/tracing-tape/issues/6), [#8](https://github.com/soehrl/tracing-tape/issues/8)).
- Recent data is lost when the tape recorder is not properly dropped ([#7](https://github.com/soehrl/tracing-tape/issues/7)).
- Loading large tape files can be slow ([#9](https://github.com/soehrl/tracing-tape/issues/9)).
- Recording tape files will occasionally cause lag spikes ([#10](https://github.com/soehrl/tracing-tape/issues/10)).
