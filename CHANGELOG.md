# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- This CHANGELOG file that will contain all notable changes to this project ([#2](https://github.com/soehrl/tracing-tape/pull/2/))

### Fixed
- Parsing of *SpanExit* records ([#3](https://github.com/soehrl/tracing-tape/pull/3/))
- Parsing of Threads only used for spans ([#4](https://github.com/soehrl/tracing-tape/pull/4/))
- Reported tape time range ([#5](https://github.com/soehrl/tracing-tape/pull/5/))


## [0.1.0] - 2024-11-10

### Added
- `tracing-tape` crate specifying the binary format
- `tracing-tape-recorder` crate for recording traces
- `tracing-tape-parser` crate for parsing traces
- `trace-deck` application for viewing traces
- README file
- CI for testing crates and trace-deck deployment ([#1](https://github.com/soehrl/tracing-tape/pull/2/))
