//! Exercise registration's production parser with malformed and exact retained records.
#![allow(unexpected_cfgs)]
use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::{string::StringSliceAdditionalSpecFns, vpanic as panic};
#[cfg(verus_keep_ghost)]
#[path = "../../judge-registration/src/inputs.rs"]
mod inputs;
#[cfg(verus_keep_ghost)]
#[path = "../../judge-registration/src/verified_object_fields.rs"]
mod schema;
#[cfg(verus_keep_ghost)]
mod policy {
    pub use super::{inputs::*, schema::*};
}
#[cfg(not(verus_keep_ghost))]
mod policy {
    pub use chrono_judge_registration::inputs::*;
    pub use chrono_judge_registration::{
        ObjectFieldError, object_error_parts, validate_object_fields,
    };
}

verus! {
fn expect(value: bool)
    requires value,
{ if !value { panic!(); } }

#[cfg_attr(not(verus_keep_ghost), test)]
fn object_shape_rejects_nonobjects_before_inspecting_fields() {
    let result = policy::validate_object_fields(None::<u64>, &["missing"], &[], &["unexpected"]);
    match result {
        Err(policy::ObjectFieldError::ExpectedObject) => {},
        _ => { expect(false); },
    }
}

fn object_shape_keeps_the_exact_successful_value(value: u64) {
    let required = ["blob", "sha256"];
    let optional = ["length", "blob"];
    let keys = ["sha256", "length", "blob"];
    let result = policy::validate_object_fields(Some(value), &required, &optional, &keys);
    match result {
        Ok(actual) => expect(actual == value),
        Err(_) => { expect(false); },
    }
    let empty = policy::validate_object_fields(Some(value), &[], &[], &[]);
    match empty {
        Ok(actual) => expect(actual == value),
        Err(_) => { expect(false); },
    }
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn object_shape_accepts_only_complete_known_fields_and_preserves_values() {
    object_shape_keeps_the_exact_successful_value(0);
    object_shape_keeps_the_exact_successful_value(u64::MAX);
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn object_shape_reports_the_first_required_field_without_any_keys() {
    let required = ["first", "second"];
    let keys: [&str; 0] = [];
    proof { assert(required@[0]@ == "first"@); }
    let result = policy::validate_object_fields(Some(7u64), &required, &[], &keys);
    match result {
        Err(policy::ObjectFieldError::Missing(name)) => {
            let first = name == "first";
            expect(first);
        },
        _ => { expect(false); },
    }
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn object_shape_missing_required_precedes_an_unknown_key() {
    proof {
        reveal_strlit("required"); reveal_strlit("unknown");
        assert("required"@[0] != "unknown"@[0]);
    }
    let result = policy::validate_object_fields(Some(1u64), &["required"], &[], &["unknown"]);
    match result {
        Err(policy::ObjectFieldError::Missing(name)) => {
            let same = name == "required";
            expect(same);
        },
        _ => { expect(false); },
    }
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn object_shape_reports_the_first_unknown_key_after_all_required_fields() {
    proof {
        reveal_strlit("known"); reveal_strlit("first"); reveal_strlit("second");
        assert("known"@[0] != "first"@[0]);
        assert("known"@[0] != "second"@[0]);
    }
    let result = policy::validate_object_fields(
        Some(9u64), &["known"], &[], &["known", "first", "second"],
    );
    match result {
        Err(policy::ObjectFieldError::Unknown(name)) => {
            let same = name == "first";
            expect(same);
        },
        _ => { expect(false); },
    }
}

fn object_shape_errors_preserve_the_complete_field_name(name: &str) {
    let missing = policy::object_error_parts(policy::ObjectFieldError::Missing(name));
    let prefix = missing[0] == "missing field ";
    let field = missing[1] == name;
    expect(prefix && field);
    let unknown = policy::object_error_parts(policy::ObjectFieldError::Unknown(name));
    let prefix = unknown[0] == "unknown field ";
    let field = unknown[1] == name;
    expect(prefix && field);
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn object_shape_error_messages_keep_empty_unicode_nul_and_newline_names() {
    object_shape_errors_preserve_the_complete_field_name("");
    object_shape_errors_preserve_the_complete_field_name("字段\0original\nname");
    let expected = policy::object_error_parts(policy::ObjectFieldError::ExpectedObject);
    let prefix = expected[0] == "expected object";
    let suffix = expected[1] == "";
    expect(prefix && suffix);
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn retained_absence_checks_fields_before_the_boolean_value() {
    proof {
        reveal_strlit("absent"); reveal_strlit("bytes");
        assert("absent"@[0] != "bytes"@[0]);
        reveal(policy::retained_selection);
    }
    let observed = policy::RetainedObservation {
        object: true, keys: vec!["absent", "bytes"], absent: Some(Some(false)),
        bytes: Some(7u64), blob: None, digest: None, length: None,
    };
    proof {
        assert(policy::retained_mode(observed) == 0);
        assert(policy::retained_field_failure(observed, policy::ObjectFieldError::Unknown("bytes")));
    }
    let result = policy::select_retained(observed);
    match result {
        Err(policy::RetainedError::Fields(policy::ObjectFieldError::Unknown(name))) => {
            let exact = name == "bytes";
            expect(exact);
        },
        _ => { expect(false); },
    }
}

fn retained_decode(calls: Vec<u64>, input: u64) -> (out: (Vec<u64>, Result<Vec<u8>, ()>))
    ensures out.0@ == calls@.push(input),
        match out.1 {
            Ok(bytes) => input != 0 && bytes@ == seq![0u8, 255u8],
            Err(()) => input == 0,
        },
{
    let mut calls = calls;
    calls.push(input);
    if input == 0 { (calls, Err(())) } else { (calls, Ok(vec![0u8, 255u8])) }
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn retained_absence_requires_true_and_never_decodes_bytes() {
    proof {
        reveal(policy::retained_selection);
        reveal(policy::retained_decoding);
    }
    let valid = policy::RetainedObservation::<u64> {
        object: true, keys: vec!["absent"], absent: Some(Some(true)),
        bytes: None, blob: None, digest: None, length: None,
    };
    proof { assert(policy::retained_fields_ok(valid)); }
    let (calls, result) = policy::parse_retained(Vec::new(), valid, retained_decode);
    expect(calls.len() == 0);
    match result {
        Ok(policy::Retained::Absent) => {},
        _ => { expect(false); },
    }
    let invalid = policy::RetainedObservation::<u64> {
        object: true, keys: vec!["absent"], absent: Some(None),
        bytes: None, blob: None, digest: None, length: None,
    };
    proof { assert(policy::retained_fields_ok(invalid)); }
    let (calls, result) = policy::parse_retained(Vec::new(), invalid, retained_decode);
    expect(calls.len() == 0);
    match result {
        Err(policy::RetainedError::Absent) => {},
        _ => { expect(false); },
    }
    let false_value = policy::RetainedObservation::<u64> {
        object: true, keys: vec!["absent"], absent: Some(Some(false)),
        bytes: None, blob: None, digest: None, length: None,
    };
    proof { assert(policy::retained_fields_ok(false_value)); }
    match policy::select_retained(false_value) {
        Err(policy::RetainedError::Absent) => {},
        _ => { expect(false); },
    }
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn retained_bytes_and_blob_must_have_exactly_one_present_key() {
    let both = policy::RetainedObservation {
        object: true, keys: vec!["bytes", "blob"], absent: None,
        bytes: Some(u64::MAX), blob: Some(None), digest: None, length: None,
    };
    let (calls, result) = policy::parse_retained(Vec::new(), both, retained_decode);
    expect(calls.len() == 0);
    match result {
        Err(policy::RetainedError::Representation) => {},
        _ => { expect(false); },
    }
    let neither = policy::RetainedObservation::<u64> {
        object: false, keys: Vec::new(), absent: None,
        bytes: None, blob: None, digest: None, length: None,
    };
    let (calls, result) = policy::parse_retained(Vec::new(), neither, retained_decode);
    expect(calls.len() == 0);
    match result {
        Err(policy::RetainedError::Representation) => {},
        _ => { expect(false); },
    }
}

fn retained_bytes_preserve_decoder_calls_and_original_output(input: u64) {
    proof {
        reveal(policy::retained_selection);
        reveal(policy::retained_decoding);
    }
    let observed = policy::RetainedObservation {
        object: true, keys: vec!["bytes"], absent: None,
        bytes: Some(input), blob: None, digest: None, length: None,
    };
    proof {
        assert(policy::retained_mode(observed) == 1);
        assert(policy::retained_fields_ok(observed));
    }
    let (calls, result) = policy::parse_retained(vec![31u64], observed, retained_decode);
    expect(calls.len() == 2 && calls[0] == 31 && calls[1] == input);
    match result {
        Ok(policy::Retained::Bytes(bytes)) => {
            expect(input != 0 && bytes.len() == 2 && bytes[0] == 0 && bytes[1] == 255);
        },
        Err(policy::RetainedError::Bytes) => { expect(input == 0); },
        _ => { expect(false); },
    }
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn retained_bytes_decode_once_after_fields_and_keep_success_or_failure() {
    proof {
        reveal(policy::retained_selection);
        reveal(policy::retained_decoding);
    }
    retained_bytes_preserve_decoder_calls_and_original_output(0);
    retained_bytes_preserve_decoder_calls_and_original_output(1);
    retained_bytes_preserve_decoder_calls_and_original_output(u64::MAX);
    proof {
        reveal_strlit("bytes"); reveal_strlit("unexpected");
        assert("bytes"@.len() != "unexpected"@.len());
    }
    let unknown = policy::RetainedObservation {
        object: true, keys: vec!["bytes", "unexpected"], absent: None,
        bytes: Some(0u64), blob: None, digest: None, length: None,
    };
    proof {
        assert(policy::retained_mode(unknown) == 1);
        assert(policy::retained_field_failure(unknown, policy::ObjectFieldError::Unknown("unexpected")));
    }
    let (calls, result) = policy::parse_retained(Vec::new(), unknown, retained_decode);
    expect(calls.len() == 0);
    match result {
        Err(policy::RetainedError::Fields(policy::ObjectFieldError::Unknown(name))) => {
            let exact = name == "unexpected";
            expect(exact);
        },
        _ => { expect(false); },
    }
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn retained_blob_requires_fields_before_path_digest_or_length_checks() {
    proof {
        reveal_strlit("blob"); reveal_strlit("sha256"); reveal_strlit("length");
        reveal_strlit("unknown");
        assert("blob"@.len() != "sha256"@.len());
        assert("blob"@.len() != "length"@.len());
        assert("sha256"@[0] != "length"@[0]);
        assert("unknown"@.len() != "blob"@.len());
        assert("unknown"@.len() != "sha256"@.len());
        assert("unknown"@.len() != "length"@.len());
        reveal(policy::retained_selection);
        reveal(policy::retained_decoding);
    }
    let missing = policy::RetainedObservation::<u64> {
        object: true, keys: vec!["blob", "unknown"], absent: None,
        bytes: None, blob: Some(None), digest: None, length: None,
    };
    proof {
        assert(policy::retained_mode(missing) == 2);
        assert(policy::retained_field_failure(missing, policy::ObjectFieldError::Missing("sha256")));
    }
    let (calls, result) = policy::parse_retained(Vec::new(), missing, retained_decode);
    expect(calls.len() == 0);
    match result {
        Err(policy::RetainedError::Fields(policy::ObjectFieldError::Missing(name))) => {
            let exact = name == "sha256";
            expect(exact);
        },
        _ => { expect(false); },
    }
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn retained_blob_reports_the_first_unknown_field_before_checking_values() {
    proof {
        reveal_strlit("blob"); reveal_strlit("sha256"); reveal_strlit("length");
        reveal_strlit("unknown");
        assert("unknown"@.len() != "blob"@.len());
        assert("unknown"@.len() != "sha256"@.len());
        assert("unknown"@.len() != "length"@.len());
        reveal(policy::retained_selection);
    }
    let unknown = policy::RetainedObservation::<u64> {
        object: true, keys: vec!["blob", "sha256", "length", "unknown", ""], absent: None,
        bytes: None, blob: Some(None), digest: None, length: None,
    };
    let result = policy::select_retained(unknown);
    match result {
        Err(policy::RetainedError::Fields(policy::ObjectFieldError::Unknown(name))) => {
            let exact = name == "unknown";
            expect(exact);
        },
        _ => { expect(false); },
    }
}

fn retained_blob_boundaries(path: &str, digest: &str, length: u64)
    requires policy::blob_path(path.spec_bytes()),
        policy::blob_digest(digest.spec_bytes()),
{
    let success = policy::RetainedObservation::<u64> {
        object: true, keys: vec!["length", "sha256", "blob"], absent: None,
        bytes: None, blob: Some(Some(path)), digest: Some(digest), length: Some(length),
    };
    let (calls, result) = policy::parse_retained(Vec::new(), success, retained_decode);
    expect(calls.len() == 0);
    match result {
        Ok(policy::Retained::Blob { path: actual_path, digest: actual_digest, length: actual_length }) => {
            let same_path = actual_path == path;
            let same_digest = actual_digest == digest;
            expect(same_path && same_digest && actual_length == length);
        },
        _ => { expect(false); },
    }
    let bad_path = policy::RetainedObservation::<u64> {
        object: true, keys: vec!["blob", "sha256", "length"], absent: None,
        bytes: None, blob: Some(None), digest: None, length: None,
    };
    match policy::select_retained(bad_path) {
        Err(policy::RetainedError::Path) => {},
        _ => { expect(false); },
    }
    let bad_digest = policy::RetainedObservation::<u64> {
        object: true, keys: vec!["blob", "sha256", "length"], absent: None,
        bytes: None, blob: Some(Some(path)), digest: Some(""), length: None,
    };
    proof { reveal_strlit(""); vstd::utf8::is_ascii_chars_encode_utf8(""@); }
    match policy::select_retained(bad_digest) {
        Err(policy::RetainedError::Digest) => {},
        _ => { expect(false); },
    }
    let bad_length = policy::RetainedObservation::<u64> {
        object: true, keys: vec!["blob", "sha256", "length"], absent: None,
        bytes: None, blob: Some(Some(path)), digest: Some(digest), length: None,
    };
    match policy::select_retained(bad_length) {
        Err(policy::RetainedError::Length) => {},
        _ => { expect(false); },
    }
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn retained_blob_checks_path_then_digest_then_length_and_preserves_boundary_values() {
    let path = ".chrono-harness/state/x";
    let digest = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    proof {
        reveal_strlit(".chrono-harness/state/x");
        reveal_strlit("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef");
        vstd::utf8::is_ascii_chars_encode_utf8(path@);
        vstd::utf8::is_ascii_chars_encode_utf8(digest@);
        let prefix = ".chrono-harness/state/";
        reveal_strlit(".chrono-harness/state/");
        vstd::utf8::is_ascii_chars_encode_utf8(prefix@);
        assert(path.spec_bytes().take(prefix.spec_bytes().len() as int) =~= prefix.spec_bytes());
        assert forall|i: int| prefix.spec_bytes().len() <= i < path.spec_bytes().len()
            implies #[trigger] policy::blob_byte(path.spec_bytes(), i) by {
            assert(i == 22);
        }
        assert(policy::blob_path(path.spec_bytes()));
        assert forall|i: int| 0 <= i < digest.spec_bytes().len() implies
            (48 <= #[trigger] digest.spec_bytes()[i] <= 57 || 97 <= digest.spec_bytes()[i] <= 102) by {
            assert(digest.spec_bytes()[i] == digest@[i] as u8);
        }
        assert(policy::blob_digest(digest.spec_bytes()));
    }
    retained_blob_boundaries(path, digest, 0);
    retained_blob_boundaries(path, digest, u64::MAX);
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn retained_nonobject_precedes_fields_and_invalid_paths_precede_other_values() {
    let nonobject = policy::RetainedObservation {
        object: false, keys: vec!["unknown"], absent: Some(None),
        bytes: Some(7u64), blob: Some(None), digest: None, length: None,
    };
    let (calls, result) = policy::parse_retained(Vec::new(), nonobject, retained_decode);
    expect(calls.len() == 0);
    match result {
        Err(policy::RetainedError::Fields(policy::ObjectFieldError::ExpectedObject)) => {},
        _ => { expect(false); },
    }
    let bad_path = policy::RetainedObservation::<u64> {
        object: true, keys: vec!["blob", "sha256", "length"], absent: None,
        bytes: None, blob: Some(Some("")), digest: None, length: None,
    };
    proof { reveal_strlit(""); vstd::utf8::is_ascii_chars_encode_utf8(""@); }
    match policy::select_retained(bad_path) {
        Err(policy::RetainedError::Path) => {},
        _ => { expect(false); },
    }
}

fn retained_error_has_exact_parts(error: policy::RetainedError, head: &str, suffix: &str)
    requires head@ == policy::retained_error_head(error),
        suffix@ == policy::retained_error_suffix(error),
{
    let parts = error.parts();
    let prefix = parts[0] == "E_INPUT_BLOB: ";
    let body = parts[1] == head;
    let field = parts[2] == suffix;
    expect(prefix && body && field);
}

fn retained_error_preserves_any_field_name(name: &str) {
    retained_error_has_exact_parts(policy::RetainedError::Fields(policy::ObjectFieldError::Missing(name)),
        "missing field ", name);
    retained_error_has_exact_parts(policy::RetainedError::Fields(policy::ObjectFieldError::Unknown(name)),
        "unknown field ", name);
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn retained_errors_keep_the_prefix_category_and_full_original_field_name() {
    retained_error_preserves_any_field_name("");
    retained_error_preserves_any_field_name("字段\0原文\nfield");
    retained_error_has_exact_parts(policy::RetainedError::Fields(policy::ObjectFieldError::ExpectedObject),
        "expected object", "");
    retained_error_has_exact_parts(policy::RetainedError::Representation,
        "expected exactly one bytes or blob representation", "");
    retained_error_has_exact_parts(policy::RetainedError::Absent, "absent must be true", "");
    retained_error_has_exact_parts(policy::RetainedError::Bytes, "invalid retained byte array", "");
    retained_error_has_exact_parts(policy::RetainedError::Path, "reference must be within host state", "");
    retained_error_has_exact_parts(policy::RetainedError::Digest, "invalid digest", "");
    retained_error_has_exact_parts(policy::RetainedError::Length, "invalid length", "");
}

}

#[cfg(not(verus_keep_ghost))]
#[test]
fn retained_json_preserves_lexical_paths_and_rejects_invalid_digests() {
    use chrono_judge_registration::inputs::{Retained, retained_shape};
    use serde_json::json;

    let prefix = ".chrono-harness/state/";
    let digest = "0123456789abcdef".repeat(4);
    for suffix in [
        "x", "../x", "a/../x", "x/..", "", "x\n", "x\r", "x\0", "x\\y", "./x", "/x/", "x..", "é",
    ] {
        let path = format!("{prefix}{suffix}");
        let record = json!({"blob": path, "sha256": digest, "length": u64::MAX});
        let result = retained_shape(&record);
        match suffix {
            "x" | "./x" | "/x/" | "x.." | "é" => {
                let Ok(Retained::Blob {
                    path: seen,
                    digest: hash,
                    length,
                }) = result
                else {
                    panic!("valid blob rejected: {suffix:?}");
                };
                assert_eq!(
                    (seen, hash, length),
                    (path.as_str(), digest.as_str(), u64::MAX)
                );
            }
            _ => assert_eq!(
                result.err().as_deref(),
                Some("E_INPUT_BLOB: reference must be within host state"),
                "{suffix:?}"
            ),
        }
    }
    for invalid in [
        digest.to_uppercase(),
        digest[1..].to_owned(),
        format!("{digest}0"),
        "g".repeat(64),
    ] {
        let record = json!({"blob": ".chrono-harness/state/x", "sha256": invalid, "length": false});
        assert_eq!(
            retained_shape(&record).err().as_deref(),
            Some("E_INPUT_BLOB: invalid digest")
        );
    }
}

#[cfg(not(verus_keep_ghost))]
#[test]
fn canonical_json_and_file_identity_share_the_published_digest_format() {
    use chrono_judge_registration::{inputs::observe_file, wire};
    use std::io::Write;

    let expected = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(b"abc").unwrap();
    assert_eq!(
        observe_file(file.path()).unwrap(),
        Some((expected.to_owned(), 3))
    );
    assert_eq!(wire::sha256(b"abc"), expected);
    assert!(wire::is_digest(expected));
    assert!(!wire::is_digest(&expected.to_uppercase()));
    let value: serde_json::Value = serde_json::from_str(r#"{"z":2,"a":1}"#).unwrap();
    assert_eq!(wire::canonical(&value).unwrap(), br#"{"a":1,"z":2}"#);
}

#[cfg(not(verus_keep_ghost))]
#[test]
fn effective_projection_preserves_the_full_retained_blob_length() {
    use chrono_judge_registration::{inputs, wire};
    use chrono_registration_fixture::{CONFIG, load, values};
    use serde_json::json;

    let digest = "a".repeat(64);
    let mut documents = values();
    documents.get_mut(CONFIG).unwrap()["environment"] = json!({
        "inherit": [], "values": {},
        "inputs": [{"id": "data", "location": "input.bin", "sha256": digest}]
    });
    let registrations = load(&documents);
    for length in [0, u64::MAX] {
        let retained =
            json!({"blob": ".chrono-harness/state/input", "sha256": digest, "length": length});
        let Ok(inputs::Retained::Blob {
            digest,
            length: decoded,
            ..
        }) = inputs::retained_shape(&retained)
        else {
            panic!("expected a retained blob of length {length}");
        };
        let fact = json!({"location": "input.bin", "sha256": digest, "length": decoded});
        let endpoint = |commit| {
            json!({
                "commit": commit, "environment": {}, "inherited": {}, "files": {"data": fact}
            })
        };
        let endpoints = json!({
            "base": endpoint("1".repeat(40)), "candidate": endpoint("2".repeat(40))
        });
        let evidence = json!({
            "schema": "chrono-effective-inputs/v1", "completeness_proven": false,
            "endpoints": endpoints, "identity": wire::digest(&endpoints).unwrap()
        });
        let projected =
            inputs::project_effective(&evidence, &registrations, &registrations, None).unwrap();
        for name in ["base", "candidate"] {
            assert_eq!(
                projected["endpoints"][name]["files"]["data"]["length"].as_u64(),
                Some(length)
            );
        }
        assert_eq!(projected["identity"], evidence["identity"]);
    }
}
