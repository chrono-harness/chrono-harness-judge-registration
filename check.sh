#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
if [[ $# -eq 0 ]]; then
  /bin/sh .chrono-harness/ci/install.sh
  export PATH="$PWD/.chrono-harness/bin:$PATH"
  export CHRONO_CHECK_SOURCE=${CHRONO_CHECK_SOURCE:-local}
  exec .chrono-harness/bin/chrono-harness check
fi
if [[ $# == 1 && $1 == --native ]]; then
  exec chrono-host-bootstrap native-scratch run /bin/bash ./check.sh --native-in-directory
fi
if [[ $# -ne 1 || $1 != --native-in-directory ]]; then
  echo 'usage: ./check.sh [--native]' >&2
  exit 2
fi

scratch=${CHRONO_NATIVE_SCRATCH:?missing native check directory}
export TMPDIR="$scratch"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$scratch/cargo}"
cargo fmt --check --manifest-path crates/registration-fixture/Cargo.toml
cargo fmt --check --manifest-path crates/judge-registration/Cargo.toml
cargo fmt --check --manifest-path crates/judge-registration-tests/Cargo.toml
product=crates/judge-registration/Cargo.toml
tests=crates/judge-registration-tests/Cargo.toml
# Cargo resolves the selected native test's local packages. FILEMAP owns the
# traversal from each manifest to the test that must run when it changes.
cargo metadata --locked --format-version 1 --filter-platform host-tuple --manifest-path "$tests" |
  jq --arg root "$(pwd -P)/" --slurpfile map .chrono-harness/FILEMAP.json '
    [$map[0].files[] | .path as $path | .edges[] | . + {from: ("file:" + $path)}] as $files
    | [.packages[] | select(.source == null) | .manifest_path | ltrimstr($root)
       | {id: ., node: ("file:" + .), reference: ., reason: "local Cargo package"}] as $manifests
    | {base: [], candidate: ($files + $map[0].project_edges), seeds: $manifests, reachable_only: false}
  ' | chrono-judge-filemap closure-v1 |
  jq -e --arg target 'test:judge-registration-tests' '
    (.closure.predecessors | keys) as $manifests
    | [.closure.traversed[] | select(.edge.kind == "test-execution" and .edge.to == $target)
       | .seeds[]] as $covered
    | ($manifests - $covered) as $missing
    | if ($manifests | length) == 0 then error("Cargo reported no local manifests")
      elif ($missing | length) > 0 then
        error("E_INPUT_UNDECLARED: local Cargo manifests missing a registered path to " + $target + ": " + ($missing | join(", ")))
      else true end
  ' > /dev/null
cargo build --locked --message-format=json-render-diagnostics --manifest-path "$product" > "$scratch/build.jsonl"
export CHRONO_TEST_REGISTRATION="$CARGO_TARGET_DIR/debug/chrono-judge-registration"
CHRONO_TEST_GIT=$(command -v chrono-test-git) || { echo "chrono-test-git is required by registration tests" >&2; exit 1; }
export CHRONO_TEST_GIT
# Cargo invokes the test executables with the fixture first on PATH.
# Build scripts and the compiler keep their normal environment.
mkdir "$scratch/test-bin"
ln -s "$CHRONO_TEST_GIT" "$scratch/test-bin/git"
ln -s "$CARGO_TARGET_DIR/debug/chrono-harness-test-cli" "$scratch/test-bin/chrono-harness"
test_runner=$(jq -cn --arg path "PATH=$scratch/test-bin" '["/usr/bin/env", $path]')
cargo test --locked --manifest-path "$tests" --config "target.'cfg(all())'.runner=$test_runner" --color never -- --format pretty | tee "$scratch/tests.log"
for consumer in \
  execution_plan_queries_enforce_operation_identity_and_both_resource_limits \
  execution_plan_limits_are_inclusive_and_require_time_and_output \
  classification_preparation_consumes_each_endpoint_and_starts_only_admitted_selectors \
  protocol_field_errors_precede_launch_and_positive_values_are_preserved \
  git_facts::input_closure_bindings_accept_explicit_external_nodes \
  git_facts::required_candidate_snapshot_is_rejected_by_registration \
  retained_input_duplicates_check_each_descriptor_and_refresh_on_each_validation; do
  grep -Fxq "test $consumer ... ok" "$scratch/tests.log" || {
    echo "E_VERUS_COVERAGE: required registration consumer did not pass: $consumer" >&2
    exit 1
  }
done

coverage_failure() {
  echo "E_VERUS_COVERAGE: $*" >&2
  exit 1
}

# Read the exact library selected by Cargo in the build of the tested executable.
metadata=$(jq -ers --arg manifest "$(pwd -P)/$product" '
  [.[] | select(.reason == "compiler-artifact" and .manifest_path == $manifest
                 and .target.kind == ["lib"]) | .filenames[] | select(endswith(".rmeta"))]
  | if length == 1 then .[0] else error("expected the product library") end
' "$scratch/build.jsonl") || coverage_failure "native library was not identified"
filename=${metadata##*/}
stem=${filename#lib}
stem=${metadata%/*}/${stem%.rmeta}
chrono-host-bootstrap check-compiler-sources native "$stem" \
  src/lib.rs src/registrations.rs src/verified_nodes.rs src/verified_path_components.rs \
  src/inputs.rs src/verified_readiness.rs src/input_coverage.rs src/verified_input_coverage.rs \
  src/execution.rs src/classification_inputs.rs src/verified_scheduling.rs src/verified_snapshot_header.rs \
  src/verified_snapshot_records.rs src/schema.rs src/verified_object_fields.rs \
  src/verified_reference_syntax.rs || coverage_failure "native registration decisions differ from required sources"
