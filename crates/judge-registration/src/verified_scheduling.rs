//! 调度只要求参与计划的操作都有声明，真实生产声明可独立保留。
use std::collections::{BTreeMap, BTreeSet};
#[cfg(not(verus_keep_ghost))]
use vstd::prelude::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

verus! {

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
    &&& forall|i: int, j: int| 0 <= i < j < end ==> #[trigger] ops[i] != #[trigger] ops[j]
}

pub open spec fn permitted_override(
    ops: Seq<String>, name: String, bounds: Bounds, limit: usize,
) -> bool {
    ops.contains(name) && bounds.admitted(limit)
}

pub fn valid_plan(
    ops: &[String], global: Bounds, overrides: &BTreeMap<String, Bounds>, limit: usize,
) -> (ok: bool)
    requires vstd::laws_cmp::obeys_cmp::<String>(), vstd::laws_cmp::obeys_cmp::<&String>(),
    ensures ok == (ops@.len() > 0 && admitted_operations(ops@, ops@.len() as int)
        && global.admitted(limit)
        && forall|name: String| #[trigger] overrides@.contains_key(name)
            ==> permitted_override(ops@, name, overrides@[name], limit)),
{
    broadcast use vstd::std_specs::btree::axiom_set_contains_deref_key;
    if ops.is_empty() { return false; }
    let mut selected: BTreeSet<&String> = BTreeSet::new();
    let mut pos = 0;
    while pos < ops.len()
        invariant
            vstd::laws_cmp::obeys_cmp::<&String>(),
            0 <= pos <= ops.len(),
            admitted_operations(ops@, pos as int),
            forall|name: &String| #[trigger] selected@.contains(name)
                <==> exists|j: int| 0 <= j < pos && #[trigger] ops@[j] == *name,
        decreases ops.len() - pos,
    {
        let name = &ops[pos];
        if name.as_str().is_empty() {
            assert(ops@[pos as int]@.len() == 0);
            return false;
        }
        if !selected.insert(name) {
            let ghost previous = choose|j: int| 0 <= j < pos && #[trigger] ops@[j] == *name;
            assert(ops@[previous] == ops@[pos as int]);
            return false;
        }
        pos += 1;
    }
    if !global.valid(limit) { return false; }
    let mut remaining = overrides.iter();
    loop
        invariant
            vstd::laws_cmp::obeys_cmp::<&String>(),
            admitted_operations(ops@, ops@.len() as int), global.admitted(limit), ops@.len() > 0,
            forall|name: &String| #[trigger] selected@.contains(name) <==> ops@.contains(*name),
            remaining@.1.to_set() == overrides@.kv_pairs(),
            0 <= remaining@.0 <= remaining@.1.len(),
            forall|i: int| 0 <= i < remaining@.0 ==>
                permitted_override(ops@, remaining@.1[i].0, remaining@.1[i].1, limit),
        decreases remaining@.1.len() - remaining@.0 + 1,
    {
        let (name, bounds) = match remaining.next() {
            Some(item) => item,
            None => {
                assert forall|name: String| #[trigger] overrides@.contains_key(name)
                    implies permitted_override(ops@, name, overrides@[name], limit) by {
                    assert(overrides@.kv_pairs().contains((name, overrides@[name])));
                    let i = choose|i: int| 0 <= i < remaining@.1.len()
                        && remaining@.1[i] == (name, overrides@[name]);
                }
                return true;
            },
        };
        if !selected.contains(&name) || !bounds.valid(limit) {
            assert(overrides@.kv_pairs().contains((*name, *bounds)));
            return false;
        }
    }
}

pub fn claims_cover_operations<K: Ord, V>(
    operations: &BTreeSet<K>, claims: &BTreeMap<K, V>,
) -> (covered: bool)
    requires vstd::laws_cmp::obeys_cmp::<K>(),
    ensures covered == operations@.subset_of(claims@.dom()),
{
    broadcast use vstd::std_specs::btree::axiom_contains_deref_key;
    let mut remaining = operations.iter();
    loop
        invariant vstd::laws_cmp::obeys_cmp::<K>(),
            remaining@.1.to_set() == operations@,
            0 <= remaining@.0 <= remaining@.1.len(),
            forall|i: int| 0 <= i < remaining@.0
                ==> claims@.contains_key(remaining@.1[i]),
        decreases remaining@.1.len() - remaining@.0 + 1,
    {
        let operation = match remaining.next() {
            Some(operation) => operation,
            None => {
                assert forall|key: K| #[trigger] operations@.contains(key)
                    implies claims@.contains_key(key) by {
                    let at = choose|i: int| 0 <= i < remaining@.1.len()
                        && remaining@.1[i] == key;
                }
                return true;
            },
        };
        if !claims.contains_key(operation) {
            assert(operations@.contains(*operation));
            return false;
        }
    }
}

}
