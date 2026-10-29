#!/usr/bin/env python3
"""
SPARKRUSH-CORE optimizer agent — DGK x PROTON protocol.

This is an honest regex-based optimizer agent. It does NOT pretend to be
an LLM. It applies deterministic, testable transformations:

  1. strip trailing whitespace from every line
  2. convert tabs to 4 spaces
  3. collapse runs of more than N consecutive blank lines into exactly N
  4. report what was changed

Output:
  - writes the cleaned file to <input>.opt
  - prints a transformation report on stdout

This agent is invoked by:
  - sparkrush-editor <file> --optimize
  - sparkrush-core.sh optimize <file>

DGK wanted "agents IA qui optimisent le code en temps réel". We deliver
an honest regex agent that does what it claims and nothing it doesn't.
"""

from __future__ import annotations
import os
import re
import sys
from pathlib import Path


def transform(content: str, max_blank: int = 2) -> tuple[str, dict]:
    """Apply deterministic transformations; return (new_content, stats)."""
    stats: dict[str, int] = {
        "lines_in": 0,
        "lines_out": 0,
        "trailing_ws_stripped": 0,
        "tabs_converted": 0,
        "blank_runs_collapsed": 0,
    }

    lines = content.split("\n")
    stats["lines_in"] = len(lines)

    # 1. strip trailing whitespace + 2. tabs -> 4 spaces
    cleaned: list[str] = []
    for line in lines:
        # count tabs being converted (before replacing)
        n_tabs = line.count("\t")
        if n_tabs:
            line = line.replace("\t", "    ")
            stats["tabs_converted"] += n_tabs
        # detect trailing ws before stripping
        if line != line.rstrip():
            stats["trailing_ws_stripped"] += 1
            line = line.rstrip()
        cleaned.append(line)

    # 3. collapse runs of more than `max_blank` consecutive blank lines
    out: list[str] = []
    blank_run = 0
    for line in cleaned:
        if line.strip() == "":
            blank_run += 1
            if blank_run <= max_blank:
                out.append(line)
            else:
                # exceeded: skip
                continue
        else:
            if blank_run > max_blank:
                stats["blank_runs_collapsed"] += 1
            blank_run = 0
            out.append(line)
    if blank_run > max_blank:
        stats["blank_runs_collapsed"] += 1

    stats["lines_out"] = len(out)
    new_content = "\n".join(out)
    # ensure single trailing newline
    new_content = new_content.rstrip("\n") + "\n"
    return new_content, stats


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print("usage: optimizer.py <file>", file=sys.stderr)
        return 2
    src = Path(argv[1])
    if not src.is_file():
        print(f"optimizer: file not found: {src}", file=sys.stderr)
        return 2

    original = src.read_text(encoding="utf-8", errors="replace")
    new_content, stats = transform(original, max_blank=2)

    dest = src.with_suffix(src.suffix + ".opt")
    dest.write_text(new_content, encoding="utf-8")
    # make sure the .opt is readable
    os.chmod(dest, 0o644)

    print(f"optimizer: input  {src}  ({stats['lines_in']} lines)")
    print(f"optimizer: output {dest} ({stats['lines_out']} lines)")
    print(f"optimizer: tabs converted (count={stats['tabs_converted']})")
    print(f"optimizer: trailing-ws stripped on {stats['trailing_ws_stripped']} lines")
    print(f"optimizer: blank-run collapses: {stats['blank_runs_collapsed']}")
    if stats["tabs_converted"] == 0 and stats["trailing_ws_stripped"] == 0 and stats["blank_runs_collapsed"] == 0:
        print("optimizer: no transformations needed — file already clean")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
