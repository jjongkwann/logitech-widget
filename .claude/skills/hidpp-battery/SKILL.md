---
name: hidpp-battery
description: Logitech HID++ protocol reference for reading device battery status over hidapi on Windows — report formats, feature discovery, battery features 0x1000/0x1001/0x1004, HID++ 1.0 registers, enumeration quirks, sleep/error handling. Read before writing or modifying any code under src-tauri/src/battery/hidpp/.
---

# HID++ battery protocol (verified against Solaar, Linux kernel hid-logitech-hidpp.c, LGSTrayBattery)

## Message framing

| Kind  | Report ID | Total bytes | Layout |
|-------|-----------|-------------|--------|
| Short | `0x10`    | 7  | `[0x10, dev_idx, feat_idx/sub_id, (fn<<4)\|sw_id, p0, p1, p2]` |
| Long  | `0x11`    | 20 | same header + 16 param bytes |

- `dev_idx`: `0xFF` = receiver itself or direct-connected (wired/Bluetooth) device; `1..6` = paired slot on a Unifying/Bolt/Lightspeed receiver.
- `sw_id` is a 4-bit nibble echoed in responses — use it to match replies. Pick a fixed nonzero value (e.g. `0x0A` like LGSTrayBattery). **Never 0** (reserved for device-initiated events).
- Replies may come back on a different report length than the request. Match on `(dev_idx, feat_idx, fn|sw_id)`, never on report ID. Some BT devices echo dev_idx `0x00` instead of the one sent — accept both.
- Unsolicited events (battery broadcasts, `0x41` connection notifications) interleave with replies; the read loop must filter, not assume next-read-is-my-reply.

## Errors

- HID++ 1.0 error: report `0x10`, byte2 = `0x8F`, echoed sub_id/address at bytes 3–4, error code at byte 5. Codes: `0x01` INVALID_SUBID, `0x02` INVALID_ADDRESS, `0x03` INVALID_VALUE, `0x04` CONNECT_FAIL, `0x07` BUSY, `0x08` UNKNOWN_DEVICE, `0x09` RESOURCE_ERROR.
- HID++ 2.0 error: report `0x11`, byte2 = `0xFF`, echoed feat_idx + fn|sw_id, then error code.
- **Sleeping device**: the receiver answers on its behalf with `0x8F`, typically code `0x09` (also `0x08`, `0x04`). Treat as *offline*, surface "last known %", do not hot-retry.

## HID++ 2.0 flow (modern devices)

1. **Ping / version probe** — IRoot (feature index `0x00`), function 1: `[0x10, idx, 0x00, 0x10|sw, 0, 0, marker]`. Response p0.p1 = protocol version; p2 echoes `marker`. An `0x8F` error code `0x01` ⇒ device is HID++ 1.0 → use register protocol below. LGSTrayBattery requires 3 successful pings (of ≤10 attempts) before init — copy that.
2. **Feature lookup** — IRoot function 0: params = feature ID as BE16, e.g. `[0x10, idx, 0x00, 0x00|sw, 0x10, 0x04, 0]` for 0x1004. Response p0 = feature **index** (0 ⇒ unsupported), p1 = flags, p2 = version. Probe in order: **0x1004 → 0x1001 → 0x1000**; use the first supported.
3. **Device name/type** — feature `0x0005`: fn0 = name length (p0); fn1 with p0 = offset returns up to 16 name bytes per call (loop); fn2 = type enum (0 keyboard, 3 mouse, 4 touchpad, 5 trackball, 7 receiver, …).

### 0x1004 UNIFIED_BATTERY (preferred)
- fn0 `get_capabilities`: p1 bit1 set ⇒ state_of_charge (percentage) supported.
- fn1 `get_status` (`byte3 = 0x10|sw`): p0 = % (0–100), p1 = level bits (1 critical, 2 low, 4 good, 8 full), p2 = charging (0 discharging, 1 charging, 2 charging-slow, 3 full, 4 error), p3 = external power.
- Event `0x00` broadcasts the same layout when state changes.

### 0x1001 BATTERY_VOLTAGE
- fn0: p0–p1 = millivolts BE16; p2 bit7 set = external power (then bits0–2: 0 charging, 1 full, 2 error), bit7 clear = discharging.
- No percentage — convert mV→% with a Li-ion lookup table (LGSTrayBattery `Battery1001.cs` has a 100-entry `_mvLUT`; port it).

### 0x1000 BATTERY_STATUS
- fn0 `getBatteryLevelStatus`: p0 = % (0 ⇒ device doesn't report %), p1 = next level, p2 status: 0 discharging, 1 recharging, 2 final-stage, 3 full, 4 slow-recharge; ≥5 treat as not-charging/error.
- Event `0x00` broadcasts same.

## HID++ 1.0 (old Unifying-era devices) — register protocol

- Read short register: `[0x10, idx, 0x81, reg, 0, 0, 0]`; long read uses sub_id `0x83`.
- Battery: try reg `0x0D` (p0 = %, p2&0xF0: `0x30` discharging / `0x50` recharging / `0x90` full), else reg `0x07` (p0 level code 7 full / 5 good / 3 low / 1 critical).
- Receiver (dev_idx `0xFF`):
  - Enumerate paired devices: **write** reg `0x02` = `[0x10, 0xFF, 0x80, 0x02, 0x02, 0, 0]` → receiver emits a fake `0x41` connection notification per paired device. In a `0x41`, `byte4 & 0x40` set = link NOT established (offline).
  - Pairing info: long-read reg `0xB5` with p0 = `0x20|(n-1)` (wpid bytes [3:5], kind byte[7]&0x0F), `0x40|(n-1)` (codename: len at [1], ASCII at [2..]).
  - Reg `0x00`: write notification flags to receive battery/connection events.

## Windows / hidapi enumeration

- Filter: VID `0x046D` + `(usage_page & 0xFF00) == 0xFF00`. Usage `0x0001` ⇒ short channel, `0x0002` ⇒ long channel. Bluetooth devices appear under usage page `0xFF43`, usage `0x0202`, **long reports only** — send everything as 20-byte `0x11` there. No PID whitelist.
- Windows splits each HID collection into its own device path; keyboard/mouse collections are OS-exclusive — only the vendor collection is openable.
- One physical receiver ⇒ two openable paths (short + long). Group them per physical device (container ID, or path/serial prefix; `DeviceInfo::usage_page()/usage()` work on Windows in the `hidapi` crate).
- `write()` buffer = report ID + payload (7 or 20 bytes total), written to the matching short/long handle. Use `read_timeout()` (~100 ms per transaction) and serialize write→read per device with a mutex.
- Wired quirk: some devices (e.g. G403 wired) answer on every dev_idx — dedupe by device identity, not by index.

## Rust ecosystem note

No mature Windows-proven HID++ crate exists (crates `hidpp`, `hidpp-transport` are young/unproven). Implement the ~300-line protocol directly over `hidapi`, mirroring LGSTrayBattery's structure. Keep all byte parsing in pure functions over `&[u8]` with fixture-based unit tests.

## Primary references

- Solaar: `lib/logitech_receiver/{base,hidpp10,hidpp20,receiver}.py` (github.com/pwr-Solaar/Solaar)
- LGSTrayBattery: `LGSTrayHID/` esp. `HidppDevices.cs`, `Features/Battery100{0,1,4}.cs` (github.com/andyvorld/LGSTrayBattery)
- Linux kernel: `drivers/hid/hid-logitech-hidpp.c`
- Logitech official HID++ 2.0 docs: github.com/Logitech/cpg-docs
