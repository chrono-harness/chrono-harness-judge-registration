"""Select the immutable Copier dev merge from its public GitHub landing records."""
import json
import os
import re
from pathlib import Path
from urllib.request import urlopen


def public_read(repository, path):
    with urlopen(f"https://api.github.com/repos/{repository}/{path}", timeout=30) as reply:
        return json.load(reply)


def codeql_attempt(run_id, attempt, read):
    run = read(f"actions/runs/{run_id}/attempts/{attempt}")
    if (run["id"], run["run_attempt"]) != (run_id, attempt):
        raise ValueError("unexpected CodeQL run or attempt")
    return run


def select(repository, run, read):
    if not (
        run["repository"]["full_name"] == run["head_repository"]["full_name"] == repository
        and run["path"] == "dynamic/github-code-scanning/codeql"
        and run["event"] == "dynamic"
        and run["head_branch"] == "dev"
        and run["status"] == "completed"
        and run["conclusion"] == "success"
        and re.fullmatch(r"[0-9a-f]{40}", run["head_sha"])
    ):
        raise ValueError("expected a successful CodeQL run on this repository's dev branch")
    sha = run["head_sha"]
    candidates = []
    page = 1
    while True:
        rows = read(f"commits/{sha}/pulls?per_page=100&page={page}")
        candidates.extend(
            pr for pr in rows if pr["merge_commit_sha"] == sha and pr["merged_at"]
            and pr["base"]["ref"] == "dev" and pr["base"]["repo"]["full_name"] == repository
            and pr["user"]["login"] == "dependabot[bot]"
        )
        if len(rows) < 100:
            break
        page += 1
    if not candidates:
        return None
    if len(candidates) != 1:
        raise ValueError("ambiguous landed dependency pull request")
    number = candidates[0]["number"]
    pr = read(f"pulls/{number}")
    if (pr["number"] != number or not pr.get("merged") or not pr.get("merged_at")
            or pr["merge_commit_sha"] != sha or pr["base"]["repo"]["full_name"] != repository
            or pr["base"]["ref"] != "dev" or pr["user"]["login"] != "dependabot[bot]"):
        raise ValueError("pull request does not identify this merge")
    if (pr.get("merged_by") or {}).get("login") != "github-actions[bot]":
        return None
    if ((pr.get("auto_merge") or {}).get("enabled_by") or {}).get("login") != "github-actions[bot]":
        return None
    commit = read(f"git/commits/{sha}")
    parents = [parent["sha"] for parent in commit["parents"]]
    if (commit["sha"] != sha or len(parents) != 2
            or any(not re.fullmatch(r"[0-9a-f]{40}", parent) for parent in parents)
            or pr["head"]["repo"]["full_name"] != repository or parents[1] != pr["head"]["sha"]):
        raise ValueError("Git merge parents do not bind the dependency pull request head")
    return {"pr": number, "base": parents[0], "head": parents[1], "sha": sha}


def bind(repository, event_name, event, read):
    if event_name == "workflow_run":
        run = event["workflow_run"]
    elif event_name == "workflow_dispatch":
        source = event["inputs"]["codeql_attempt"]
        if not re.fullmatch(r"[1-9][0-9]*/[1-9][0-9]*", source):
            raise ValueError("expected a CodeQL run ID and attempt")
        run_id, attempt = map(int, source.split("/"))
        run = codeql_attempt(run_id, attempt, read)
    else:
        raise ValueError("unsupported landing event")
    result = select(repository, run, read)
    if result is None and event_name == "workflow_dispatch":
        raise ValueError("recovery requires a landed dependency merge by GitHub Actions")
    if result is not None:
        result.update(codeql_run=run["id"], codeql_attempt=run["run_attempt"])
    return result


if __name__ == "__main__":
    repository = os.environ["GITHUB_REPOSITORY"]
    def read(path):
        return public_read(repository, path)
    event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_bytes())
    result = bind(repository, os.environ["GITHUB_EVENT_NAME"], event, read)
    if result is None:
        print("No Actions-merged dependency pull request at this CodeQL commit")
    else:
        print(json.dumps(result, sort_keys=True))
        with open(os.environ["GITHUB_OUTPUT"], "a") as output:
            print("sha=" + result["sha"], file=output)
            print("base=" + result["base"], file=output)
