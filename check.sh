#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
if [[ $# -eq 0 ]]; then
  /bin/sh .chrono-harness/ci/install.sh
  export PATH="$PWD/.chrono-harness/bin:$PATH"
  export CHRONO_CHECK_SOURCE=${CHRONO_CHECK_SOURCE:-local}
  exec .chrono-harness/bin/chrono-harness check
fi
if [[ $# -ne 1 || $1 != --native ]]; then
  echo 'usage: ./check.sh [--native]' >&2
  exit 2
fi

mkdir -p target
scratch=$(mktemp -d "$(pwd -P)/target/check.XXXXXXXX")
trap 'rm -rf -- "$scratch"' EXIT
export TMPDIR="$scratch"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$scratch/cargo}"
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
cargo test --locked --manifest-path crates/judge-registration-tests/Cargo.toml --config "target.'cfg(all())'.runner=$test_runner" --color never -- --format pretty | tee "$scratch/tests.log"
for consumer in \
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
  src/lib.rs src/registrations.rs src/verified_nodes.rs \
  src/inputs.rs src/verified_readiness.rs src/input_coverage.rs src/verified_input_coverage.rs \
  src/execution.rs src/verified_scheduling.rs src/verified_snapshot_header.rs \
  src/verified_snapshot_records.rs src/schema.rs src/verified_object_fields.rs \
  src/verified_reference_syntax.rs || coverage_failure "native registration decisions differ from required sources"
