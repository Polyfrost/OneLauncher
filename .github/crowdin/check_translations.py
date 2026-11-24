"""Check bundled translations without building the graphics dependencies."""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LANG = ROOT / "packages" / "oneclient_app" / "lang"
SRC = ROOT / "packages" / "oneclient_app" / "src"
PLACEHOLDERS = re.compile(r"%%|%s|%(\d+)\$s")


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate key: {key}")
        result[key] = value
    return result


def load(path):
    table = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=unique_object)
    if not isinstance(table, dict):
        raise ValueError(f"{path.name}: expected a JSON object")
    for key, value in table.items():
        if not isinstance(value, str) or not value.strip():
            raise ValueError(f"{path.name}: empty or non-string translation: {key}")
    return table


def arguments(text):
    indices = []
    sequential = 0
    escaped_percent = 0
    for match in PLACEHOLDERS.finditer(text):
        if match[0] == "%%":
            escaped_percent += 1
        elif match[1] is not None:
            indices.append(int(match[1]))
        else:
            sequential += 1
            indices.append(sequential)
    return sorted(indices), escaped_percent


def main():
    source = load(LANG / "en_us.json")
    errors = []
    used = set()
    for path in SRC.rglob("*.rs"):
        text = path.read_text(encoding="utf-8")
        used.update(re.findall(r'\btf?\(\s*"(oneclient\.[^"]+)"', text))
    for key in sorted(used - source.keys()):
        errors.append(f"en_us.json: missing key used in Rust: {key}")

    for path in sorted(LANG.glob("*.json")):
        if path.name == "en_us.json":
            continue
        table = load(path)
        for key in sorted(source.keys() - table.keys()):
            errors.append(f"{path.name}: missing key: {key}")
        for key in sorted(table.keys() - source.keys()):
            errors.append(f"{path.name}: unknown key: {key}")
        for key in source.keys() & table.keys():
            if arguments(source[key]) != arguments(table[key]):
                errors.append(f"{path.name}: placeholders differ: {key}")
        print(f"{path.name}: checked {len(table)} translations")

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("All bundled translations have matching keys and placeholders.")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError) as error:
        print(error, file=sys.stderr)
        sys.exit(1)
