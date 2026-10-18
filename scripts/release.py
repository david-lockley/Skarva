#!/usr/bin/env python3
"""Version and release helpers. Requires Python 3.11+; no third-party packages."""
import argparse
import hashlib
import os
from pathlib import Path
import re
import tomllib

ROOT = Path(__file__).resolve().parent.parent
SEMVER = re.compile(r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?")


def validate_version(version):
    match = SEMVER.fullmatch(version)
    if not match or (match[4] and any(p.isdigit() and len(p) > 1 and p[0] == "0" for p in match[4].split("."))):
        raise ValueError(f"Invalid semantic version: {version}")
    return version


def read_version(root=ROOT):
    return validate_version(tomllib.loads((root / "Cargo.toml").read_text())["package"]["version"])


def check_version(root=ROOT, tag=""):
    version = read_version(root)
    packages = tomllib.loads((root / "Cargo.lock").read_text())["package"]
    app = [p for p in packages if p["name"] == "skarva" and "source" not in p]
    if len(app) != 1 or app[0]["version"] != version:
        raise ValueError("Cargo.lock and Cargo.toml disagree. Run scripts/release.py set VERSION.")
    if tag and tag != f"v{version}":
        raise ValueError(f"Tag {tag!r} does not match Cargo.toml; expected v{version}")
    return version


def set_version(version, root=ROOT):
    validate_version(version)
    manifest = root / "Cargo.toml"
    lock = root / "Cargo.lock"
    manifest_text, count = re.subn(r'(?m)^version\s*=\s*"[^"]+"', f'version = "{version}"', manifest.read_text(), count=1)
    lock_text, lock_count = re.subn(r'(\[\[package\]\]\nname = "skarva"\nversion = ")[^"]+("\n)', lambda m: m[1] + version + m[2], lock.read_text(), count=1)
    if count != 1 or lock_count != 1:
        raise ValueError("Could not find the application version in both Cargo files")
    manifest.write_text(manifest_text)
    lock.write_text(lock_text)


def checksums(directory, version=None):
    files = sorted(p for p in directory.iterdir() if p.is_file() and p.name != "SHA256SUMS")
    if not files:
        raise ValueError("No release assets found")
    if version:
        validate_version(version)
        prefix = f"skarva-{version}-"
        expected = {prefix + suffix for suffix in (
            "macos-arm64.zip", "macos-arm64.dmg", "macos-x64.zip", "macos-x64.dmg",
            "windows-x64.zip", "windows-x64-setup.exe", "linux-x64.tar.gz", "linux-x64.deb")}
        actual = {p.name for p in files}
        if actual != expected:
            raise ValueError(f"Incomplete or unexpected release assets. Missing: {sorted(expected - actual)}; unexpected: {sorted(actual - expected)}")
    lines = []
    for path in files:
        with path.open("rb") as source:
            digest = hashlib.file_digest(source, "sha256").hexdigest()
        lines.append(f"{digest}  {path.name}\n")
    (directory / "SHA256SUMS").write_text("".join(lines))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("version")
    check = commands.add_parser("check")
    check.add_argument("--tag", default=os.environ.get("RELEASE_TAG", ""))
    bump = commands.add_parser("set")
    bump.add_argument("version")
    sums = commands.add_parser("checksums")
    sums.add_argument("directory", type=Path)
    sums.add_argument("--version")
    args = parser.parse_args()
    if args.command == "set":
        set_version(args.version)
        print(f"Set Skarva version to {args.version}. Commit both Cargo files before tagging.")
    elif args.command == "checksums":
        checksums(args.directory, args.version)
    else:
        version = check_version(tag=args.tag) if args.command == "check" else read_version()
        print(version)
        if args.command == "check" and os.environ.get("GITHUB_OUTPUT"):
            with open(os.environ["GITHUB_OUTPUT"], "a") as output:
                output.write(f"version={version}\nprerelease={'true' if '-' in version.split('+')[0] else 'false'}\n")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, KeyError) as error:
        raise SystemExit(str(error)) from error
