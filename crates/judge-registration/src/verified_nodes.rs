//! 实际登记节点类型及其完整复制／相等语义。
#[cfg(not(verus_keep_ghost))]
use vstd::prelude::verus;
#[cfg(verus_keep_ghost)]
use vstd::prelude::*;

verus! {

#[derive(Copy)]
#[cfg_attr(not(verus_keep_ghost), derive(Debug, serde::Serialize))]
#[cfg_attr(not(verus_keep_ghost), serde(rename_all = "lowercase"))]
pub enum NodeKind {
    File,
    Project,
    Script,
    Input,
    Test,
    Judge,
    Tool,
    Environment,
}

impl NodeKind {
    pub fn input_consumer(&self) -> (allowed: bool)
        ensures allowed == matches!(self, Self::Project | Self::Script | Self::Test | Self::Judge),
    {
        matches!(self, Self::Project | Self::Script | Self::Test | Self::Judge)
    }

    pub fn external_input(&self) -> (allowed: bool)
        ensures allowed == matches!(self, Self::Tool | Self::Input | Self::Environment),
    {
        matches!(self, Self::Tool | Self::Input | Self::Environment)
    }
}

impl Clone for NodeKind {
    fn clone(&self) -> (out: Self)
        ensures out == *self,
    { *self }
}

#[cfg(verus_keep_ghost)]
impl vstd::std_specs::cmp::PartialEqSpecImpl for NodeKind {
    open spec fn obeys_eq_spec() -> bool { true }
    open spec fn eq_spec(&self, other: &Self) -> bool { *self == *other }
}

impl PartialEq for NodeKind {
    fn eq(&self, other: &Self) -> (equal: bool)
        ensures equal == (*self == *other),
    {
        match self {
            Self::File => matches!(other, Self::File),
            Self::Project => matches!(other, Self::Project),
            Self::Script => matches!(other, Self::Script),
            Self::Input => matches!(other, Self::Input),
            Self::Test => matches!(other, Self::Test),
            Self::Judge => matches!(other, Self::Judge),
            Self::Tool => matches!(other, Self::Tool),
            Self::Environment => matches!(other, Self::Environment),
        }
    }
}

impl Eq for NodeKind {}

}
