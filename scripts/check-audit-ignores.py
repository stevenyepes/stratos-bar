#!/usr/bin/env python3
"""Enforce security/audit-ignore.toml against cargo audit / npm audit.

Usage:
  check-audit-ignores.py --tool cargo
      Prints "--ignore ID --ignore ID ..." for the still-valid cargo entries
      (to be passed through to `cargo audit`), or exits 1 if any cargo entry
      has expired.

  check-audit-ignores.py --tool npm --npm-audit-json <path>
      Exits 0 if every advisory in the `npm audit --json` output at <path>
      is covered by a still-valid npm entry, else exits 1. Also exits 1 if
      any npm entry has expired.

  check-audit-ignores.py --self-test
      Verifies the expiry check itself fails closed on an expired entry.
"""
import argparse
import datetime
import json
import sys
import tomllib
from pathlib import Path

DEFAULT_IGNORE_FILE = Path(__file__).resolve().parent.parent / "security" / "audit-ignore.toml"


def load_entries(path: Path) -> list[dict]:
    with open(path, "rb") as f:
        data = tomllib.load(f)
    return data.get("ignore", [])


def split_by_expiry(entries: list[dict], tool: str, today: datetime.date):
    active, expired = [], []
    for entry in entries:
        if entry.get("tool") != tool:
            continue
        expires = datetime.date.fromisoformat(entry["expires"])
        (expired if expires < today else active).append(entry)
    return active, expired


def resolve_advisory_ids(name: str, vulns: dict, seen: set[str]) -> set[str]:
    if name in seen:
        return set()
    seen.add(name)
    ids: set[str] = set()
    for via in vulns.get(name, {}).get("via", []):
        if isinstance(via, dict):
            url = via.get("url", "")
            if url:
                ids.add(url.rstrip("/").split("/")[-1])
        elif isinstance(via, str):
            ids |= resolve_advisory_ids(via, vulns, seen)
    return ids


def check_cargo(ignore_file: Path, today: datetime.date) -> int:
    active, expired = split_by_expiry(load_entries(ignore_file), "cargo", today)
    if expired:
        print("The following cargo audit ignore entries have expired and must be re-triaged:", file=sys.stderr)
        for entry in expired:
            print(f"  - {entry['id']} (expired {entry['expires']})", file=sys.stderr)
        return 1
    print(" ".join(f"--ignore {entry['id']}" for entry in active))
    return 0


def check_npm(ignore_file: Path, npm_audit_json: Path, today: datetime.date) -> int:
    active, expired = split_by_expiry(load_entries(ignore_file), "npm", today)
    if expired:
        print("The following npm audit ignore entries have expired and must be re-triaged:", file=sys.stderr)
        for entry in expired:
            print(f"  - {entry['id']} (expired {entry['expires']})", file=sys.stderr)
        return 1

    active_ids = {entry["id"] for entry in active}
    with open(npm_audit_json) as f:
        audit = json.load(f)
    vulns = audit.get("vulnerabilities", {})

    unignored = []
    for pkg in vulns:
        advisory_ids = resolve_advisory_ids(pkg, vulns, set())
        if not advisory_ids.issubset(active_ids):
            unignored.append((pkg, advisory_ids - active_ids))

    if unignored:
        print("The following npm packages have vulnerabilities not covered by an ignore entry:", file=sys.stderr)
        for pkg, missing in unignored:
            print(f"  - {pkg}: {', '.join(sorted(missing)) or '(no advisory id found)'}", file=sys.stderr)
        return 1

    print("All npm audit findings are covered by non-expired ignore entries.")
    return 0


def self_test() -> int:
    import tempfile

    fixture = """
[[ignore]]
id = "RUSTSEC-0000-0000"
tool = "cargo"
reason = "fixture: deliberately expired entry"
expires = "2000-01-01"
"""
    with tempfile.NamedTemporaryFile("w", suffix=".toml", delete=False) as f:
        f.write(fixture)
        fixture_path = Path(f.name)
    try:
        today = datetime.date.today()
        rc = check_cargo(fixture_path, today)
        assert rc == 1, "expected an expired entry to fail the check"
        print("self-test passed: expired ignore entries correctly fail the build")
        return 0
    finally:
        fixture_path.unlink()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tool", choices=["cargo", "npm"])
    parser.add_argument("--file", type=Path, default=DEFAULT_IGNORE_FILE)
    parser.add_argument("--npm-audit-json", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    if not args.tool:
        parser.error("--tool is required unless --self-test is given")

    today = datetime.date.today()
    if args.tool == "cargo":
        return check_cargo(args.file, today)

    if not args.npm_audit_json:
        parser.error("--npm-audit-json is required for --tool npm")
    return check_npm(args.file, args.npm_audit_json, today)


if __name__ == "__main__":
    sys.exit(main())
