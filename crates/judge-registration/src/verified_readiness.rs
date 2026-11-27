//! full 检查直接消费的登记状态与输入闭合判断。
use vstd::prelude::*;

verus! {

/// Ordered depth-first dependency search. A missing record or malformed edge
/// ends this search at the same point as the registry protocol.
pub open spec fn walk(graph: Seq<Vec<Option<usize>>>, pending: Seq<usize>,
    seen: Set<usize>, target: usize, fuel: nat) -> bool
    decreases fuel, 0nat,
{
    if fuel == 0 || pending.len() == 0 { false }
    else {
        let node = pending.last();
        let tail = pending.drop_last();
        if seen.contains(node) { walk(graph, tail, seen, target, (fuel - 1) as nat) }
        else if node >= graph.len() { false }
        else { edges(graph, graph[node as int]@, 0, tail, seen.insert(node), target, (fuel - 1) as nat) }
    }
}

pub open spec fn edges(graph: Seq<Vec<Option<usize>>>, row: Seq<Option<usize>>, slot: nat,
    pending: Seq<usize>, seen: Set<usize>, target: usize, fuel: nat) -> bool
    decreases fuel, (row.len() - slot) as nat + 1,
{
    if slot >= row.len() { walk(graph, pending, seen, target, fuel) }
    else {
        match row[slot as int] {
            None => false,
            Some(next) => next == target || edges(graph, row, slot + 1, pending.push(next), seen, target, fuel),
        }
    }
}

pub fn validator_reaches(graph: &[Vec<Option<usize>>], validator: Option<usize>, target: Option<usize>,
    steps: usize) -> (found: bool)
    ensures found == match (validator, target) {
        (Some(start),Some(goal)) => start != goal && walk(graph@, seq![start], Set::empty(), goal, steps as nat),
        _ => false,
    },
{
    broadcast use vstd::std_specs::btree::group_btree_axioms;
    let (Some(start), Some(goal)) = (validator, target) else { return false; };
    if start == goal { return false; }
    let ghost intended = walk(graph@, seq![start], Set::empty(), goal, steps as nat);
    let mut pending = Vec::new();
    pending.push(start);
    let mut seen = std::collections::BTreeSet::<usize>::new();
    let mut remaining = steps;
    while remaining > 0 && pending.len() > 0
        invariant validator == Some(start), target == Some(goal), start != goal,
            vstd::std_specs::btree::btree_set_inv(&seen),
            walk(graph@, pending@, seen@, goal, remaining as nat) == intended,
            intended == walk(graph@, seq![start], Set::empty(), goal, steps as nat),
        decreases remaining,
    {
        let ghost original = pending@;
        let ghost visited = seen@;
        let node = pending.pop().unwrap();
        assert(original == pending@.push(node));
        assert(original.drop_last() =~= pending@);
        assert(original.last() == node);
        remaining -= 1;
        if !seen.insert(node) {
            assert(seen@ =~= visited);
            assert(walk(graph@, original, visited, goal, (remaining + 1) as nat) ==
                walk(graph@, pending@, seen@, goal, remaining as nat));
            continue;
        }
        if node >= graph.len() {
            assert(!walk(graph@, original, visited, goal, (remaining + 1) as nat));
            return false;
        }
        let row = &graph[node];
        assert(walk(graph@, original, visited, goal, (remaining + 1) as nat) ==
            edges(graph@, row@, 0, pending@, seen@, goal, remaining as nat));
        let mut index = 0;
        while index < row.len()
            invariant validator == Some(start), target == Some(goal), start != goal, index <= row.len(), edges(graph@, row@, index as nat, pending@, seen@, goal, remaining as nat)
                == intended,
                intended == walk(graph@, seq![start], Set::empty(), goal, steps as nat),
            decreases row.len() - index,
        {
            let Some(next) = row[index] else {
                assert(!edges(graph@, row@, index as nat, pending@, seen@, goal, remaining as nat));
                return false;
            };
            if next == goal {
                assert(edges(graph@, row@, index as nat, pending@, seen@, goal, remaining as nat));
                return true;
            }
            pending.push(next);
            index += 1;
        }
    }
    false
}

pub open spec fn active(status: Option<&str>) -> bool {
    match status {
        Some(text) => text@ == "active"@,
        None => false,
    }
}

pub fn registry_active(status: Option<&str>) -> (ready: bool)
    ensures ready == active(status),
{
    match status {
        Some(text) => text == "active",
        None => false,
    }
}

pub open spec fn ready_inputs<T>(
    enforcement: Option<&str>, closure: Option<&str>, unresolved: Option<&Vec<T>>,
) -> bool {
    (match enforcement { Some(text) => text@ == "enabled"@, None => false })
        && (match closure { Some(text) => text@ == "declared-complete"@, None => false })
        && (match unresolved { Some(inputs) => inputs@.len() == 0, None => false })
}

pub fn input_ready<T>(
    enforcement: Option<&str>, closure: Option<&str>, unresolved: Option<&Vec<T>>,
) -> (ready: bool)
    ensures ready == ready_inputs(enforcement, closure, unresolved),
{
    (match enforcement { Some(text) => text == "enabled", None => false })
        && (match closure { Some(text) => text == "declared-complete", None => false })
        && (match unresolved { Some(inputs) => inputs.len() == 0, None => false })
}

pub open spec fn snapshot_bound(
    schema_present: bool, files: usize, strict_environment: bool, inherited: usize,
    actual: Option<&str>, expected: &str,
) -> bool {
    (schema_present || files > 0 || (strict_environment && inherited > 0))
        ==> (match actual { Some(text) => text@ == expected@, None => false })
}

pub fn endpoint_snapshot_matches(
    schema_present: bool, files: usize, strict_environment: bool, inherited: usize,
    actual: Option<&str>, expected: &str,
) -> (matches: bool)
    ensures matches == snapshot_bound(
        schema_present, files, strict_environment, inherited, actual, expected,
    ),
{
    if schema_present || files > 0 || (strict_environment && inherited > 0) {
        match actual {
            Some(text) => text == expected,
            None => false,
        }
    } else {
        true
    }
}

}
