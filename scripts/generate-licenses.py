"""Build the offline license inventory from the locked Windows dependency graph."""
import argparse
import hashlib
import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "docs/licenses/dependencies.json"


def cargo(*args):
    return subprocess.check_output(["cargo", *args], cwd=ROOT, encoding="utf-8")


def generate(target):
    metadata = json.loads(cargo("metadata", "--locked", "--offline", "--format-version", "1", "--filter-platform", target))
    tree = cargo("tree", "--locked", "--offline", "--target", target,
                 "--edges", "normal,build", "--prefix", "none", "--format", "{p}")
    active = set(re.findall(r"^([A-Za-z0-9_-]+) v([^\s]+)", tree, re.MULTILINE))
    overrides = ROOT / "docs/licenses/overrides"
    supplements = json.loads((overrides / "manifest.json").read_text(encoding="utf-8"))
    components = []
    for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
        key = (package["name"], package["version"])
        if key not in active or package["name"] == "file-beholder":
            continue
        root = Path(package["manifest_path"]).parent
        files = sorted(path for path in root.rglob("*") if path.is_file()
                       and ".git" not in path.parts
                       and path.suffix.lower() not in {".rs", ".toml", ".json", ".html", ".svg"}
                       and path.name.lower().startswith(("license", "licence", "copying", "notice", "copyright", "ofl")))
        texts = [(path.relative_to(root).as_posix(), path.read_text(encoding="utf-8-sig")) for path in files]
        for record in supplements:
            if (record["package"], record["version"]) == key:
                path = overrides / record["file"]
                data = path.read_bytes()
                if hashlib.sha256(data).hexdigest() != record["sha256"]:
                    raise ValueError(f"Supplement checksum mismatch: {path.name}")
                texts.append((record["file"], data.decode("utf-8-sig")))
        if not texts:
            raise ValueError(f"No retained license text for {key}")
        components.append({
            "title": f"{key[0]} {key[1]}",
            "license": package["license"] or "See license text",
            "source": package.get("repository") or package.get("homepage") or "",
            "text": "\n\n".join(f"--- {name} ---\n\n{text.strip()}" for name, text in texts),
        })
    return json.dumps({
        "target": target,
        "lock_sha256": hashlib.sha256((ROOT / "Cargo.lock").read_bytes()).hexdigest(),
        "components": components,
    }, ensure_ascii=False, indent=2) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", help="Windows target; defaults to the retained inventory target")
    parser.add_argument("--check", action="store_true", help="Fail if the checked-in inventory is stale")
    args = parser.parse_args()
    target = args.target
    if not target and OUTPUT.exists():
        target = json.loads(OUTPUT.read_text(encoding="utf-8"))["target"]
    target = target or "x86_64-pc-windows-msvc"
    if "windows" not in target:
        parser.error("The release inventory requires a Windows target")
    content = generate(target)
    if args.check:
        if not OUTPUT.exists() or OUTPUT.read_text(encoding="utf-8") != content:
            raise SystemExit("License inventory is stale. Run scripts/generate-licenses.py.")
    else:
        OUTPUT.write_text(content, encoding="utf-8", newline="\n")
    print(f"{'Checked' if args.check else 'Generated'} {len(json.loads(content)['components'])} dependency notices for {target}.")


if __name__ == "__main__":
    main()
