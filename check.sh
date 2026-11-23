#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
scratch=$(mktemp -d)
trap 'rm -rf -- "$scratch"' EXIT
export CARGO_TARGET_DIR="$scratch/cargo"
cargo fmt --check --manifest-path crates/registration-fixture/Cargo.toml
cargo fmt --check --manifest-path crates/judge-registration/Cargo.toml
cargo fmt --check --manifest-path crates/judge-registration-tests/Cargo.toml
cargo build --locked --manifest-path crates/judge-registration/Cargo.toml
CHRONO_TEST_GIT=$(command -v chrono-test-git) || { echo "chrono-test-git is required by registration tests" >&2; exit 1; }
CHRONO_TEST_GIT_PROFILE_BIN=$(command -v chrono-test-git-profile) || { echo "chrono-test-git-profile is required by registration tests" >&2; exit 1; }
export CHRONO_TEST_GIT CHRONO_TEST_GIT_PROFILE_BIN
cargo test --locked --manifest-path crates/judge-registration-tests/Cargo.toml

prove() {
  local label=$1 root=$2
  local output="$scratch/$label.rlib"
  "${VERUS:-verus}" --crate-type=lib --no-cheating --no-external-by-default --num-threads 2 \
    --output-json --compile "--emit=link=$output,dep-info=$output.d" "$root" -o "$output" \
    -Zchecksum-hash-algorithm=sha256 > "$scratch/proof.json" || { cat "$scratch/proof.json" >&2; return 1; }
  cat "$scratch/proof.json"
  jq -e '.verus.version == "0.2026.04.24.f8e1704" and
    .verus.commit == "f8e17045c4ac17a02e566b8997070f2eecc255be" and
    (."verification-results" | .success == true and ."is-verifying-entire-crate" == true and
    ."encountered-error" == false and ."encountered-vir-error" == false and
    .errors == 0 and .verified > 0)' "$scratch/proof.json" >/dev/null
  test -s "$output" && test -s "$output.d"
}

prove verify.registration-preparation crates/judge-registration/src/verified_consumption.rs
prove verify.registration-nodes crates/judge-registration/src/verified_nodes.rs
prove verify.registration-readiness crates/judge-registration/src/verified_readiness.rs
prove verify.registration-input-coverage crates/judge-registration/src/verified_input_coverage.rs
prove verify.registration-scheduling crates/judge-registration/src/verified_scheduling.rs
prove verify.registration-preparation-tests crates/judge-registration-tests/tests/verified_consumption.rs
prove verify.registration-nodes-tests crates/judge-registration-tests/tests/verified_nodes.rs
prove verify.registration-readiness-tests crates/judge-registration-tests/tests/verified_readiness.rs
prove verify.registration-input-coverage-tests crates/judge-registration-tests/tests/verified_input_coverage.rs
prove verify.registration-scheduling-tests crates/judge-registration-tests/tests/verified_scheduling.rs
prove verify.registration-snapshot-header-tests crates/judge-registration-tests/tests/verified_snapshot_header.rs
