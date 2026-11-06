#!/usr/bin/env python3
"""Fail when a comment in crates/ cites the retired TypeScript (PixiJS) client.

The Rust game must make sense to readers who never saw that client, so comments
state the rule itself and cite the original game when a rule needs a source.
This repository has no TypeScript of its own, so any `.ts` or `.tsx` path in a
comment points at the old client. Mentions of the Website as a live service
(accounts, cloud saves, the mod editor) are fine and are not checked.
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PORT_REFERENCE = re.compile(r"\b[\w./-]+\.tsx?\b|\bpixi|\btypescript\b", re.IGNORECASE)
CHAR_LITERAL = re.compile(r"'(?:\\u\{[0-9a-fA-F]+\}|\\x[0-9a-fA-F]{2}|\\.|[^\\'\n])'")
RAW_STRING = re.compile(r'b?r(#*)"')


def comments(source):
    """Yield (line number, text) for each comment, skipping string and char literals."""
    i, line = 0, 1
    while i < len(source):
        if source.startswith("//", i):
            end = source.find("\n", i)
            end = len(source) if end == -1 else end
        elif source.startswith("/*", i):
            depth, end = 1, i + 2
            while end < len(source) and depth:
                if source.startswith("/*", end):
                    depth, end = depth + 1, end + 2
                elif source.startswith("*/", end):
                    depth, end = depth - 1, end + 2
                else:
                    end += 1
        else:
            raw = RAW_STRING.match(source, i)
            if raw and (i == 0 or not (source[i - 1].isalnum() or source[i - 1] == "_")):
                close = source.find('"' + raw.group(1), raw.end())
                end = len(source) if close == -1 else close + 1 + len(raw.group(1))
            elif source[i] == '"':
                end = i + 1
                while end < len(source) and source[end] != '"':
                    end += 2 if source[end] == "\\" else 1
                end += 1
            elif source[i] == "'" and CHAR_LITERAL.match(source, i):
                end = CHAR_LITERAL.match(source, i).end()
            else:
                line += source[i] == "\n"
                i += 1
                continue
            line += source.count("\n", i, end)
            i = end
            continue
        yield line, source[i:end]
        line += source.count("\n", i, end)
        i = end


def main():
    problems = []
    for path in sorted((ROOT / "crates").rglob("*.rs")):
        for line, text in comments(path.read_text(encoding="utf-8")):
            for match in PORT_REFERENCE.finditer(text):
                number = line + text.count("\n", 0, match.start())
                problems.append(f"{path.relative_to(ROOT)}:{number}: {match.group(0)}")
    for problem in problems:
        print(problem)
    if problems:
        print(f"{len(problems)} comments cite the retired TypeScript client; describe the rule instead.")
        return 1
    print("No comments cite the retired TypeScript client.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
