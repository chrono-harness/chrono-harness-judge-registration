#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p target
scratch=$(mktemp -d "$(pwd -P)/target/check.XXXXXXXX")
trap 'rm -rf -- "$scratch"' EXIT
export TMPDIR="$scratch"
export CARGO_TARGET_DIR="$scratch/cargo"
cargo fmt --check --manifest-path crates/registration-fixture/Cargo.toml
cargo fmt --check --manifest-path crates/judge-registration/Cargo.toml
cargo fmt --check --manifest-path crates/judge-registration-tests/Cargo.toml
product=crates/judge-registration/Cargo.toml
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
cargo test --locked --manifest-path crates/judge-registration-tests/Cargo.toml --config "target.'cfg(all())'.runner=$test_runner"

coverage_failure() {
  echo "E_VERUS_COVERAGE: $*" >&2
  exit 1
}

verifier=$(command -v "${VERUS:-verus}") || coverage_failure "verifier not found"

prove() {
  chrono-host-bootstrap run-compiler-proof "$scratch" "$1" "$verifier" "$2" ||
    coverage_failure "$1: verifier execution failed"
}

admit_proof() {
  local label=$1
  shift
  printf '%s (%s)\n' "$label" "$1" >&2
  chrono-host-bootstrap collect-compiler-proof "$scratch" "$label" "$verifier" \
    0.2026.04.24.f8e1704 f8e17045c4ac17a02e566b8997070f2eecc255be "$@" ||
    coverage_failure "$label: required proof result was not admitted"
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
  src/lib.rs src/registrations.rs src/verified_nodes.rs \
  src/inputs.rs src/verified_readiness.rs src/input_coverage.rs src/verified_input_coverage.rs \
  src/execution.rs src/verified_scheduling.rs src/verified_snapshot_header.rs \
  src/verified_snapshot_records.rs src/schema.rs src/verified_object_fields.rs \
  src/verified_reference_syntax.rs || coverage_failure "native registration decisions differ from required sources"

prove verify.registration-nodes crates/judge-registration/src/verified_nodes.rs
prove verify.registration-readiness crates/judge-registration/src/verified_readiness.rs
prove verify.registration-input-coverage crates/judge-registration/src/verified_input_coverage.rs
prove verify.registration-scheduling crates/judge-registration/src/verified_scheduling.rs
prove verify.registration-nodes-tests crates/judge-registration-tests/tests/verified_nodes.rs
prove verify.registration-readiness-tests crates/judge-registration-tests/tests/verified_readiness.rs
prove verify.registration-input-coverage-tests crates/judge-registration-tests/tests/verified_input_coverage.rs
prove verify.registration-scheduling-tests crates/judge-registration-tests/tests/verified_scheduling.rs
prove verify.registration-snapshot-header-tests crates/judge-registration-tests/tests/verified_snapshot_header.rs
prove verify.registration-snapshot-records-tests crates/judge-registration-tests/tests/verified_snapshot_records.rs
prove verify.registration-reference-syntax-tests crates/judge-registration-tests/tests/reference_syntax.rs

admit_proof verify.registration-nodes crates/judge-registration/src/verified_nodes.rs
admit_proof verify.registration-readiness crates/judge-registration/src/verified_readiness.rs
admit_proof verify.registration-input-coverage crates/judge-registration/src/verified_input_coverage.rs
admit_proof verify.registration-scheduling crates/judge-registration/src/verified_scheduling.rs
admit_proof verify.registration-nodes-tests crates/judge-registration-tests/tests/verified_nodes.rs \
  crates/judge-registration-tests/tests/../../judge-registration/src/verified_nodes.rs
admit_proof verify.registration-readiness-tests crates/judge-registration-tests/tests/verified_readiness.rs \
  crates/judge-registration-tests/tests/../../judge-registration/src/verified_readiness.rs
admit_proof verify.registration-input-coverage-tests crates/judge-registration-tests/tests/verified_input_coverage.rs \
  crates/judge-registration-tests/tests/../../judge-registration/src/verified_input_coverage.rs
admit_proof verify.registration-scheduling-tests crates/judge-registration-tests/tests/verified_scheduling.rs \
  crates/judge-registration-tests/tests/../../judge-registration/src/verified_scheduling.rs
admit_proof verify.registration-snapshot-header-tests crates/judge-registration-tests/tests/verified_snapshot_header.rs \
  crates/judge-registration-tests/tests/../../judge-registration/src/verified_snapshot_header.rs
admit_proof verify.registration-snapshot-records-tests crates/judge-registration-tests/tests/verified_snapshot_records.rs \
  crates/judge-registration-tests/tests/../../judge-registration/src/verified_snapshot_header.rs \
  crates/judge-registration-tests/tests/../../judge-registration/src/verified_snapshot_records.rs \
  crates/judge-registration-tests/tests/../../judge-registration/src/verified_object_fields.rs
admit_proof verify.registration-reference-syntax-tests crates/judge-registration-tests/tests/reference_syntax.rs \
  crates/judge-registration-tests/tests/../../judge-registration/src/verified_reference_syntax.rs
