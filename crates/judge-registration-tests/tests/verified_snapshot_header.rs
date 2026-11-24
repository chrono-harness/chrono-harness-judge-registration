//! Check the public snapshot contract at its registration owner.
#![allow(unexpected_cfgs)]
#[cfg(not(verus_keep_ghost))]
use chrono_judge_registration::inputs as policy;
#[cfg(verus_keep_ghost)]
#[path = "../../judge-registration/src/verified_snapshot_header.rs"]
mod policy;
#[cfg(verus_keep_ghost)]
use vstd::vpanic as panic;
use vstd::{contrib::exec_spec::ExecSpecEq, prelude::*};

verus! {

fn expect(value: bool)
    requires value,
{ if !value { panic!(); } }

#[cfg_attr(not(verus_keep_ghost), test)]
fn snapshot_selector_dispatch_requires_the_complete_v3_schema() {
    proof {
        reveal_strlit("chrono-input-snapshot/v1"); reveal_strlit("chrono-input-snapshot/v2");
        reveal_strlit("chrono-input-snapshot/v3");
        assert("chrono-input-snapshot/v1"@[23] != "chrono-input-snapshot/v3"@[23]);
        assert("chrono-input-snapshot/v2"@[23] != "chrono-input-snapshot/v3"@[23]);
    }
    expect(policy::snapshot_uses_selector(Some("chrono-input-snapshot/v3")));
    expect(!policy::snapshot_uses_selector(Some("chrono-input-snapshot/v1")));
    expect(!policy::snapshot_uses_selector(Some("chrono-input-snapshot/v2")));
    expect(!policy::snapshot_uses_selector(None));
}

fn non_v3_snapshot_does_not_select_contents(schema: &str)
    requires schema@ != "chrono-input-snapshot/v3"@,
{
    expect(!policy::snapshot_uses_selector(Some(schema)));
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn snapshot_selector_dispatch_preserves_unknown_text_boundaries() {
    proof {
        reveal_strlit(""); reveal_strlit("chrono-input-snapshot/v3");
        reveal_strlit("chrono-input-snapshot/v3\0"); reveal_strlit("chrono-input-snapshot/v3λ");
        reveal_strlit("chrono-input-snapshot/v3\n"); reveal_strlit("chrono-input-snapshot/v4");
        assert(""@.len() != "chrono-input-snapshot/v3"@.len());
        assert("chrono-input-snapshot/v3\0"@.len() != "chrono-input-snapshot/v3"@.len());
        assert("chrono-input-snapshot/v3λ"@.len() != "chrono-input-snapshot/v3"@.len());
        assert("chrono-input-snapshot/v3\n"@.len() != "chrono-input-snapshot/v3"@.len());
        assert("chrono-input-snapshot/v4"@[23] != "chrono-input-snapshot/v3"@[23]);
    }
    non_v3_snapshot_does_not_select_contents("");
    non_v3_snapshot_does_not_select_contents("chrono-input-snapshot/v3\0");
    non_v3_snapshot_does_not_select_contents("chrono-input-snapshot/v3λ");
    non_v3_snapshot_does_not_select_contents("chrono-input-snapshot/v3\n");
    non_v3_snapshot_does_not_select_contents("chrono-input-snapshot/v4");
}

fn selector_binding_error(result: Result<(), &str>)
    requires match result { Err(message) => message@ == "E_INPUT_SNAPSHOT: invalid selector binding schema"@, _ => false },
{
    expect_snapshot_error(result, "E_INPUT_SNAPSHOT: invalid selector binding schema");
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn snapshot_selector_binding_accepts_literal_platform_and_exact_links() {
    proof {
        reveal_strlit("linux-x86_64"); reveal_strlit("平台 λ");
        assert("linux-x86_64"@.len() > 0);
        assert("平台 λ"@.len() > 0);
    }
    let ascii = policy::admit_snapshot_selector_binding(
        Some("chrono-registry-selection/v1"), Some(("linux-x86_64", "linux-x86_64")), true, true, false);
    let unicode = policy::admit_snapshot_selector_binding(
        Some("chrono-registry-selection/v1"), Some(("平台 λ", "平台 λ")), true, true, false);
    expect(ascii.is_ok() && unicode.is_ok());
}

fn arbitrary_literal_platform_is_preserved(platform: &str)
    requires platform@.len() > 0,
{
    let actual = policy::admit_snapshot_selector_binding(
        Some("chrono-registry-selection/v1"), Some((platform, platform)), true, true, false);
    expect(actual.is_ok());
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn snapshot_selector_binding_accepts_internal_spaces_and_nul_without_normalizing() {
    proof { reveal_strlit("a b"); reveal_strlit("\0"); }
    arbitrary_literal_platform_is_preserved("a b");
    arbitrary_literal_platform_is_preserved("\0");
}

fn unknown_binding_schema_fails(schema: Option<&str>)
    requires match schema { None => true, Some(text) => text@ != "chrono-registry-selection/v1"@ },
{
    selector_binding_error(policy::admit_snapshot_selector_binding(
        schema, Some(("linux", "linux")), true, true, false));
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn snapshot_selector_binding_rejects_missing_and_unknown_schema() {
    proof {
        reveal_strlit(""); reveal_strlit("chrono-registry-selection/v1");
        reveal_strlit("chrono-registry-selection/v1\0");
        assert(""@.len() == 0);
        assert("chrono-registry-selection/v1\0"@.len() != "chrono-registry-selection/v1"@.len());
    }
    unknown_binding_schema_fails(None);
    unknown_binding_schema_fails(Some(""));
    unknown_binding_schema_fails(Some("chrono-registry-selection/v1\0"));
}

fn nonliteral_platform_fails(platform: &str, trimmed: &str)
    requires platform@.len() == 0 || platform@ != trimmed@,
{
    selector_binding_error(policy::admit_snapshot_selector_binding(
        Some("chrono-registry-selection/v1"), Some((platform, trimmed)), true, true, false));
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn snapshot_selector_binding_rejects_missing_empty_and_trimmed_platform() {
    proof {
        reveal_strlit(""); reveal_strlit(" linux"); reveal_strlit("linux");
        reveal_strlit("linux\n"); reveal_strlit("\u{2003}linux");
        assert(" linux"@.len() != "linux"@.len());
        assert("linux\n"@.len() != "linux"@.len());
        assert("\u{2003}linux"@.len() != "linux"@.len());
    }
    selector_binding_error(policy::admit_snapshot_selector_binding(
        Some("chrono-registry-selection/v1"), None, true, true, false));
    nonliteral_platform_fails("", "");
    nonliteral_platform_fails(" linux", "linux");
    nonliteral_platform_fails("linux\n", "linux");
    nonliteral_platform_fails("\u{2003}linux", "linux");
}

fn incorrect_binding_links_fail(entry: bool, selected: bool, self_link: bool)
    requires !entry || !selected || self_link,
{
    selector_binding_error(policy::admit_snapshot_selector_binding(
        Some("chrono-registry-selection/v1"), Some(("linux", "linux")), entry, selected, self_link));
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn snapshot_selector_binding_rejects_all_seven_incorrect_link_combinations() {
    incorrect_binding_links_fail(false, false, false);
    incorrect_binding_links_fail(false, false, true);
    incorrect_binding_links_fail(false, true, false);
    incorrect_binding_links_fail(false, true, true);
    incorrect_binding_links_fail(true, false, false);
    incorrect_binding_links_fail(true, false, true);
    incorrect_binding_links_fail(true, true, true);
}

fn snapshot_schema_literals_differ()
    ensures
        "chrono-input-snapshot/v1"@ != "chrono-input-snapshot/v2"@,
        "chrono-input-snapshot/v1"@ != "chrono-input-snapshot/v3"@,
        "chrono-input-snapshot/v2"@ != "chrono-input-snapshot/v3"@,
{
    proof {
        reveal_strlit("chrono-input-snapshot/v1");
        reveal_strlit("chrono-input-snapshot/v2");
        reveal_strlit("chrono-input-snapshot/v3");
        assert("chrono-input-snapshot/v1"@[23] != "chrono-input-snapshot/v2"@[23]);
        assert("chrono-input-snapshot/v1"@[23] != "chrono-input-snapshot/v3"@[23]);
        assert("chrono-input-snapshot/v2"@[23] != "chrono-input-snapshot/v3"@[23]);
    }
}

fn expect_snapshot_error(result: Result<(), &str>, expected: &str)
    requires match result { Err(message) => message@ == expected@, _ => false },
{
    match result {
        Err(message) => {
            let exact = <&str>::exec_eq(message, expected);
            expect(exact);
        },
        _ => { expect(false); },
    }
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn snapshot_header_accepts_only_the_three_complete_schemas() {
    snapshot_schema_literals_differ();
    let one = policy::admit_snapshot_header(Some("chrono-input-snapshot/v1"), false, false);
    let two = policy::admit_snapshot_header(Some("chrono-input-snapshot/v2"), false, false);
    let three = policy::admit_snapshot_header(Some("chrono-input-snapshot/v3"), false, false);
    expect(one.is_ok() && two.is_ok() && three.is_ok());
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn snapshot_v3_defers_all_selector_presence_cases_to_its_binding_validator() {
    snapshot_schema_literals_differ();
    let absent = policy::admit_snapshot_header(Some("chrono-input-snapshot/v3"), false, false);
    let effective = policy::admit_snapshot_header(Some("chrono-input-snapshot/v3"), true, false);
    let selection = policy::admit_snapshot_header(Some("chrono-input-snapshot/v3"), false, true);
    let both = policy::admit_snapshot_header(Some("chrono-input-snapshot/v3"), true, true);
    expect(absent.is_ok() && effective.is_ok() && selection.is_ok() && both.is_ok());
}

fn legacy_snapshot_rejects_either_selector_field(schema: &str)
    requires schema@ == "chrono-input-snapshot/v1"@ || schema@ == "chrono-input-snapshot/v2"@,
{
    snapshot_schema_literals_differ();
    let effective = policy::admit_snapshot_header(Some(schema), true, false);
    let selection = policy::admit_snapshot_header(Some(schema), false, true);
    let both = policy::admit_snapshot_header(Some(schema), true, true);
    expect_snapshot_error(effective, "E_INPUT_SNAPSHOT: selection binding requires snapshot v3");
    expect_snapshot_error(selection, "E_INPUT_SNAPSHOT: selection binding requires snapshot v3");
    expect_snapshot_error(both, "E_INPUT_SNAPSHOT: selection binding requires snapshot v3");
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn snapshot_v1_and_v2_reject_present_selector_fields() {
    legacy_snapshot_rejects_either_selector_field("chrono-input-snapshot/v1");
    legacy_snapshot_rejects_either_selector_field("chrono-input-snapshot/v2");
}

fn unknown_snapshot_schema_keeps_priority(schema: Option<&str>, effective: bool, selection: bool)
    requires match schema {
        None => true,
        Some(value) => value@ != "chrono-input-snapshot/v1"@
            && value@ != "chrono-input-snapshot/v2"@ && value@ != "chrono-input-snapshot/v3"@,
    },
{
    let actual = policy::admit_snapshot_header(schema, effective, selection);
    expect_snapshot_error(actual, "E_INPUT_SNAPSHOT: unsupported snapshot schema");
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn snapshot_unknown_and_missing_schemas_fail_before_selector_fields() {
    proof {
        reveal_strlit(""); reveal_strlit("chrono-input-snapshot/v0");
        reveal_strlit("chrono-input-snapshot/v4"); reveal_strlit("chrono-input-snapshot/v3\0");
        reveal_strlit("Chrono-input-snapshot/v3"); reveal_strlit("chrono-input-snapshot/v3é");
        reveal_strlit("chrono-input-snapshot/v1"); reveal_strlit("chrono-input-snapshot/v2");
        reveal_strlit("chrono-input-snapshot/v3");
        assert(""@.len() == 0);
        assert("chrono-input-snapshot/v1"@.len() == 24);
        assert("chrono-input-snapshot/v2"@.len() == 24);
        assert("chrono-input-snapshot/v3"@.len() == 24);
        assert("chrono-input-snapshot/v0"@[23] != "chrono-input-snapshot/v1"@[23]);
        assert("chrono-input-snapshot/v0"@[23] != "chrono-input-snapshot/v2"@[23]);
        assert("chrono-input-snapshot/v0"@[23] != "chrono-input-snapshot/v3"@[23]);
        assert("chrono-input-snapshot/v4"@[23] != "chrono-input-snapshot/v1"@[23]);
        assert("chrono-input-snapshot/v4"@[23] != "chrono-input-snapshot/v2"@[23]);
        assert("chrono-input-snapshot/v4"@[23] != "chrono-input-snapshot/v3"@[23]);
        assert("chrono-input-snapshot/v3\0"@.len() == 25);
        assert("Chrono-input-snapshot/v3"@[0] != "chrono-input-snapshot/v1"@[0]);
        assert("Chrono-input-snapshot/v3"@[0] != "chrono-input-snapshot/v2"@[0]);
        assert("Chrono-input-snapshot/v3"@[0] != "chrono-input-snapshot/v3"@[0]);
        assert("chrono-input-snapshot/v3é"@.len() == 25);
    }
    unknown_snapshot_schema_keeps_priority(None, false, false);
    unknown_snapshot_schema_keeps_priority(None, true, true);
    unknown_snapshot_schema_keeps_priority(Some(""), true, true);
    unknown_snapshot_schema_keeps_priority(Some("chrono-input-snapshot/v0"), true, false);
    unknown_snapshot_schema_keeps_priority(Some("chrono-input-snapshot/v4"), false, true);
    unknown_snapshot_schema_keeps_priority(Some("chrono-input-snapshot/v3\0"), true, true);
    unknown_snapshot_schema_keeps_priority(Some("Chrono-input-snapshot/v3"), true, true);
    unknown_snapshot_schema_keeps_priority(Some("chrono-input-snapshot/v3é"), true, true);
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn absent_retained_input_requires_snapshot_v2_or_v3() {
    snapshot_schema_literals_differ();
    let absent: policy::Retained<Vec<u8>> = policy::Retained::Absent;
    let one = policy::admit_snapshot_retained(Some("chrono-input-snapshot/v1"), &absent);
    let missing = policy::admit_snapshot_retained(None, &absent);
    let two = policy::admit_snapshot_retained(Some("chrono-input-snapshot/v2"), &absent);
    let three = policy::admit_snapshot_retained(Some("chrono-input-snapshot/v3"), &absent);
    expect_snapshot_error(one, "E_INPUT_SNAPSHOT: absence requires snapshot v2");
    expect_snapshot_error(missing, "E_INPUT_SNAPSHOT: absence requires snapshot v2");
    expect(two.is_ok() && three.is_ok());
}

fn present_retained_input_has_no_additional_schema_restriction(schema: Option<&str>, length: u64) {
    let bytes = policy::Retained::Bytes(vec![0u8, 255u8]);
    let blob: policy::Retained<Vec<u8>> = policy::Retained::Blob {
        path: ".chrono-harness/state/blob", digest: "original digest", length,
    };
    let bytes_ok = policy::admit_snapshot_retained(schema, &bytes);
    let blob_ok = policy::admit_snapshot_retained(schema, &blob);
    expect(bytes_ok.is_ok() && blob_ok.is_ok());
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn bytes_and_blob_keep_the_callers_schema_and_content_obligations() {
    present_retained_input_has_no_additional_schema_restriction(None, 0);
    present_retained_input_has_no_additional_schema_restriction(Some("chrono-input-snapshot/v1"), 1);
    present_retained_input_has_no_additional_schema_restriction(Some("chrono-input-snapshot/v2"), u64::MAX);
    present_retained_input_has_no_additional_schema_restriction(Some("chrono-input-snapshot/v3"), 0);
    present_retained_input_has_no_additional_schema_restriction(Some("unknown\0é"), u64::MAX);
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn direct_snapshot_versions_two_three_and_four_select_v2() {
    let two = policy::select_snapshot_schema(Some(2), false);
    let three = policy::select_snapshot_schema(Some(3), false);
    let four = policy::select_snapshot_schema(Some(4), false);
    let two_ok = <&str>::exec_eq(two, "chrono-input-snapshot/v2");
    let three_ok = <&str>::exec_eq(three, "chrono-input-snapshot/v2");
    let four_ok = <&str>::exec_eq(four, "chrono-input-snapshot/v2");
    expect(two_ok && three_ok && four_ok);
}

fn direct_unknown_snapshot_version_selects_v1(version: u64)
    requires version != 2, version != 3, version != 4,
{
    let actual = policy::select_snapshot_schema(Some(version), false);
    let exact = <&str>::exec_eq(actual, "chrono-input-snapshot/v1");
    expect(exact);
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn direct_snapshot_missing_zero_one_five_and_maximum_select_v1() {
    let missing = policy::select_snapshot_schema(None, false);
    let missing_ok = <&str>::exec_eq(missing, "chrono-input-snapshot/v1");
    expect(missing_ok);
    direct_unknown_snapshot_version_selects_v1(0);
    direct_unknown_snapshot_version_selects_v1(1);
    direct_unknown_snapshot_version_selects_v1(5);
    direct_unknown_snapshot_version_selects_v1(u64::MAX);
}

fn selected_snapshot_ignores_any_direct_version(version: Option<u64>) {
    let actual = policy::select_snapshot_schema(version, true);
    let exact = <&str>::exec_eq(actual, "chrono-input-snapshot/v3");
    expect(exact);
}

#[cfg_attr(not(verus_keep_ghost), test)]
fn selected_snapshot_uses_v3_for_missing_known_and_boundary_versions() {
    selected_snapshot_ignores_any_direct_version(None);
    selected_snapshot_ignores_any_direct_version(Some(0));
    selected_snapshot_ignores_any_direct_version(Some(1));
    selected_snapshot_ignores_any_direct_version(Some(2));
    selected_snapshot_ignores_any_direct_version(Some(3));
    selected_snapshot_ignores_any_direct_version(Some(4));
    selected_snapshot_ignores_any_direct_version(Some(5));
    selected_snapshot_ignores_any_direct_version(Some(u64::MAX));
}

}

#[cfg(not(verus_keep_ghost))]
#[test]
fn decoded_snapshot_applies_schema_and_selector_field_rules() {
    use chrono_judge_registration::inputs::snapshot_shape;
    use serde_json::json;

    let mut snapshot = json!({
        "schema": "chrono-input-snapshot/v2",
        "commit": "1".repeat(40),
        "config_path": "config.json",
        "config_digest": "a".repeat(64),
        "environment": {},
        "files": {}
    });
    assert_eq!(snapshot_shape(&snapshot), Ok(()));

    snapshot["effective_config_path"] = json!(null);
    assert_eq!(
        snapshot_shape(&snapshot).unwrap_err(),
        "E_INPUT_SNAPSHOT: selection binding requires snapshot v3"
    );
    snapshot["schema"] = json!("chrono-input-snapshot/v4");
    assert_eq!(
        snapshot_shape(&snapshot).unwrap_err(),
        "E_INPUT_SNAPSHOT: unsupported snapshot schema"
    );
}
