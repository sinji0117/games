#!/usr/bin/env python3
"""모노레포 전체 버전을 한 번에 갱신합니다.

사용법:
    python3 scripts/bump-version.py 0.1.1
    python3 scripts/bump-version.py 0.1.1 --dry-run

갱신 대상:
  - VERSION
  - Cargo.toml          [workspace.package] version
  - Cargo.lock          워크스페이스 크레이트(game-*) 항목
  - package.json        루트, apps/*, packages/*
  - README.md           버전 배지 (version-X.Y.Z-)
  - CHANGELOG.md        새 버전 섹션이 없으면 템플릿 추가
"""
from __future__ import annotations

import argparse
import datetime as dt
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SEMVER = re.compile(r"^\d+\.\d+\.\d+$")
WORKSPACE_CRATE_PREFIX = "game-"


def read_current() -> str:
    return (ROOT / "VERSION").read_text(encoding="utf-8").strip()


def write(path: Path, text: str, dry: bool, changed: list[str]) -> None:
    old = path.read_text(encoding="utf-8")
    if old != text:
        changed.append(str(path.relative_to(ROOT)))
        if not dry:
            path.write_text(text, encoding="utf-8", newline="\n")


def bump_cargo_toml(new: str, dry: bool, changed: list[str]) -> None:
    path = ROOT / "Cargo.toml"
    text = path.read_text(encoding="utf-8")
    pattern = re.compile(r'(\[workspace\.package\][^\[]*?\nversion\s*=\s*")[^"]+(")', re.S)
    if not pattern.search(text):
        sys.exit("Cargo.toml 에서 [workspace.package] version 을 찾지 못했습니다.")
    write(path, pattern.sub(rf"\g<1>{new}\g<2>", text, count=1), dry, changed)


def bump_cargo_lock(new: str, dry: bool, changed: list[str]) -> None:
    path = ROOT / "Cargo.lock"
    if not path.exists():
        return
    text = path.read_text(encoding="utf-8")
    pattern = re.compile(
        rf'(\[\[package\]\]\nname = "{WORKSPACE_CRATE_PREFIX}[^"]+"\nversion = ")[^"]+(")'
    )
    write(path, pattern.sub(rf"\g<1>{new}\g<2>", text), dry, changed)


def package_jsons() -> list[Path]:
    paths = [ROOT / "package.json"]
    for group in ("apps", "packages"):
        base = ROOT / group
        if base.is_dir():
            paths += sorted(p / "package.json" for p in base.iterdir() if (p / "package.json").exists())
    return [p for p in paths if p.exists()]


def bump_package_json(path: Path, new: str, dry: bool, changed: list[str]) -> None:
    text = path.read_text(encoding="utf-8")
    data = json.loads(text)
    if "version" not in data:
        return
    # 키 순서와 들여쓰기를 보존하기 위해 정규식으로 첫 version 만 교체
    updated = re.sub(r'("version"\s*:\s*")[^"]+(")', rf"\g<1>{new}\g<2>", text, count=1)
    write(path, updated, dry, changed)


def bump_readme(new: str, dry: bool, changed: list[str]) -> None:
    path = ROOT / "README.md"
    if not path.exists():
        return
    text = path.read_text(encoding="utf-8")
    write(path, re.sub(r"version-\d+\.\d+\.\d+-", f"version-{new}-", text), dry, changed)


def bump_changelog(new: str, dry: bool, changed: list[str]) -> None:
    path = ROOT / "CHANGELOG.md"
    text = path.read_text(encoding="utf-8")
    if f"## [{new}]" in text:
        return
    today = dt.date.today().isoformat()
    section = f"## [{new}] - {today}\n\n### Changed\n- \n\n"
    idx = text.find("## [")
    updated = text + "\n" + section if idx == -1 else text[:idx] + section + text[idx:]
    write(path, updated, dry, changed)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("version", help="새 버전 (예: 0.1.1)")
    ap.add_argument("--dry-run", action="store_true", help="파일을 바꾸지 않고 대상만 출력")
    args = ap.parse_args()

    new = args.version.strip().lstrip("v")
    if not SEMVER.match(new):
        sys.exit(f"버전 형식이 올바르지 않습니다: {new} (예: 0.1.1)")
    current = read_current()
    if tuple(map(int, new.split("."))) <= tuple(map(int, current.split("."))):
        sys.exit(f"새 버전({new})은 현재 버전({current})보다 커야 합니다.")

    changed: list[str] = []
    write(ROOT / "VERSION", new + "\n", args.dry_run, changed)
    bump_cargo_toml(new, args.dry_run, changed)
    bump_cargo_lock(new, args.dry_run, changed)
    for p in package_jsons():
        bump_package_json(p, new, args.dry_run, changed)
    bump_readme(new, args.dry_run, changed)
    bump_changelog(new, args.dry_run, changed)

    mode = "(dry-run) " if args.dry_run else ""
    print(f"{mode}{current} -> {new}")
    for c in changed:
        print(f"  - {c}")
    if not args.dry_run:
        print("CHANGELOG.md 의 새 섹션에 변경 내용을 기록하세요.")


if __name__ == "__main__":
    main()
