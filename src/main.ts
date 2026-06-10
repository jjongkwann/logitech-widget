import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

interface DeviceBattery {
  id: string;
  name: string;
  device_type: string;
  percentage: number | null;
  charging: boolean;
  online: boolean;
  source: string;
}

const app = document.querySelector<HTMLElement>("#app")!;

const TYPE_ICONS: Record<string, string> = {
  mouse: "🖱️",
  keyboard: "⌨️",
  headset: "🎧",
  touchpad: "🖱️",
  trackball: "🖱️",
};

/** Device names come from HID descriptors / receiver registers — escape them. */
function esc(s: string): string {
  return s.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
}

function levelClass(p: number): string {
  if (p >= 50) return "high";
  if (p >= 20) return "mid";
  return "low";
}

function card(d: DeviceBattery): string {
  const icon = TYPE_ICONS[d.device_type] ?? "🔋";
  const pct = d.percentage;
  // Offline devices still show their last known level (filled in by the
  // poller's cache) — BLE mice drop the link within seconds of idling.
  if (!d.online) {
    const bar =
      pct === null
        ? ""
        : `
        <div class="bar">
          <div class="fill ${levelClass(pct)}" style="width:${pct}%"></div>
        </div>`;
    return `
      <div class="card offline">
        <span class="icon">${icon}</span>
        <div class="info">
          <div class="name">${esc(d.name)}</div>
          ${bar}
          <div class="state">절전 중${pct === null ? "" : " · 마지막 잔량"}</div>
        </div>
        ${pct === null ? "" : `<span class="pct">${pct}%</span>`}
      </div>`;
  }
  const bar =
    pct === null
      ? `<div class="state">잔량 미지원</div>`
      : `
        <div class="bar">
          <div class="fill ${levelClass(pct)}" style="width:${pct}%"></div>
        </div>`;
  const pctLabel = pct === null ? "—" : `${pct}%`;
  return `
    <div class="card">
      <span class="icon">${icon}</span>
      <div class="info">
        <div class="name">${esc(d.name)}${d.charging ? ' <span class="charging">⚡</span>' : ""}</div>
        ${bar}
      </div>
      <span class="pct">${pctLabel}</span>
    </div>`;
}

function render(devices: DeviceBattery[]) {
  if (devices.length === 0) {
    app.innerHTML = `<div class="empty">Logitech 기기를 찾지 못했습니다</div>`;
    return;
  }
  app.innerHTML = devices.map(card).join("");
}

// Frameless window: drag from anywhere on the widget.
document.addEventListener("mousedown", (e) => {
  if (e.button === 0) getCurrentWindow().startDragging();
});

listen<DeviceBattery[]>("battery-update", (e) => render(e.payload));

// The poller may have completed its first sweep before this webview loaded —
// pull the latest snapshot instead of waiting a full poll cycle.
invoke<DeviceBattery[]>("get_batteries").then((devices) => {
  if (devices.length > 0) render(devices);
});
