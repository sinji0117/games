# 사다리 게임

## 규칙
- 참가자 N명(2~16), 결과 항목 N개(당첨/벌칙 등, 참가자 수와 동일). 이름은 1~20자, 참가자 이름 중복 불가.
- 세로줄 N개 사이에 가로선을 놓을 칸이 `rows`개(6~40, 기본 `clamp(N×2, 10, 40)`) 있습니다.
- 출발점에서 아래로 내려가다 가로선을 만나면 반드시 건너갑니다. 도착한 세로줄의 항목이 결과입니다.
- **같은 행에서 인접한 가로선은 금지**합니다. 그래야 모든 경로가 유일하고 결과가 항상 1:1 순열이 됩니다.
- 참가자는 결과 공개 전까지 가로선을 추가할 수 있습니다 (방당 최대 N×3개).

## 데이터 모델 (`crates/core/src/games/ladder.rs`)

| 타입 | 설명 |
| --- | --- |
| `LadderConfig` | `participants`, `outcomes`, `rows?`, `density?`(0.15~0.6, 기본 0.35) |
| `Rung { row, col, source }` | `col`번과 `col+1`번 세로줄을 `row`행에서 잇는 가로선. `source = generated \| player` |
| `Point { x, y }` | 경로 좌표. `x` = 세로줄, `y` = 단계 |
| `LadderResult` | `start`, `end`, `participant`, `outcome`, `path` |

내부 격자: `grid[row * (N-1) + col] : Option<RungSource>`

## 생성 알고리즘
```text
rng = SeededRng(seed)
for row in 0..rows:
  for col in 0..N-1:
    if 왼쪽(col-1)에 가로선 없음 and rng.chance(density): 가로선 배치
for col in 0..N-1:            # 섞이지 않는 쌍 방지
  if 이 열에 가로선이 하나도 없음:
    이웃이 비어 있는 행 중 하나를 rng 로 골라 배치
```
같은 seed·config 면 항상 같은 사다리가 나옵니다 (테스트 `deterministic_by_seed`).

## 경로 추적
```text
x = start; path = [(x, 0)]
for row in 0..rows:
  y = row + 1
  if grid[row][x-1]: path += (x,y), (x-1,y); x -= 1
  elif grid[row][x]: path += (x,y), (x+1,y); x += 1
path += (x, rows + 1)
```
클라이언트는 `path` 의 점을 순서대로 직선으로 이으면 됩니다. 세로 구간은 `y` 증가, 가로 구간은 `x` ±1.
화면 좌표 변환 예: `px = padX + x * colGap`, `py = padY + y * (height / (rows + 1))`.

## 진행 흐름과 공정성
1. **생성** `POST /api/ladder/games` → `commitment`, `host_token` 발급. 기본 가로선은 **숨김**.
2. **가로선 추가** `POST /games/{id}/rungs` — 누구나. 숨은 가로선과 겹치면 사유를 감춘 422 를 반환합니다
   (단, "놓을 수 없음" 자체가 1비트 정보를 주는 것은 파티 게임 특성상 허용).
3. **결과 공개** `POST /games/{id}/reveal` (`x-host-token`) → 상태 `finished`, 전체 가로선 · `seed` · `nonce` · 결과 공개.
4. **검증(클라이언트)**: `sha256(seed:nonce) == commitment` 확인 후, `LadderGame::create(config, seed)` 로 재생성한
   `generated` 가로선이 응답과 같은지 비교. 결과 공개 이후 상태는 바뀌지 않습니다.

## 프론트엔드 (`apps/site`)

| 파일 | 역할 |
| --- | --- |
| `src/lib/ladder/rng.ts`, `engine.ts` | Rust 엔진의 TS 포팅 (BigInt 로 u64 연산). 생성·추적 결과가 Rust 와 비트 단위로 같음 |
| `src/lib/ladder/verify.ts` | 공개 후 해시·재생성 사다리·경로를 브라우저에서 다시 계산해 검증 |
| `src/lib/ladder/geometry.ts` | 좌표 → SVG 픽셀, 경로 길이·보간, 포인터 → 칸 판정 |
| `src/pages/ladder/LadderBoard.tsx` | 사다리 SVG. 이름/결과 라벨은 `repeat(N, 1fr)` 그리드라 세로줄 중심과 정확히 정렬 |
| `src/pages/ladder/LadderRoom.tsx` | 방 화면: 2.5초 폴링(공개 전, 탭이 보일 때만), 가로선 긋기, 공개, 애니메이션, 검증 표시 |
| `src/lib/ladder/tests/` | `rust-fixtures.json`(Rust 에서 뽑은 RNG 수열·사다리 24개)과 비교하는 교차 검증 |

- 방장 토큰은 `localStorage["ladder.host.{id}"]` 에 저장합니다. 다른 브라우저에서 열면 공개 버튼이 보이지 않습니다.
- 공개 전 화면은 기본 가로선 영역을 빗금 가림막으로 덮고, 참가자가 그은 가로선만 그 위에 그립니다.
- 터치 기기는 hover 가 없으므로 클릭 좌표로 칸을 다시 계산합니다.
- `prefers-reduced-motion` 이면 경로 애니메이션을 생략하고 바로 결과를 보여줍니다.
- Rust 엔진을 바꾸면 기준값을 다시 생성합니다: `cargo run -p game-core --example ladder_fixtures > apps/site/src/lib/ladder/tests/rust-fixtures.json`

## 테스트로 보장하는 불변식
- 모든 크기(2~16명) × 200개 시드 × 밀도/행 수 경계값에서: 인접 가로선 없음, 결과가 순열, 모든 인접 쌍이 1회 이상 연결
- 경로: 시작·끝 좌표 일치, 항상 아래로 진행, 가로 이동은 1칸이며 실제 가로선 위에서만
- 결과 확정 후 가로선 추가 불가, `finish()` 재호출 시 동일 결과
