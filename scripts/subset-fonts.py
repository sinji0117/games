#!/usr/bin/env python3
"""화면 글꼴을 부분 글꼴(woff)로 다시 만듭니다.

    python3 scripts/subset-fonts.py <google/fonts 의 ofl 폴더>

KS X 1001 한글 2,350자 + 영문 · 기호 + apps/site 소스에 쓰인 모든 글자를 담습니다.
결과는 packages/ui/src/fonts/ 에 덮어씁니다. fontTools(pyftsubset) 가 필요합니다.
"""
import glob
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "packages/ui/src/fonts"
FONTS = {
    "Jua-Regular": "jua/Jua-Regular.ttf",
    "IBMPlexSansKR-Regular": "ibmplexsanskr/IBMPlexSansKR-Regular.ttf",
    "IBMPlexSansKR-Medium": "ibmplexsanskr/IBMPlexSansKR-Medium.ttf",
    "IBMPlexSansKR-Bold": "ibmplexsanskr/IBMPlexSansKR-Bold.ttf",
}


def charset() -> str:
    chars = set()
    # KS X 1001 완성형 한글 (EUC-KR B0A1..C8FE)
    for lead in range(0xB0, 0xC9):
        for trail in range(0xA1, 0xFF):
            try:
                chars.add(bytes([lead, trail]).decode("euc_kr"))
            except UnicodeDecodeError:
                pass
    ranges = [(0x20, 0x7F), (0xA0, 0x100), (0x2010, 0x2070), (0x2190, 0x21A0), (0x3000, 0x3020), (0x3131, 0x318F)]
    for a, b in ranges:
        chars.update(chr(c) for c in range(a, b))
    sources = glob.glob(str(ROOT / "apps/site/src/**/*.ts*"), recursive=True) + [str(ROOT / "apps/site/index.html")]
    for f in sources:
        chars.update(c for c in Path(f).read_text(encoding="utf-8") if ord(c) > 0x7F)
    return "".join(sorted(chars))


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    ofl = Path(sys.argv[1])
    text = OUT / "subset-chars.txt"
    text.write_text(charset(), encoding="utf-8")
    for name, src in FONTS.items():
        subprocess.run(
            ["pyftsubset", str(ofl / src), f"--text-file={text}", "--flavor=woff",
             "--layout-features=*", f"--output-file={OUT / (name + '.woff')}"],
            check=True,
        )
        print(f"✔ {name}.woff {(OUT / (name + '.woff')).stat().st_size // 1024}KB")


if __name__ == "__main__":
    main()
