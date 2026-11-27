//! Snapshot version, selector binding and absence checks used when admitting retained inputs.
use vstd::prelude::*;

verus! {

pub const SNAPSHOT_SCHEMA: &'static str = "chrono-input-snapshot/v1";
pub const SNAPSHOT_SCHEMA_V2: &'static str = "chrono-input-snapshot/v2";
pub const SNAPSHOT_SCHEMA_V3: &'static str = "chrono-input-snapshot/v3";

pub open spec fn snapshot_format_version(schema: Option<&str>) -> int {
    match schema {
        Some(value) => {
            if value@ == "chrono-input-snapshot/v1"@ { 1 }
            else if value@ == "chrono-input-snapshot/v2"@ { 2 }
            else if value@ == "chrono-input-snapshot/v3"@ { 3 }
            else { 0 }
        },
        None => 0,
    }
}

fn classify_snapshot_schema(schema: Option<&str>) -> (version: u8)
    ensures version as int == snapshot_format_version(schema),
{
    match schema {
        Some(value) => {
            if value == SNAPSHOT_SCHEMA { 1 }
            else if value == SNAPSHOT_SCHEMA_V2 { 2 }
            else if value == SNAPSHOT_SCHEMA_V3 { 3 }
            else { 0 }
        },
        None => 0,
    }
}

/// 只有完整 v3 schema 才选择 selector 内容和端点绑定分支。
pub fn snapshot_uses_selector(schema: Option<&str>) -> (selected: bool)
    ensures selected == match schema {
        Some(text) => text@ == "chrono-input-snapshot/v3"@,
        None => false,
    },
{
    match schema {
        Some(text) => text == SNAPSHOT_SCHEMA_V3,
        None => false,
    }
}

/// 先拒绝未知 schema；只有 v3 可以携带 selector 字段。
pub fn admit_snapshot_header(schema: Option<&str>, effective: bool, selection: bool)
    -> (out: Result<(), &'static str>)
    ensures match out {
        Ok(()) => snapshot_format_version(schema) != 0
            && (snapshot_format_version(schema) == 3 || (!effective && !selection)),
        Err(message) => {
            (snapshot_format_version(schema) == 0
                && message@ == "E_INPUT_SNAPSHOT: unsupported snapshot schema"@)
            || (snapshot_format_version(schema) != 0 && snapshot_format_version(schema) != 3
                && (effective || selection)
                && message@ == "E_INPUT_SNAPSHOT: selection binding requires snapshot v3"@)
        },
    },
{
    let version = classify_snapshot_schema(schema);
    if version == 0 { return Err("E_INPUT_SNAPSHOT: unsupported snapshot schema"); }
    if version != 3 && (effective || selection) {
        return Err("E_INPUT_SNAPSHOT: selection binding requires snapshot v3");
    }
    Ok(())
}

pub open spec fn snapshot_selector_binding_valid(
    schema: Option<&str>, platform: Option<(&str, &str)>,
    entry_matches: bool, selected_matches: bool, self_link: bool,
) -> bool {
    (match schema { Some(text) => text@ == "chrono-registry-selection/v1"@, None => false })
        && (match platform {
            Some((text, trimmed)) => text@.len() > 0 && text@ == trimmed@,
            None => false,
        }) && entry_matches && selected_matches && !self_link
}

/// JSON 全值相等与 trim 只提供观察，完整准入和原错误由本函数决定。
pub fn admit_snapshot_selector_binding(
    schema: Option<&str>, platform: Option<(&str, &str)>,
    entry_matches: bool, selected_matches: bool, self_link: bool,
) -> (out: Result<(), &'static str>)
    ensures match out {
        Ok(()) => snapshot_selector_binding_valid(schema, platform, entry_matches, selected_matches, self_link),
        Err(message) => !snapshot_selector_binding_valid(schema, platform, entry_matches, selected_matches, self_link)
            && message@ == "E_INPUT_SNAPSHOT: invalid selector binding schema"@,
    },
{
    let schema_ok = match schema {
        Some(text) => text == "chrono-registry-selection/v1",
        None => false,
    };
    let platform_ok = match platform {
        Some((text, trimmed)) => !text.is_empty() && text == trimmed,
        None => false,
    };
    if schema_ok && platform_ok && entry_matches && selected_matches && !self_link {
        Ok(())
    } else { Err("E_INPUT_SNAPSHOT: invalid selector binding schema") }
}

/// 选择器优先使用 v3；直接配置仅在版本 2、3、4 时使用 v2。
pub fn select_snapshot_schema(version: Option<u64>, selected: bool) -> (schema: &'static str)
    ensures schema@ == if selected { "chrono-input-snapshot/v3"@ }
        else if version == Some(2u64) || version == Some(3u64) || version == Some(4u64) {
            "chrono-input-snapshot/v2"@
        } else { "chrono-input-snapshot/v1"@ },
{
    if selected { SNAPSHOT_SCHEMA_V3 }
    else {
        match version {
            Some(2) | Some(3) | Some(4) => SNAPSHOT_SCHEMA_V2,
            _ => SNAPSHOT_SCHEMA,
        }
    }
}

/// Decide the endpoint version gate before decoding the rest of a retained snapshot.
pub fn admit_endpoint_snapshot_version(
    config_version: Option<u64>, selected: bool, schema: Option<&str>,
) -> (out: Result<(), &'static str>)
    ensures ({
        let required = config_version == Some(2u64) || config_version == Some(3u64)
            || config_version == Some(4u64);
        let expected = if selected { "chrono-input-snapshot/v3"@ } else { "chrono-input-snapshot/v2"@ };
        let accepted = !required || match schema { Some(value) => value@ == expected, None => false };
        match out {
            Ok(()) => accepted,
            Err(message) => !accepted && message@ == if selected {
                "selected config requires snapshot v3"@
            } else { "config v2 requires snapshot v2"@ },
        }
    }),
{
    match config_version {
        Some(2) | Some(3) | Some(4) => {
            let expected = select_snapshot_schema(config_version, selected);
            if let Some(value) = schema {
                if value == expected { return Ok(()); }
            }
            if selected { Err("selected config requires snapshot v3") }
            else { Err("config v2 requires snapshot v2") }
        },
        _ => Ok(()),
    }
}

/// Parse the retained representation without reading or repairing its contents.
pub enum Retained<'a, B = Vec<u8>> {
    Absent,
    Bytes(B),
    Blob {
        path: &'a str,
        digest: &'a str,
        length: u64,
    },
}

/// 只检查缺失表示与版本的相容性；其余表示保留原内容校验义务。
pub fn admit_snapshot_retained<B>(schema: Option<&str>, retained: &Retained<B>)
    -> (out: Result<(), &'static str>)
    ensures match out {
        Ok(()) => match *retained {
            Retained::Absent => snapshot_format_version(schema) == 2 || snapshot_format_version(schema) == 3,
            _ => true,
        },
        Err(message) => matches!(*retained, Retained::Absent)
            && snapshot_format_version(schema) != 2 && snapshot_format_version(schema) != 3
            && message@ == "E_INPUT_SNAPSHOT: absence requires snapshot v2"@,
    },
{
    match *retained {
        Retained::Absent => {
            let version = classify_snapshot_schema(schema);
            if version != 2 && version != 3 {
                return Err("E_INPUT_SNAPSHOT: absence requires snapshot v2");
            }
            Ok(())
        },
        _ => Ok(()),
    }
}

}
