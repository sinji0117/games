# 글꼴

| 파일 | 원본 | 라이선스 |
| --- | --- | --- |
| `Jua-Regular.woff` | [Jua](https://github.com/google/fonts/tree/main/ofl/jua) | SIL OFL 1.1 ([OFL-Jua.txt](OFL-Jua.txt)) |
| `IBMPlexSansKR-{Regular,Medium,Bold}.woff` | [IBM Plex Sans KR](https://github.com/google/fonts/tree/main/ofl/ibmplexsanskr) | SIL OFL 1.1 ([OFL-IBMPlexSansKR.txt](OFL-IBMPlexSansKR.txt)) |

외부 글꼴 서버(Google Fonts)를 쓰지 않으려고 저장소에 넣었습니다. 용량을 줄이려고 **부분 글꼴**로 만들었습니다:
KS X 1001 한글 2,350자 + 영문 · 라틴-1 · 문장 부호 · 한글 낱자 + 화면 소스에 쓰인 글자 ([subset-chars.txt](subset-chars.txt)).
여기에 없는 글자(드문 한글 등)는 시스템 글꼴로 보입니다.

화면 문구에 새 글자를 넣었다면 다시 만듭니다 (fontTools 필요: `pip install fonttools`).

```bash
# google/fonts 저장소에서 ofl/jua, ofl/ibmplexsanskr 의 ttf 를 받아 둔 폴더를 G 로
python3 scripts/subset-fonts.py "$G"
```
