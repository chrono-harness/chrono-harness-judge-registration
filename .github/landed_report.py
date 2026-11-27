"""Read the installed host's local check and select its evidence for the landing artifact."""
import json
import os
from pathlib import Path
import subprocess
import sys


def runner_read(request):
    # A single operation owns its read leases until the runner has returned the bytes.
    result = subprocess.run(
        [".chrono-harness/bin/chrono-harness", "retained-artifacts-v1"],
        input=json.dumps(request) + "\n", capture_output=True, text=True, check=True,
    )
    reply = json.loads(result.stdout)
    if "Ok" not in reply:
        raise ValueError(reply["Err"])
    return reply["Ok"]


def artifact_path(name):
    path = Path(name)
    if (not path.is_relative_to(".chrono-harness/state") or path.as_posix() != name
            or ".." in path.parts or any(c in name for c in "\r\n*?[]!{}")
            or any(part.is_symlink() for part in [path, *path.parents])):
        raise ValueError(f"invalid landed check artifact path: {name}")
    return path


def landed_report(base, candidate, paths, read=runner_read):
    host = json.loads(Path(".chrono-harness/config.json").read_bytes())
    profile_path = host["canonical_check"]["profile"]
    profile = read({"operation": "scoped-check-profile", "path": profile_path})
    slot = artifact_path(profile["report_path"])
    paths.append(slot.parent)
    settings = json.loads(Path(profile_path).read_bytes())
    limit = settings["policy"].get("collection_limits", {}).get("report_bytes", 64 << 20)
    check = json.loads(read({"operation": "read-bounded-text", "path": str(slot), "limit": limit}))
    if profile["retained_reference"]:
        if check["schema"] != "chrono-check-reference/v1":
            raise ValueError("expected a retained check reference")
        original = check["original"]
        original_path = artifact_path(original["path"])
        paths.append(original_path)
        def read_reference(ref, operation):
            nonlocal limit
            path = artifact_path(ref["path"])
            paths.append(path)
            limit -= path.stat().st_size
            if limit < 0:
                raise ValueError("landed report exceeds the registered byte limit")
            return read({"operation": operation, "original": ref})

        report = json.loads(read_reference(original, "read-original"))
        if (report["schema"] != "chrono-check-report/v2" or report["retained_report"] != str(original_path)
                or "response" in report or report["publication"]["slot"] != str(slot)):
            raise ValueError("unexpected retained check publication")
        for name in report["publication"]["originals"]:
            path = artifact_path(name)
            if not path.is_file():
                raise ValueError(f"missing publication original: {name}")
            if not path.is_relative_to(slot.parent):
                paths.append(path)

        judge = report["judge"]
        if any(key in judge for key in ("stdout", "stderr", "stdout_bytes", "stderr_bytes")):
            raise ValueError("retained process contains a second stream representation")
        for stream in ("stdout", "stderr"):
            ref = judge[stream + "_original"]
            if not judge[stream + "_complete"] or ref["sha256"] != judge[stream + "_sha256"]:
                raise ValueError(f"incomplete or mismatched judge {stream}")
            if stream == "stdout":
                response = json.loads(read_reference(ref, "read-original"))
            else:
                read_reference(ref, "verify-original")
        if judge.get("launch_original") is not None:
            read_reference(judge["launch_original"], "verify-original")
    else:
        report = check
        if report["schema"] != "chrono-check-report/v1":
            raise ValueError("expected an inline check report")
        response = report["response"]

    if (report["request"]["base"] != base or report["request"]["candidate"] != candidate
            or report["request"]["request_id"] != response["request_id"]
            or report["judge"]["exit_code"] != 0 or report["publication"]["exit_code"] != 0
            or report["judge"]["failure"] is not None or not report["judge"]["owned_work_joined"]
            or response["status"] != "passed"):
        raise ValueError("check result does not confirm the selected landed range")


if __name__ == "__main__":
    paths = []
    try:
        landed_report(*sys.argv[1:], paths=paths)
    finally:
        if paths:
            with open(os.environ["GITHUB_OUTPUT"], "a") as output:
                print("paths<<LANDED_CHECK_PATHS", file=output)
                for path in dict.fromkeys(paths):
                    print(Path("host") / path, file=output)
                print("LANDED_CHECK_PATHS", file=output)
