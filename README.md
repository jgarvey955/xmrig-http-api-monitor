# XMRIGCC-Proxy Monitor
To Donate SAL1=SC11UA22DFrAQerDwJwcf8Yh2ySTb7ipaFL8qSEX26tqUDdPf1RQBmmRuZG4SnRd8DNpp5vE1zDHnKNStiFDQsce49Q7fyp8Yp

Rust desktop monitor for the xmrigcc-proxy HTTP API.

The app is built with `iced` and opens a native window titled `XMRIGCC-Proxy Monitor`. It now uses a single HTTP API connection model instead of separate daemon and wallet RPC checks.

## Current Behavior

- The health check is `GET /1/summary`
- Connection state is `Connected` when `GET /1/summary` succeeds and `Disconnected` when it fails
- The dashboard shows summary fields parsed from the xmrigcc-proxy HTTP API
- The `XMRIGCC-Proxy API` tab can manually poll documented safe `GET` routes
- API routes and config keys are loaded from [http-api.output](http-api.output)
- Documented write routes are shown in the UI but are not executed automatically
- HTTP and HTTPS are supported
- Bearer-token auth is supported through the `Authorization: Bearer <token>` header
- HTTPS certificate validation is currently relaxed to match the existing local testing workflow

## Build

```bash
cargo build --release
```

The binary is written to:

```bash
target/release/xmrigcc-proxy-monitor
```

## Run

```bash
./target/release/xmrigcc-proxy-monitor
```

You can also run:

```bash
cargo run
```

## Settings

The app stores settings in `settings.json`.

Saved fields:

- API host
- API port
- API transport
- API access token
- Poll frequency
- Preferred manual API route

`Save Settings` verifies `GET /1/summary` before writing the file.

## Views

- `Home`: current summary metrics, base URL, health route, and monitor status
- `XMRIGCC-Proxy API`: safe manual `GET` route polling, config-key reference, and documented write routes
- `Preferences`: API connection settings and poll interval

## Source Reference

The monitor was rewritten against the xmrigcc-proxy HTTP API reference and source tree:

- [http-api.output](http-api.output)
- `/home/jonathan/data/source/xmrigcc-proxy`

## Development Notes

Useful files:

- [src/app.rs](src/app.rs): UI, status polling, route polling, and summary rendering
- [src/rpc.rs](src/rpc.rs): HTTP API client and Bearer-token handling
- [src/settings.rs](src/settings.rs): saved settings model
- [src/inventory.rs](src/inventory.rs): parsing of `http-api.output`
