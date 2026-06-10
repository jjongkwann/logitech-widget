# logitech-widget

Logitech 기기(마우스·키보드·헤드셋)의 배터리 잔량을 Windows 바탕화면 위젯으로 보여주는 앱.
G HUB / Options+를 열지 않고도 기기별 배터리·충전 상태를 바로 확인할 수 있다.

- **스택**: Tauri v2 (Rust + Web UI)
- **데이터**: HID++ 직접 통신(주 소스) + G HUB 로컬 WebSocket(폴백)
- **형태**: 테두리 없는 투명 오버레이, 바탕화면 고정(Win+D 생존), 트레이 제어

## 문서

- 기획·마일스톤: [docs/PLAN.md](docs/PLAN.md)
- 개발 규약: [CLAUDE.md](CLAUDE.md)
- 프로토콜 레퍼런스: `.claude/skills/` (hidpp-battery / ghub-websocket / tauri-widget)

## 개발

```
npm run tauri dev     # 개발 실행
npm run tauri build   # 배포 빌드
```

요구사항: Rust(stable-msvc), Node.js LTS, VS C++ Build Tools, WebView2 런타임.

## 테스트 기기

<!-- 실기기 검증에 사용한 보유 기기를 여기에 기록 -->
- (예: G Pro X Superlight — Lightspeed 리시버)
