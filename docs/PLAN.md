# Logitech 배터리 위젯 — 프로젝트 기획

## 1. 목표

Windows 11 바탕화면에 상주하는 위젯으로, 연결된 Logitech 기기(마우스·키보드·헤드셋)별 배터리 잔량과 충전 상태를 실시간 표시한다. G HUB나 Options+를 열지 않고도 바탕화면에서 바로 확인하는 것이 핵심 가치.

**비목표 (하지 않는 것)**
- DPI/키 매핑 등 기기 설정 변경 — 읽기 전용 모니터링만
- Windows 11 위젯 보드(Win+W) 등록 — 오버레이 창 방식으로 충분
- Logitech 외 타사 기기 지원

## 2. 확정된 기술 결정

| 항목 | 결정 | 근거 |
|------|------|------|
| 스택 | Tauri v2 (Rust + Web UI) | 가벼운 상주 앱 + 자유로운 위젯 디자인 |
| 데이터 소스 | HID++ 직접 통신(주) + G HUB WebSocket(폴백) | HID++는 G HUB 없이도 동작·견고. G HUB WS는 쉬운 대신 비공식·버전 취약 |
| 위젯 형태 | 테두리 없는 투명 오버레이, 바탕화면 고정 | `tauri-plugin-wallpaper`의 pin으로 Win+D 생존 (검증된 기법) |
| HID++ 구현 | `hidapi` crate 위에 직접 구현 (~300줄) | 성숙한 Rust HID++ 크레이트 부재. LGSTrayBattery(C#) 구조를 그대로 따름 |

상세 프로토콜 레퍼런스는 `.claude/skills/` 참고 (hidpp-battery, ghub-websocket, tauri-widget).

## 3. 아키텍처

```
┌─ Frontend (WebView) ─────────────┐
│ 위젯 UI: 기기 카드(이름·%·충전)   │
│ listen("battery-update")          │
└──────────▲───────────────────────┘
           │ emit (Tauri event)
┌─ Rust (src-tauri) ───────────────┐
│ poller: 주기 폴링 + 이벤트 수신    │
│   ├─ BatterySource trait          │
│   │   ├─ HidppSource (주)        │ ← hidapi, VID 0x046D 벤더 컬렉션
│   │   └─ GHubSource (폴백)       │ ← ws://localhost:9010 (subprotocol: json)
│   ├─ tray: 트레이 아이콘/메뉴     │
│   └─ window: 오버레이 + 데스크톱 핀│
└──────────────────────────────────┘
```

공유 모델: `DeviceBattery { id, name, device_type, percentage, charging, source }`
소스 정책: HID++로 잡히는 기기는 HID++, 못 잡는 기기만 G HUB로 보충. 같은 기기가 양쪽에 잡히면 HID++ 우선.

## 4. 마일스톤 (각 단계 = 검증 통과 시 완료)

### Phase 0 — 스캐폴드
- `npm create tauri-app@latest`로 프로젝트 생성, 디렉토리 구조를 CLAUDE.md대로 정리
- ✅ 검증: `npm run tauri dev`로 기본 창이 뜬다

### Phase 1 — HID++ 데이터 레이어 (UI 없이 CLI 검증)
- hidapi 열거(VID 0x046D + 벤더 usage page), short/long 채널 페어링
- HID++ 2.0: ping → 기능 탐색(0x1004→0x1001→0x1000) → 배터리 읽기 → 기기명(0x0005)
- HID++ 1.0 폴백: 리시버 페어링 열거(reg 0x02 fake arrival), 배터리 레지스터 0x0D/0x07
- 수면 기기 처리: 타임아웃 + 0x8F 오류 → offline 표시
- ✅ 검증: `cargo run --example dump_batteries`가 실제 연결 기기들의 이름/%/충전상태를 출력; 바이트 파싱은 픽스처 기반 `cargo test` 통과

### Phase 2 — 위젯 UI + 연결
- poller가 30초 주기 + 이벤트 브로드캐스트 수신으로 갱신, `battery-update` emit
- 프론트엔드: 기기별 카드(아이콘·이름·잔량 바·충전 표시), 투명 배경
- ✅ 검증: 위젯에 실기기 배터리가 표시되고, 마우스를 충전기에 꽂으면 다음 갱신에서 충전 표시로 바뀐다

### Phase 3 — 위젯 셸 완성
- 테두리 없는 투명 창, 드래그 이동(`data-tauri-drag-region`), 위치 저장/복원
- `tauri-plugin-wallpaper` pin으로 바탕화면 고정
- 트레이: 표시/숨김 토글, 기기 목록, 종료
- ✅ 검증: Win+D를 눌러도 위젯이 남아 있다; 작업표시줄/Alt-Tab에 안 나온다; 트레이에서 종료 가능

### Phase 4 — G HUB 폴백 소스
- `ws://localhost:9010` 연결(+재연결 백오프), `/devices/list` GET + `/battery/state/changed` SUBSCRIBE
- HID++ 결과와 병합(HID++ 우선, 중복 제거)
- ✅ 검증: HID++ 소스를 강제로 끈 상태에서 G HUB만으로 동일 기기 배터리가 표시된다; G HUB 미실행 시 조용히 비활성

### Phase 5 — 마무리
- 자동 시작(tauri-plugin-autostart), 저배터리 알림(예: 15% 미만 토스트), 설정(폴링 주기·투명도) 저장
- `npm run tauri build`로 배포 빌드
- ✅ 검증: 재부팅 후 자동 실행, 설치본 단독 실행 확인

## 5. 리스크와 대응

| 리스크 | 영향 | 대응 |
|--------|------|------|
| 기기별 HID++ 편차 (1.0/2.0, 기능 미지원) | 일부 기기 미표시 | 0x1004→0x1001→0x1000→1.0 레지스터 순 폴백 체인. Solaar/LGSTrayBattery 동작 모사 |
| G HUB 업데이트로 WS 프로토콜 변경 | 폴백 소스 작동 중단 | 비공식 API임을 전제, `ghub.rs`에 격리. HID++가 주 소스라 치명적이지 않음 |
| Win+D 핀 기법 OS 업데이트 취약성 | 위젯이 숨겨짐 | 플러그인 실패 시 수동 `WM_WINDOWPOSCHANGING` 서브클래싱 폴백 (skill 문서에 기록) |
| 투명창 렌더링 이슈 (tauri#8308 등) | 시각 결함 | 알려진 이슈 목록을 skill에 기록, 발생 시 우선 대조 |
| Options+ 전용 기기(MX 시리즈) | G HUB에 안 잡힘 | HID++ 직접 통신이 커버 (Options+ 로컬 API는 미검증이라 의존하지 않음) |

## 6. 테스트 전략

- **순수 파싱 = 단위 테스트**: HID++ 응답 바이트 → 구조체 변환은 I/O 없는 함수로 작성, 실기기에서 캡처한 바이트를 픽스처로 사용
- **실기기 검증**: 각 Phase의 ✅ 항목은 실제 Logitech 기기로 확인 (보유 기기 목록을 README에 기록해 둘 것)
- **G HUB WS**: 캡처한 JSON(LGSTrayBattery_GHUB_dump 참고)으로 역직렬화 테스트
