# 보안 정책

## 취약점 제보

보안 취약점은 **공개 이슈로 올리지 말고** GitHub 의
[Security → Report a vulnerability](https://github.com/sinji0117/games/security/advisories/new) (비공개 제보)로 알려 주세요.
어려우면 [beripick.com](https://beripick.com) 으로 연락해 주세요.

제보에 넣어 주시면 좋은 것:
- 영향받는 버전 (`/api/health` 의 `version`) 과 기능 (예: 빙고 방, 로그인)
- 재현 순서 또는 요청 예시
- 예상되는 피해 (계정 탈취, 결과 조작, 서비스 중단 등)

받은 뒤 3일 안에 답하고, 고친 버전을 내면 [CHANGELOG](CHANGELOG.md) 에 기록합니다. 원하시면 제보자 이름을 함께 남깁니다.

## 지원 버전

가장 최근 버전(`main`)만 보안 수정을 받습니다.

## 설계상 보안 장치

| 영역 | 내용 |
| --- | --- |
| 비밀번호 | argon2id 해시만 저장. 8~128자 |
| 로그인 토큰 | 256비트 난수. 서버는 sha256 해시만 저장, 30일 만료, 로그아웃 시 삭제. 쿠키를 쓰지 않아 CSRF 대상이 아님 |
| 방장 · 참가자 토큰 | 해시만 저장, 상수 시간 비교 |
| 게임 결과 | 서버가 시작 때 확정하고 sha256 지문을 먼저 공개(commit-reveal). 클라이언트가 보낸 결과는 믿지 않음 |
| 요청 제한 | IP 별: 게임 만들기 · 참가 · 가입(10분 5회) · 로그인(분 10회) · WebSocket 연결. 아이디별: 로그인 15분 20회 |
| 포인트 | 같은 대상 중복 지급 불가(원장 고유 키) + 사유별 하루 한도 |
| 응답 헤더 | CSP(같은 출처 스크립트만, iframe 금지), `X-Frame-Options: DENY`, `nosniff`, `Referrer-Policy` |
| 입력 | 모든 이름 · 단어 길이 제한, 아이디는 완성형 한글 · 영문 · 숫자 · `_` 만. SQL 은 모두 바인딩 |
| 배포 | 127.0.0.1 에만 열리고 Nginx 뒤에서 동작, Docker 이미지는 비루트 사용자. 비밀값은 저장소에 두지 않음 |
| 의존성 | CI 에서 `cargo audit` · `pnpm audit`, Dependabot 주간 업데이트 |

운영자가 확인할 것은 [docs/deploy-rpanel.md](docs/deploy-rpanel.md) 의 운영 참고를 보세요.
