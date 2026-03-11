# Hawkbot

Hawkbot is a Rust project for decoding and working with Daktronics All Sport 5000 serial scoreboard data.

It includes:
- Packet decoding utilities
- Real-time state tracking (`RTDState`)
- Sport-specific parsers and accessors
- A built-in web interface for live status and admin controls
- Example programs for reading and inspecting packet data

## Getting started

1. Install Rust and Cargo.
2. Build the project:

```bash
cargo build
```

## Web interface

Hawkbot runs a local web server by default.

Start it with:

```bash
cargo run
```

Then open:
- Dashboard: `http://localhost:8080/`
- Admin: `http://localhost:8080/admin`
- Public JSON status feed: `http://localhost:8080/status/<public_status_uuid>.json`

Configuration is stored in `hawkbot-web-config.json` (or the path set in `HAWKBOT_CONFIG`).
By default it binds to `0.0.0.0:8080`.

## CLI example

You can still run packet-level examples directly:

```bash
cargo run --example read_raw_packet
```

## Project structure

- `src/` – core library, sports modules, and web server
- `examples/` – runnable examples
- `sports_data/` – source data and conversion helpers

## Credit

This project is based on and inspired by:
https://github.com/zabackary/daktronics-allsport-5000-rs

Huge thanks to the original author and contributors for the foundational work.
