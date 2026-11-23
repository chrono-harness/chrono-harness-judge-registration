//! Reference admission over the fields the source owner supplies.
#![allow(unexpected_cfgs)]
#[cfg(not(verus_keep_ghost))]
use chrono_judge_registration::reference_syntax as policy;
#[cfg(verus_keep_ghost)]
#[path = "../../judge-registration/src/verified_reference_syntax.rs"]
mod policy;
use policy::ReferenceDeclaration;
use vstd::prelude::*;
#[cfg(verus_keep_ghost)]
use vstd::vpanic as panic;

verus! {

struct Form<'a> {
    canonical: bool,
    before: &'a str,
    after: &'a str,
    base: u8,
}

impl ReferenceDeclaration for Form<'_> {
    closed spec fn fields_view(&self) -> (bool, Seq<char>, Seq<char>, u8) {
        (self.canonical, self.before@, self.after@, self.base)
    }
    fn fields(&self) -> (out: (bool, &str, &str, u8)) {
        (self.canonical, self.before, self.after, self.base)
    }
}

fn expect(value: bool)
    requires value,
{ if !value { panic!(); } }

#[cfg_attr(not(verus_keep_ghost), test)]
fn canonical_declarations_check_each_prefix_and_explicit_base() {
    proof {
        reveal_strlit("\""); reveal_strlit("/"); reveal_strlit("");
        vstd::utf8::is_ascii_chars_encode_utf8("\""@);
        vstd::utf8::is_ascii_chars_encode_utf8("/"@);
        vstd::utf8::is_ascii_chars_encode_utf8(""@);
        assert(vstd::utf8::encode_utf8("\""@) =~= seq![34u8]);
        assert(vstd::utf8::encode_utf8("/"@) =~= seq![47u8]);
        assert(vstd::utf8::encode_utf8(""@) =~= Seq::<u8>::empty());
    }
    let quoted = Form { canonical: true, before: "\"", after: "\"", base: 0 };
    let root = Form { canonical: true, before: "/", after: "/", base: 2 };
    let wrong_base = Form { canonical: true, before: "/", after: "/", base: 0 };
    let empty = Form { canonical: true, before: "", after: "", base: 0 };
    let unknown = Form { canonical: true, before: "\"", after: "\"", base: 3 };
    let bad_after = Form { canonical: true, before: "\"", after: "/", base: 1 };
    let bad_file = Form { canonical: false, before: "\"", after: "\"", base: 0 };
    expect(policy::validate_reference_syntax(&vec![quoted, root]).is_ok());
    expect(policy::validate_reference_syntax(&vec![wrong_base]).is_err());
    expect(policy::validate_reference_syntax(&vec![empty]).is_err());
    expect(policy::validate_reference_syntax(&vec![unknown]).is_err());
    expect(policy::validate_reference_syntax(&vec![bad_after]).is_err());
    expect(policy::validate_reference_syntax(&vec![bad_file]).is_err());
}
}

// Exercise the public adapter using the real source owner's canonical-path check.
#[cfg(not(verus_keep_ghost))]
#[test]
fn decoded_reference_declarations_use_the_real_source_file_name() {
    use chrono_harness::verified_verifier::sources::continuity::ReferenceSyntax;
    let check = |file: &str, before: &str, after: &str, base: u8, count: usize| {
        let form = serde_json::json!({
            "file":file, "before":before, "after":after, "trailing":"",
            "suffix_before":"", "suffix_after":"", "base":base
        });
        let forms: Vec<ReferenceSyntax> =
            serde_json::from_value(serde_json::json!(vec![form; count])).unwrap();
        chrono_judge_registration::validate_reference_syntax(&forms)
    };
    assert!(check("x", "\"", "\"", 0, 1).is_ok());
    assert!(check("x", "/", "/", 2, 250).is_ok());
    for (file, before, after, base, count) in [
        ("x", "/", "/", 0, 1),
        ("x", "", "", 0, 1),
        ("x", "\"", "\"", 3, 1),
        ("x", "\"", "/", 1, 1),
        ("./x", "\"", "\"", 0, 1),
        ("x", "\"", "\"", 0, 251),
    ] {
        assert_eq!(
            check(file, before, after, base, count).unwrap_err(),
            "E_SOURCE_CONTINUITY: invalid reference syntax declaration"
        );
    }
}
