#!/usr/bin/env python3
"""Turn cargo clippy's JSON diagnostics into annotations and a job summary.

Warnings are informational: the step exits with clippy's own status, so a
warning never fails CI. Compiler errors do fail CI through the pipeline.
"""

from __future__ import annotations

import json
import os
import sys

MAX_ANNOTATIONS = 50


def escape(message: str) -> str:
    """Escape a message for GitHub's workflow command syntax."""
    return message.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")


def annotation(level: str, span: dict, text: str) -> str:
    location = ""
    if span:
        file = span.get("file_name", "")
        line = span.get("line_start", 1)
        col = span.get("column_start", 1)
        if file:
            location = f" file={file},line={line},col={col},title=clippy"
    return f"::{level}{location}::{escape(text)}"


def main() -> None:
    diagnostics: list[tuple[str, str, str, dict | None]] = []

    for line in sys.stdin:
        try:
            item = json.loads(line)
        except json.JSONDecodeError:
            continue
        if item.get("reason") != "compiler-message":
            continue

        message = item.get("message", {})
        level = message.get("level", "")
        code = (message.get("code") or {}).get("code", "")
        if level not in ("warning", "error"):
            continue
        if level != "error" and not code.startswith("clippy::"):
            continue

        primary = next(
            (span for span in message.get("spans", []) if span.get("is_primary")),
            None,
        )
        diagnostics.append((level, code, message.get("message", ""), primary))

    # --all-targets checks some crates twice (lib and lib test): report once.
    seen = set()
    unique = []
    for level, code, text, span in diagnostics:
        key = (
            level,
            code,
            text,
            span.get("file_name", "") if span else "",
            span.get("line_start", 0) if span else 0,
            span.get("column_start", 0) if span else 0,
        )
        if key not in seen:
            seen.add(key)
            unique.append((level, code, text, span))
    diagnostics = unique

    for level, code, text, span in diagnostics[:MAX_ANNOTATIONS]:
        label = f"{code}: {text}" if code else text
        print(annotation(level, span, label))

    warnings = sum(1 for level, _, _, _ in diagnostics if level == "warning")
    print(f"clippy reported {warnings} warning(s).")

    output = os.environ.get("GITHUB_OUTPUT")
    if output:
        with open(output, "a", encoding="utf-8") as handle:
            handle.write(f"warning_count={warnings}\n")

    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary and diagnostics:
        with open(summary, "a", encoding="utf-8") as handle:
            handle.write("### Clippy\n\n")
            handle.write(f"{warnings} warning(s) across the workspace.\n\n")
            handle.write("| File | Line | Lint | Message |\n|---|---:|---|---|\n")
            for level, code, text, span in diagnostics[:MAX_ANNOTATIONS]:
                file = span.get("file_name", "") if span else ""
                line = span.get("line_start", "") if span else ""
                clean = text.replace("|", "\\|").replace("\n", " ")
                handle.write(f"| `{file}` | {line} | `{code}` | {clean} |\n")
            handle.write("\n")


if __name__ == "__main__":
    main()
