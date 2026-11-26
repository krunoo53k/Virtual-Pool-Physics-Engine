#!/usr/bin/env python3
"""The corpus/model-version guard.

Rule: if the hash of an *existing* corpus shot changes, the physics model
version must change too. If the model version changes while no shot hash
moves, a written justification is required. Adding shots changes the file
digest but not the physics digest; that is allowed once the manifest's file
digest is updated.

The corpus is self-generated: it proves change, not correctness. This guard is
what stops a physics edit from slipping through under an unchanged version.
"""

import hashlib
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
CORPUS = ROOT / "tests" / "fixtures" / "corpus.json"
MANIFEST = ROOT / "tests" / "fixtures" / "corpus_manifest.json"


def file_digest(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def hashes_by_id(document: dict) -> dict:
    return {
        shot["id"]: shot["final_state_hash"] for shot in document["shots"]
    }


def existing_shots_changed(current: dict, baseline: dict) -> bool:
    """True when a shot already in the baseline has a different hash or vanished.

    New shots are coverage, not a physics change: they are tracked by the
    corpus file digest instead.
    """
    return any(
        shot_id not in current or current[shot_id] != digest
        for shot_id, digest in baseline.items()
    )


def model_version() -> str:
    text = (ROOT / "pool-core" / "src" / "lib.rs").read_text()
    found = re.search(r'MODEL_VERSION:\s*&str\s*=\s*"([^"]+)"', text)
    if not found:
        sys.exit("cannot find MODEL_VERSION in pool-core/src/lib.rs")
    return found.group(1)


def main() -> int:
    document = json.loads(CORPUS.read_text())
    manifest = json.loads(MANIFEST.read_text())
    model = model_version()

    current_file = file_digest(CORPUS)
    current_hashes = hashes_by_id(document)
    justification = manifest.get("justification", "").strip()

    physics_changed = existing_shots_changed(current_hashes, manifest["baseline_hashes"])
    coverage_changed = current_file != manifest["corpus_sha256"]
    model_changed = model != manifest["model_version"]

    problems = []
    if physics_changed and not model_changed and not justification:
        problems.append(
            "A corpus shot hash changed but MODEL_VERSION did not. "
            "Bump the model (outcomes changed) or write a justification in "
            "tests/fixtures/corpus_manifest.json."
        )
    if model_changed and not physics_changed and not justification:
        problems.append(
            "MODEL_VERSION changed but no corpus shot hash did. "
            "Write a justification in tests/fixtures/corpus_manifest.json."
        )
    if coverage_changed and not justification:
        problems.append(
            "The corpus file changed without updating "
            "tests/fixtures/corpus_manifest.json (corpus_sha256)."
        )
    # The one-source compat manifest must agree with this workspace.
    compat = json.loads((ROOT / "compat.json").read_text())
    if compat["model"] != model:
        problems.append("compat.json model does not match pool-core::MODEL_VERSION")
    if compat["corpus_sha256"] != current_file:
        problems.append("compat.json corpus_sha256 does not match corpus.json")
    if compat["rules_version"] != "1.0":
        problems.append("compat.json rules_version does not match pool-rules")

    for problem in problems:
        print(f"ERROR: {problem}", file=sys.stderr)
    if problems:
        return 1

    print(
        f"corpus guard OK: model {model}, {len(document['shots'])} shots, "
        f"file {current_file[:16]}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
