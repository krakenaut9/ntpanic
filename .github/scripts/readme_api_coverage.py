#!/usr/bin/env python3
"""Fail if a public item of the crate is not mentioned in README.md.

The README is the crate's front page on crates.io and docs.rs, but nothing ties
it to the API: an item added to lib.rs with no README mention is invisible to
readers. This closes that gap.

Only mentions in code count, in an inline code span or a code block, so that an
ordinary word in the prose, such as "Callback" in a heading, does not pass for
the item of that name.
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LIB = ROOT / "src" / "lib.rs"
README = ROOT / "README.md"

ITEM = re.compile(
    r"^pub\s+(?:fn|const|static|mod|type|struct|enum|trait)\s+([A-Za-z_][A-Za-z0-9_]*)",
    re.M,
)
CODE_BLOCK = re.compile(r"^\s*```.*?^\s*```", re.M | re.S)
CODE_SPAN = re.compile(r"`[^`\n]+`")


def code_in(markdown: str) -> str:
    blocks = CODE_BLOCK.findall(markdown)
    spans = CODE_SPAN.findall(CODE_BLOCK.sub("", markdown))
    return "\n".join(blocks + spans)


def main() -> int:
    items = sorted(set(ITEM.findall(LIB.read_text(encoding="utf-8"))))
    code = code_in(README.read_text(encoding="utf-8"))
    missing = [n for n in items if not re.search(rf"\b{re.escape(n)}\b", code)]

    print(f"{len(items)} public items, {len(missing)} missing from README.md")
    for name in items:
        mark = "MISSING" if name in missing else "ok"
        print(f"  {mark:>7}  {name}")

    if missing:
        print(f"\nNot mentioned in README.md: {', '.join(missing)}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
