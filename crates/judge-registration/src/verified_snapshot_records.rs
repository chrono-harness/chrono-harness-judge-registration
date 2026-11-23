//! Validate retained absent, inline-byte, and file-backed input representations.
use super::{header::Retained, schema};
use schema::{ObjectFieldError, object_error_parts, validate_object_fields};
#[cfg(verus_keep_ghost)]
use vstd::string::StringSliceAdditionalSpecFns;
use vstd::{contrib::exec_spec::ExecSpecEq, prelude::*};

verus! {

/// A suffix byte is legal unless it breaks the wire path syntax or starts a parent component.
pub open spec fn blob_byte(path: Seq<u8>, i: int) -> bool {
    let b = path[i];
    b != 0 && b != 10 && b != 13 && b != 92
        && !(b == 46 && path[i-1] == 47 && i+1 < path.len() && path[i+1] == 46
            && (i+2 == path.len() || path[i+2] == 47))
}

pub open spec fn blob_path(path: Seq<u8>) -> bool {
    let prefix = ".chrono-harness/state/".spec_bytes();
    &&& path.len() > prefix.len()
    &&& path.take(prefix.len() as int) == prefix
    &&& forall|i: int| prefix.len() <= i < path.len() ==> #[trigger] blob_byte(path, i)
}

fn valid_blob_path(bytes: &[u8]) -> (valid: bool)
    ensures valid == blob_path(bytes@),
{
    let prefix = ".chrono-harness/state/".as_bytes();
    if bytes.len() <= prefix.len() { return false; }
    let (head, _) = bytes.split_at(prefix.len());
    proof { assert(head.deep_view() =~= head@); assert(prefix.deep_view() =~= prefix@); }
    if !<&[u8]>::exec_eq(head, prefix) { return false; }
    let mut i = prefix.len();
    proof { reveal_strlit(".chrono-harness/state/"); }
    while i < bytes.len()
        invariant
            prefix@ == ".chrono-harness/state/".spec_bytes(),
            bytes@.take(prefix.len() as int) == prefix@,
            0 < prefix.len() < bytes.len(), prefix.len() <= i <= bytes.len(),
            forall|j: int| prefix.len() <= j < i ==> #[trigger] blob_byte(bytes@, j),
        decreases bytes.len() - i,
    {
        let b = bytes[i];
        let parent = bytes[i-1] == 47 && b == 46 && bytes.len()-i >= 2
            && bytes[i+1] == 46 && (bytes.len()-i == 2 || bytes[i+2] == 47);
        if b == 0 || b == 10 || b == 13 || b == 92 || parent {
            assert(!blob_byte(bytes@, i as int));
            return false;
        }
        assert(blob_byte(bytes@, i as int));
        i += 1;
    }
    true
}

pub open spec fn blob_digest(bytes: Seq<u8>) -> bool {
    bytes.len() == 64 && forall|i: int| 0 <= i < bytes.len() ==>
        (48 <= #[trigger] bytes[i] <= 57 || 97 <= bytes[i] <= 102)
}

fn valid_blob_digest(bytes: &[u8]) -> (valid: bool)
    ensures valid == blob_digest(bytes@),
{
    if bytes.len() != 64 { return false; }
    for i in 0..64usize
        invariant bytes.len() == 64, forall|j: int| 0 <= j < i ==>
            (48 <= #[trigger] bytes@[j] <= 57 || 97 <= bytes@[j] <= 102),
    {
        let digit = bytes[i];
        if !(48 <= digit && digit <= 57 || 97 <= digit && digit <= 102) { return false; }
    }
    true
}

/// 同一 JSON 的原观察；缺键与非对应类型分别保留。
pub struct RetainedObservation<'a, B> {
    pub object: bool,
    pub keys: Vec<&'a str>,
    pub absent: Option<Option<bool>>,
    pub bytes: Option<B>,
    pub blob: Option<Option<&'a str>>,
    pub digest: Option<&'a str>,
    pub length: Option<u64>,
}

pub enum RetainedError<'a> {
    Fields(ObjectFieldError<'a>),
    Representation,
    Absent,
    Bytes,
    Path,
    Digest,
    Length,
}

#[verifier::inline]
pub open spec fn retained_mode<B>(observed: RetainedObservation<B>) -> int {
    if observed.absent.is_some() { 0 }
    else if observed.bytes.is_some() && observed.blob.is_none() { 1 }
    else if observed.bytes.is_none() && observed.blob.is_some() { 2 }
    else { 3 }
}

#[verifier::inline]
pub open spec fn retained_required(mode: int) -> Seq<&'static str> {
    if mode == 0 { seq!["absent"] }
    else if mode == 1 { seq!["bytes"] }
    else { seq!["blob", "sha256", "length"] }
}

#[verifier::inline]
pub open spec fn retained_fields_ok<B>(observed: RetainedObservation<B>) -> bool {
    observed.object
        && schema::required_fields_present(retained_required(retained_mode(observed)), observed.keys@)
        && schema::known_fields_only(retained_required(retained_mode(observed)), seq![], observed.keys@)
}

#[verifier::inline]
pub open spec fn retained_field_failure<B>(
    observed: RetainedObservation<B>, error: ObjectFieldError,
) -> bool {
    match error {
        ObjectFieldError::ExpectedObject => !observed.object,
        ObjectFieldError::Missing(name) => observed.object && schema::first_missing_field(
            retained_required(retained_mode(observed)), observed.keys@, name@,
        ),
        ObjectFieldError::Unknown(name) => observed.object
            && schema::required_fields_present(retained_required(retained_mode(observed)), observed.keys@)
            && schema::first_unknown_field(retained_required(retained_mode(observed)), seq![], observed.keys@, name@),
    }
}

pub open spec fn retained_path_valid(path: Option<Option<&str>>) -> bool {
    match path {
        Some(Some(path)) => blob_path(path.spec_bytes()),
        _ => false,
    }
}

pub open spec fn retained_digest_valid(digest: Option<&str>) -> bool {
    match digest {
        Some(digest) => blob_digest(digest.spec_bytes()),
        None => false,
    }
}

pub open spec fn retained_selection<B>(
    observed: RetainedObservation<B>, result: Result<Retained<B>, RetainedError>,
) -> bool {
    match result {
        Ok(Retained::Absent) => retained_mode(observed) == 0 && retained_fields_ok(observed)
            && observed.absent == Some(Some(true)),
        Ok(Retained::Bytes(bytes)) => retained_mode(observed) == 1 && retained_fields_ok(observed)
            && observed.bytes == Some(bytes),
        Ok(Retained::Blob { path, digest, length }) => retained_mode(observed) == 2
            && retained_fields_ok(observed) && retained_path_valid(observed.blob)
            && retained_digest_valid(observed.digest)
            && observed.blob == Some(Some(path)) && observed.digest == Some(digest)
            && observed.length == Some(length),
        Err(RetainedError::Representation) => retained_mode(observed) == 3,
        Err(RetainedError::Fields(error)) => retained_mode(observed) != 3
            && retained_field_failure(observed, error),
        Err(RetainedError::Absent) => retained_mode(observed) == 0 && retained_fields_ok(observed)
            && observed.absent != Some(Some(true)),
        Err(RetainedError::Path) => retained_mode(observed) == 2 && retained_fields_ok(observed)
            && !retained_path_valid(observed.blob),
        Err(RetainedError::Digest) => retained_mode(observed) == 2 && retained_fields_ok(observed)
            && retained_path_valid(observed.blob) && !retained_digest_valid(observed.digest),
        Err(RetainedError::Length) => retained_mode(observed) == 2 && retained_fields_ok(observed)
            && retained_path_valid(observed.blob) && retained_digest_valid(observed.digest)
            && observed.length.is_none(),
        Err(RetainedError::Bytes) => false,
    }
}

/// absent 键优先；先结构，再逐项检查；字节解码另由下方同源驱动调用。
pub fn select_retained<'a, B>(observed: RetainedObservation<'a, B>)
    -> (out: Result<Retained<'a, B>, RetainedError<'a>>)
    ensures retained_selection(observed, out),
{
    let object = if observed.object { Some(()) } else { None };
    if observed.absent.is_some() {
        let required = ["absent"];
        proof {
            assert(retained_mode(observed) == 0);
            assert(required@ == retained_required(retained_mode(observed)));
        }
        match validate_object_fields(object, &required, &[], &observed.keys) {
            Err(error) => {
                proof { assert(retained_field_failure(observed, error)); }
                return Err(RetainedError::Fields(error));
            },
            Ok(_) => {},
        }
        return match observed.absent {
            Some(Some(true)) => Ok(Retained::Absent),
            _ => Err(RetainedError::Absent),
        };
    }
    match (observed.bytes, observed.blob) {
        (Some(bytes), None) => {
            let required = ["bytes"];
            proof {
                assert(retained_mode(observed) == 1);
                assert(required@ == retained_required(retained_mode(observed)));
            }
            match validate_object_fields(object, &required, &[], &observed.keys) {
                Err(error) => {
                    proof { assert(retained_field_failure(observed, error)); }
                    return Err(RetainedError::Fields(error));
                },
                Ok(_) => {},
            }
            Ok(Retained::Bytes(bytes))
        },
        (None, Some(blob)) => {
            let required = ["blob", "sha256", "length"];
            proof {
                assert(retained_mode(observed) == 2);
                assert(required@ == retained_required(retained_mode(observed)));
            }
            match validate_object_fields(object, &required, &[], &observed.keys) {
                Err(error) => {
                    proof { assert(retained_field_failure(observed, error)); }
                    return Err(RetainedError::Fields(error));
                },
                Ok(_) => {},
            }
            let path = match blob {
                Some(path) => {
                    if !valid_blob_path(path.as_bytes()) { return Err(RetainedError::Path); }
                    path
                },
                None => return Err(RetainedError::Path),
            };
            let digest = match observed.digest {
                Some(digest) => {
                    if !valid_blob_digest(digest.as_bytes()) { return Err(RetainedError::Digest); }
                    digest
                },
                None => return Err(RetainedError::Digest),
            };
            let length = match observed.length {
                Some(length) => length,
                None => return Err(RetainedError::Length),
            };
            Ok(Retained::Blob { path, digest, length })
        },
        _ => Err(RetainedError::Representation),
    }
}

pub open spec fn retained_decoding<S, B, D>(
    state: S, selected: Result<Retained<B>, RetainedError>, decode: D,
    out: (S, Result<Retained, RetainedError>),
) -> bool
    where D: FnOnce(S, B) -> (S, Result<Vec<u8>, ()>),
{
    match selected {
        Err(error) => out.0 == state && out.1 == Err::<Retained, RetainedError>(error),
        Ok(Retained::Absent) => out.0 == state && out.1 == Ok::<Retained, RetainedError>(Retained::Absent),
        Ok(Retained::Blob { path, digest, length }) =>
            out.0 == state && out.1 == Ok::<Retained, RetainedError>(Retained::Blob { path, digest, length }),
        Ok(Retained::Bytes(bytes)) => exists|decoded: (S, Result<Vec<u8>, ()>)|
            decode.ensures((state, bytes), decoded) && out.0 == decoded.0
            && match decoded.1 {
                Ok(value) => out.1 == Ok::<Retained, RetainedError>(Retained::Bytes(value)),
                Err(()) => out.1 == Err::<Retained, RetainedError>(RetainedError::Bytes),
            },
    }
}

/// 仅合格 bytes 调用一次原解码器；其它情况保留初态且零调用。
pub fn parse_retained<'a, S, B, D>(state: S, observed: RetainedObservation<'a, B>, decode: D)
    -> (out: (S, Result<Retained<'a>, RetainedError<'a>>))
    where D: FnOnce(S, B) -> (S, Result<Vec<u8>, ()>),
    requires forall|state: S, bytes: B| decode.requires((state, bytes)),
    ensures exists|selected: Result<Retained<'a, B>, RetainedError<'a>>|
        retained_selection(observed, selected) && retained_decoding(state, selected, decode, out),
{
    let ghost initial = state;
    let selected = select_retained(observed);
    let ghost selection = selected;
    let out = match selected {
        Err(error) => (state, Err(error)),
        Ok(Retained::Absent) => (state, Ok(Retained::Absent)),
        Ok(Retained::Blob { path, digest, length }) =>
            (state, Ok(Retained::Blob { path, digest, length })),
        Ok(Retained::Bytes(bytes)) => {
            let ghost input = bytes;
            let decoded = decode(state, bytes);
            let ghost original = decoded;
            let (state, result) = decoded;
            let out = match result {
                Ok(bytes) => (state, Ok(Retained::Bytes(bytes))),
                Err(()) => (state, Err(RetainedError::Bytes)),
            };
            proof {
                assert(decode.ensures((initial, input), original));
                assert(retained_decoding(initial, selection, decode, out));
            }
            out
        },
    };
    proof {
        assert(retained_selection(observed, selection));
        assert(retained_decoding(initial, selection, decode, out));
        assert(exists|selected: Result<Retained<'a, B>, RetainedError<'a>>|
            retained_selection(observed, selected) && retained_decoding(initial, selected, decode, out));
    }
    out
}

pub open spec fn retained_error_head(error: RetainedError) -> Seq<char> {
    match error {
        RetainedError::Fields(ObjectFieldError::ExpectedObject) => "expected object"@,
        RetainedError::Fields(ObjectFieldError::Missing(_)) => "missing field "@,
        RetainedError::Fields(ObjectFieldError::Unknown(_)) => "unknown field "@,
        RetainedError::Representation => "expected exactly one bytes or blob representation"@,
        RetainedError::Absent => "absent must be true"@,
        RetainedError::Bytes => "invalid retained byte array"@,
        RetainedError::Path => "reference must be within host state"@,
        RetainedError::Digest => "invalid digest"@,
        RetainedError::Length => "invalid length"@,
    }
}

pub open spec fn retained_error_suffix(error: RetainedError) -> Seq<char> {
    match error {
        RetainedError::Fields(ObjectFieldError::Missing(name)) => name@,
        RetainedError::Fields(ObjectFieldError::Unknown(name)) => name@,
        _ => ""@,
    }
}

pub open spec fn retained_error_message(error: RetainedError) -> Seq<char> {
    "E_INPUT_BLOB: "@ + (retained_error_head(error) + retained_error_suffix(error))
}

impl<'a> RetainedError<'a> {
    pub fn parts(self) -> (out: [&'a str; 3])
        ensures out[0]@ == "E_INPUT_BLOB: "@,
            out[1]@ == retained_error_head(self),
            out[2]@ == retained_error_suffix(self),
            out[0]@ + (out[1]@ + out[2]@) == retained_error_message(self),
    {
        let tail = match self {
            Self::Fields(error) => object_error_parts(error),
            Self::Representation => ["expected exactly one bytes or blob representation", ""],
            Self::Absent => ["absent must be true", ""],
            Self::Bytes => ["invalid retained byte array", ""],
            Self::Path => ["reference must be within host state", ""],
            Self::Digest => ["invalid digest", ""],
            Self::Length => ["invalid length", ""],
        };
        ["E_INPUT_BLOB: ", tail[0], tail[1]]
    }
}

}
