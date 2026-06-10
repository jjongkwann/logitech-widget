# logitech-widget

Logitech 기기(마우스·키보드·헤드셋)의 배터리 잔량을 Windows 바탕화면 위젯으로 보여주는 앱.
G HUB / Options+를 열지 않고도 기기별 배터리·충전 상태를 바로 확인할 수 있다.

- **스택**: Tauri v2 (Rust + Web UI)
- **데이터**: HID++ 직접 통신(주 소스) + G HUB 로컬 WebSocket(폴백)
- **형태**: 테두리 없는 투명 오버레이, 바탕화면 고정(Win+D 생존), 트레이 제어

## 문서

- 기획·마일스톤: [docs/PLAN.md](docs/PLAN.md)

## 기능

- 기기별 배터리 카드 (잔량 바·충전 ⚡·오프라인 표시), 30초 주기 갱신
- 테두리 없는 투명 위젯 — 드래그로 이동, 위치 기억
- Win+D(바탕화면 보기) 시 위젯을 데스크톱 위로 끌어올림 — ⚠️ Win11 셸이 데스크톱 레이어를 일반 창 위로 올리는 동작 때문에 환경에 따라 동작하지 않을 수 있는 알려진 제약 (창이 다시 열리면 항상 정상 표시)
- 트레이 메뉴: 위젯 표시/숨기기, 로그인 시 자동 실행 토글, 종료
- 저배터리(기본 15% 이하) Windows 알림 — 방전 사이클당 1회
- 데이터 소스: HID++ 직접 통신 우선, G HUB 실행 중이면 폴백으로 보충 (동일 기기는 자동 병합)

## 설정

`%APPDATA%\com.jjongkwann.logitech-widget\settings.json` (없으면 기본값):

```json
{ "pollIntervalSecs": 30, "lowBatteryThreshold": 15 }
```

위젯 위치는 같은 폴더의 `position.json`에 자동 저장된다.

## 개발

```
npm run tauri dev     # 개발 실행
npm run tauri build   # 배포 빌드 (NSIS/MSI 설치본)
cargo run --example dump_batteries --manifest-path src-tauri/Cargo.toml  # HID++ 단독 점검
cargo run --example dump_ghub --manifest-path src-tauri/Cargo.toml       # G HUB 단독 점검
```

요구사항: Rust(stable-msvc), Node.js LTS, VS C++ Build Tools, WebView2 런타임.

## 테스트 기기

<!-- 실기기 검증에 사용한 보유 기기 -->
- Logitech G PRO X 2 (마우스) — Lightspeed 리시버(PID 0xC54D), HID++ 2.0 / UNIFIED_BATTERY(0x1004)
