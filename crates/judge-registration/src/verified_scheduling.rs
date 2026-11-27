//! 调度只要求参与计划的操作都有声明，真实生产声明可独立保留。
use vstd::hash_set::StringHashSet;
#[cfg(not(verus_keep_ghost))]
use vstd::prelude::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

verus! {

/// The protocol checks timeout before output capacity and preserves both values.
#[derive(Debug, PartialEq, Eq)]
pub enum ProtocolLimitError {
    Timeout,
    Output,
}

pub fn protocol_limits(time: Option<u64>, size: Option<u64>)
    -> (result: Result<(u64, u64), ProtocolLimitError>)
    ensures result == match time {
        Some(t) if t > 0 => match size {
            Some(n) if n > 0 => Ok((t, n)),
            _ => Err(ProtocolLimitError::Output),
        },
        _ => Err(ProtocolLimitError::Timeout),
    },
{
    match (time, size) {
        (None, _) | (Some(0), _) => Err(ProtocolLimitError::Timeout),
        (_, None) | (_, Some(0)) => Err(ProtocolLimitError::Output),
        (Some(t), Some(n)) => Ok((t, n)),
    }
}

#[cfg_attr(not(verus_keep_ghost), derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(not(verus_keep_ghost), serde(deny_unknown_fields))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub timeout_seconds: u64,
    pub output_limit_bytes: usize,
}

impl Bounds {
    pub open spec fn admitted(self, limit: usize) -> bool {
        self.timeout_seconds > 0 && 0 < self.output_limit_bytes <= limit
    }

    pub fn valid(&self, limit: usize) -> (ok: bool)
        ensures ok == self.admitted(limit),
    {
        self.timeout_seconds > 0 && self.output_limit_bytes > 0 && self.output_limit_bytes <= limit
    }
}

pub open spec fn admitted_operations(ops: Seq<String>, end: int) -> bool {
    &&& forall|i: int| 0 <= i < end ==> #[trigger] ops[i]@.len() > 0
    &&& forall|i: int, j: int| 0 <= i < j < end ==> #[trigger] ops[i]@ != #[trigger] ops[j]@
}

pub open spec fn names_operation(ops: Seq<String>, name: String, end: int) -> bool {
    exists|i: int| 0 <= i < end && #[trigger] ops[i]@ == name@
}

pub open spec fn permitted_override(
    ops: Seq<String>, name: String, bounds: Bounds, limit: usize,
) -> bool {
    names_operation(ops, name, ops.len() as int) && bounds.admitted(limit)
}

pub fn valid_plan(
    ops: &[String], global: Bounds, overrides: &[(&String, &Bounds)], limit: usize,
) -> (ok: bool)
    ensures ok == (ops@.len() > 0 && admitted_operations(ops@, ops@.len() as int)
        && global.admitted(limit)
        && forall|i: int| 0 <= i < overrides.len() ==> permitted_override(
            ops@, *#[trigger] overrides@[i].0, *overrides@[i].1, limit)),
{
    if ops.is_empty() { return false; }
    let mut selected = StringHashSet::new();
    let mut pos = 0;
    while pos < ops.len()
        invariant
            selected.well_formed(), 0 <= pos <= ops.len(), admitted_operations(ops@, pos as int),
            forall|name: Seq<char>| #[trigger] selected@.contains(name)
                <==> exists|j: int| 0 <= j < pos && #[trigger] ops@[j]@ == name,
        decreases ops.len() - pos,
    {
        let name = &ops[pos];
        if name.is_empty() { return false; }
        if !selected.insert(name.clone()) {
            let ghost previous = choose|j: int| 0 <= j < pos && #[trigger] ops@[j]@ == name@;
            assert(ops@[previous]@ == ops@[pos as int]@);
            return false;
        }
        pos += 1;
    }
    if !global.valid(limit) { return false; }
    let mut index = 0;
    while index < overrides.len()
        invariant
            selected.well_formed(), 0 <= index <= overrides.len(),
            forall|name: Seq<char>| #[trigger] selected@.contains(name)
                <==> exists|j: int| 0 <= j < ops.len() && #[trigger] ops@[j]@ == name,
            forall|i: int| 0 <= i < index ==> permitted_override(
                ops@, *#[trigger] overrides@[i].0, *overrides@[i].1, limit),
        decreases overrides.len() - index,
    {
        let (name, bounds) = overrides[index];
        if !selected.contains(name.as_str()) || !bounds.valid(limit) {
            assert(!permitted_override(ops@, *name, *bounds, limit));
            return false;
        }
        index += 1;
    }
    true
}

}
