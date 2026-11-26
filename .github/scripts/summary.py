#!/usr/bin/env python3
"""Write a one-table CI overview to the GitHub Actions job summary."""

from __future__ import annotations

import os
import sys
import xml.etree.ElementTree as ET

MARKS = {
    "success": "✅",
    "failure": "❌",
    "cancelled": "⏹️",
    "skipped": "⏭️",
}


def mark(result: str) -> str:
    return MARKS.get(result.lower(), "❔")


def read_junit(path: str) -> tuple[dict[str, int] | None, list[str]]:
    try:
        root = ET.parse(path).getroot()
    except (OSError, ET.ParseError):
        return None, []

    def total(attribute: str) -> int:
        return sum(int(suite.get(attribute, 0) or 0) for suite in root.iter("testsuite"))

    counts = {
        "tests": total("tests"),
        "failures": total("failures"),
        "errors": total("errors"),
        "skipped": sum(1 for case in root.iter("testcase") if case.find("skipped") is not None),
    }
    failed = [
        f"{case.get('classname', '?')}::{case.get('name', '?')}"
        for case in root.iter("testcase")
        if any(child.tag in ("failure", "error") for child in case)
    ]
    return counts, failed


def main() -> None:
    junit = sys.argv[1] if len(sys.argv) > 1 else "test-results/junit.xml"
    counts, failed = read_junit(junit)

    test_details = ""
    if counts:
        test_details = (
            f"{counts['tests']} run, {counts['failures'] + counts['errors']} failed, "
            f"{counts['skipped']} skipped"
        )

    warnings = os.environ.get("CLIPPY_WARNINGS", "")
    clippy_details = f"{warnings} warning(s)" if warnings.isdigit() else ""

    rows = [
        ("Format", os.environ.get("FMT_RESULT", "unknown"), ""),
        ("Build", os.environ.get("BUILD_RESULT", "unknown"), ""),
        ("Tests", os.environ.get("TESTS_RESULT", "unknown"), test_details),
        ("Clippy", os.environ.get("CLIPPY_RESULT", "unknown"), clippy_details),
        ("Demos", os.environ.get("DEMOS_RESULT", "unknown"), ""),
        ("Corpus (3 targets)", os.environ.get("CORPUS_RESULT", "unknown"), ""),
        (
            "Corpus compile (wasm32)",
            os.environ.get("CORPUS_COMPILE_RESULT", "unknown"),
            "",
        ),
        (
            "Corpus / version guard",
            os.environ.get("COMPAT_GUARD_RESULT", "unknown"),
            "",
        ),
    ]

    lines = ["## CI overview", "", "| Check | Result | Details |", "|---|---|---|"]
    for name, result, details in rows:
        lines.append(f"| {name} | {mark(result)} {result} | {details} |")
    lines.append("")

    if failed:
        lines.append("<details><summary>Failed tests</summary>\n")
        for name in failed[:50]:
            lines.append(f"- `{name}`")
        if len(failed) > 50:
            lines.append(f"- … and {len(failed) - 50} more")
        lines.append("\n</details>\n")

    text = "\n".join(lines)
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a", encoding="utf-8") as handle:
            handle.write(text + "\n")
    else:
        print(text)


if __name__ == "__main__":
    main()
