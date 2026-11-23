//! Admission of an observed object against its registered required and optional fields.
#[cfg(verus_keep_ghost)]
use vstd::assert_seqs_equal;
use vstd::{contrib::exec_spec::ExecSpecEq, prelude::*};

verus! {

pub enum ObjectFieldError<'a> {
    ExpectedObject,
    Missing(&'a str),
    Unknown(&'a str),
}

pub open spec fn field_named(fields: Seq<&str>, name: Seq<char>) -> bool {
    exists|i: int| 0 <= i < fields.len() && #[trigger] fields[i]@ == name
}

pub open spec fn required_fields_present(required: Seq<&str>, keys: Seq<&str>) -> bool {
    forall|i: int| 0 <= i < required.len() ==> field_named(keys, #[trigger] required[i]@)
}

pub open spec fn known_fields_only(required: Seq<&str>, optional: Seq<&str>, keys: Seq<&str>) -> bool {
    forall|i: int| 0 <= i < keys.len() ==> (
        field_named(required, #[trigger] keys[i]@) || field_named(optional, keys[i]@)
    )
}

pub open spec fn first_missing_field(required: Seq<&str>, keys: Seq<&str>, name: Seq<char>) -> bool {
    exists|i: int| 0 <= i < required.len() && required[i]@ == name
        && !field_named(keys, required[i]@)
        && forall|j: int| 0 <= j < i ==> field_named(keys, #[trigger] required[j]@)
}

pub open spec fn first_unknown_field(
    required: Seq<&str>, optional: Seq<&str>, keys: Seq<&str>, name: Seq<char>,
) -> bool {
    exists|i: int| 0 <= i < keys.len() && keys[i]@ == name
        && !field_named(required, keys[i]@) && !field_named(optional, keys[i]@)
        && forall|j: int| 0 <= j < i ==> (
            field_named(required, #[trigger] keys[j]@) || field_named(optional, keys[j]@)
        )
}

fn field_list_contains(fields: &[&str], name: &str) -> (found: bool)
    ensures found == field_named(fields@, name@),
{
    let mut i = 0;
    while i < fields.len()
        invariant 0 <= i <= fields.len(),
            forall|j: int| 0 <= j < i ==> fields[j]@ != name@,
        decreases fields.len() - i,
    {
        if <&str>::exec_eq(fields[i], name) {
            return true;
        }
        i += 1;
    }
    false
}

/// 返回同一个对象；缺字段按 required 顺序优先，未知字段按实际键顺序优先。
pub fn validate_object_fields<'a, T>(
    object: Option<T>, required: &[&'a str], optional: &[&str], keys: &[&'a str],
) -> (out: std::result::Result<T, ObjectFieldError<'a>>)
    ensures out.is_ok() == (object.is_some()
        && required_fields_present(required@, keys@)
        && known_fields_only(required@, optional@, keys@)),
        match out {
            Ok(value) => object == Some(value),
            Err(ObjectFieldError::ExpectedObject) => object.is_none(),
            Err(ObjectFieldError::Missing(name)) => object.is_some()
                && first_missing_field(required@, keys@, name@),
            Err(ObjectFieldError::Unknown(name)) => object.is_some()
                && required_fields_present(required@, keys@)
                && first_unknown_field(required@, optional@, keys@, name@),
        },
{
    let value = match object {
        Some(value) => value,
        None => return Err(ObjectFieldError::ExpectedObject),
    };
    let mut i = 0;
    while i < required.len()
        invariant 0 <= i <= required.len(), object == Some(value),
            forall|j: int| 0 <= j < i ==> field_named(keys@, #[trigger] required[j]@),
        decreases required.len() - i,
    {
        if !field_list_contains(keys, required[i]) {
            proof { assert(first_missing_field(required@, keys@, required[i as int]@)); }
            return Err(ObjectFieldError::Missing(required[i]));
        }
        i += 1;
    }
    let mut j = 0;
    while j < keys.len()
        invariant 0 <= j <= keys.len(), object == Some(value),
            required_fields_present(required@, keys@),
            forall|k: int| 0 <= k < j ==> (
                field_named(required@, #[trigger] keys[k]@) || field_named(optional@, keys[k]@)
            ),
        decreases keys.len() - j,
    {
        if !field_list_contains(required, keys[j]) && !field_list_contains(optional, keys[j]) {
            proof { assert(first_unknown_field(required@, optional@, keys@, keys[j as int]@)); }
            return Err(ObjectFieldError::Unknown(keys[j]));
        }
        j += 1;
    }
    Ok(value)
}

pub open spec fn object_error_message(error: ObjectFieldError) -> Seq<char> {
    match error {
        ObjectFieldError::ExpectedObject => "expected object"@,
        ObjectFieldError::Missing(name) => "missing field "@ + name@,
        ObjectFieldError::Unknown(name) => "unknown field "@ + name@,
    }
}

/// 标准 concat 只连接两片；错误类别、前缀及完整字段名在此产生。
pub fn object_error_parts<'a>(error: ObjectFieldError<'a>) -> (out: [&'a str; 2])
    ensures out[0]@ + out[1]@ == object_error_message(error),
        match error {
            ObjectFieldError::ExpectedObject => out[0]@ == "expected object"@ && out[1]@ == ""@,
            ObjectFieldError::Missing(name) => out[0]@ == "missing field "@ && out[1]@ == name@,
            ObjectFieldError::Unknown(name) => out[0]@ == "unknown field "@ && out[1]@ == name@,
        },
{
    match error {
        ObjectFieldError::ExpectedObject => {
            proof {
                reveal_strlit("");
                assert_seqs_equal!("expected object"@ + ""@, "expected object"@);
            }
            ["expected object", ""]
        },
        ObjectFieldError::Missing(name) => ["missing field ", name],
        ObjectFieldError::Unknown(name) => ["unknown field ", name],
    }
}

}
