---
name: ghub-websocket
description: Unofficial Logitech G HUB local WebSocket API reference for reading device battery state — endpoint, subprotocol, JSON envelope, paths, response shapes, fragility caveats. Read before writing or modifying src-tauri/src/battery/ghub.rs (the fallback battery source).
---

# G HUB local WebSocket API (unofficial, fallback source)

Served by `lghub_agent.exe` — **only available while G HUB is running**. Verified against LGSTrayBattery (`LGSTrayCore/Managers/GHubManager.cs`).

## Connection

- Endpoint: `ws://localhost:9010`
- WebSocket subprotocol **must** be `json` (`Sec-WebSocket-Protocol: json`) — handshake is rejected without it.
- If connection fails ⇒ G HUB not running ⇒ this source reports "unavailable"; never block or crash the poller on it.

## Message envelope

Every request/response is JSON: `{ msgId, verb, path, origin?, result?, payload? }`. `msgId` may be `""`. Responses echo the request `path`; match responses by path (LGSTrayBattery regex-matches `/battery/devN/state`).

## Requests

```json
{ "msgId": "", "verb": "GET",       "path": "/devices/list" }
{ "msgId": "", "verb": "GET",       "path": "/battery/dev0/state" }
{ "msgId": "", "verb": "SUBSCRIBE", "path": "/devices/state/changed" }
{ "msgId": "", "verb": "SUBSCRIBE", "path": "/battery/state/changed" }
```

After SUBSCRIBE, the agent pushes messages with `path: "/battery/state/changed"` on every battery change — prefer subscribe + initial GET sweep over polling.

## Response shapes

`/devices/list` → `payload.deviceInfos[]`, each:
- `id` — e.g. `"dev0"`
- `deviceType` — Mouse / Keyboard / Headset …
- `extendedDisplayName` — human-readable name (NOT `displayName`)
- `capabilities.hasBatteryStatus` — filter on this

Battery state (GET result and pushed changes) → `payload`:
- `deviceId` (string), `percentage` (0–100), `charging` (bool), `mileage` (double, estimated hours left)

## Caveats

- Unofficial; LGSTrayBattery's README warns the IPC protocol/endpoints may change in future G HUB versions. Keep ALL G HUB knowledge inside `ghub.rs` behind the `BatterySource` trait; degrade gracefully (source unavailable ≠ error dialog).
- G HUB covers G-series gaming devices. Devices managed by **Logi Options+** are not here — its agent (`logioptionsplus_agent`, local WS port 19010) has no verified battery API; for those devices the HID++ source is the only proven route.
- Reconnect with backoff: G HUB restarts itself during updates.

## References

- github.com/andyvorld/LGSTrayBattery — `LGSTrayCore/Managers/GHubManager.cs`
- github.com/andyvorld/LGSTrayBattery_GHUB_dump — raw captured WS traffic
- github.com/bmrussell/LGBattery — Python implementation of same protocol
