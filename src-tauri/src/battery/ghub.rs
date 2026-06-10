//! Fallback battery source: Logitech G HUB's local WebSocket API (unofficial —
//! only works while G HUB runs; see .claude/skills/ghub-websocket/SKILL.md).
//! Per poll we open a short-lived connection and issue plain GETs; if anything
//! fails the source quietly reports no devices.

use std::net::TcpStream;
use std::time::Duration;

use serde_json::{json, Value};
use tungstenite::client::IntoClientRequest;
use tungstenite::http::HeaderValue;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

use super::{BatterySource, DeviceBattery};

const SOURCE: &str = "ghub";
const ENDPOINT: &str = "ws://localhost:9010";
const READ_TIMEOUT: Duration = Duration::from_millis(1500);

pub struct GHubSource;

impl BatterySource for GHubSource {
    fn name(&self) -> &'static str {
        SOURCE
    }

    fn poll(&mut self) -> Vec<DeviceBattery> {
        poll_inner().unwrap_or_default()
    }
}

type Ws = WebSocket<MaybeTlsStream<TcpStream>>;

fn poll_inner() -> Option<Vec<DeviceBattery>> {
    let mut ws = open()?;
    let devices = request(&mut ws, "/devices/list")?;
    let mut out = Vec::new();
    for info in devices.get("deviceInfos")?.as_array()? {
        let has_battery = info
            .pointer("/capabilities/hasBatteryStatus")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if !has_battery {
            continue;
        }
        let id = info.get("id").and_then(Value::as_str)?;
        let name = info
            .get("extendedDisplayName")
            .and_then(Value::as_str)
            .unwrap_or(id)
            .to_string();
        let device_type = info
            .get("deviceType")
            .and_then(Value::as_str)
            .unwrap_or("device")
            .to_lowercase();

        let state = request(&mut ws, &format!("/battery/{id}/state"));
        let percentage = state
            .as_ref()
            .and_then(|s| s.get("percentage"))
            .and_then(Value::as_f64);
        let charging = state
            .as_ref()
            .and_then(|s| s.get("charging"))
            .and_then(Value::as_bool)
            .unwrap_or(false);

        out.push(DeviceBattery {
            id: format!("ghub:{id}"),
            name,
            device_type,
            percentage: percentage.map(|p| p.clamp(0.0, 100.0).round() as u8),
            charging,
            online: percentage.is_some(),
            source: SOURCE,
        });
    }
    let _ = ws.close(None);
    Some(out)
}

fn open() -> Option<Ws> {
    let mut req = ENDPOINT.into_client_request().ok()?;
    // G HUB rejects the handshake without this subprotocol.
    req.headers_mut()
        .insert("Sec-WebSocket-Protocol", HeaderValue::from_static("json"));
    let (ws, _) = tungstenite::connect(req).ok()?;
    if let MaybeTlsStream::Plain(stream) = ws.get_ref() {
        let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
    }
    Some(ws)
}

/// GET `path` and read frames until the response echoing that path arrives
/// (the agent pushes unrelated messages in between). Returns the payload.
fn request(ws: &mut Ws, path: &str) -> Option<Value> {
    let msg = json!({ "msgId": "", "verb": "GET", "path": path });
    ws.send(Message::Text(msg.to_string().into())).ok()?;
    // Bounded scan: don't spin forever on a chatty or broken agent.
    for _ in 0..32 {
        let Message::Text(text) = ws.read().ok()? else {
            continue;
        };
        let v: Value = serde_json::from_str(&text).ok()?;
        if v.get("path").and_then(Value::as_str) == Some(path) {
            return v.get("payload").cloned();
        }
    }
    None
}
