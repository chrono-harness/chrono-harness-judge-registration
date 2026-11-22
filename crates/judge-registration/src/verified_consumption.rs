//! 精确判定已明确登记的 bootstrap 准备前置。
use vstd::{contrib::exec_spec::ExecSpecEq, prelude::*};

verus! {

#[verifier::inline]
pub open spec fn preparation(
    command: Seq<&str>, program: Seq<char>, script: Seq<char>, config: Seq<char>,
    schema: Seq<char>, prerequisites: Seq<&str>, verifier: Seq<char>,
    executor: Seq<char>, verifier_program: Seq<char>, projects: Seq<char>, expected_projects: Seq<char>,
) -> bool {
    command.len() == 4
        && command[0]@ == program && command[1]@ == script
        && command[2]@ == "."@ && command[3]@ == config
        && schema == "chrono-bootstrap/v1"@
        && prerequisites.len() == 1 && prerequisites[0]@ == verifier
        && executor == verifier_program && projects == expected_projects
}

pub fn matches_preparation(
    command: &Vec<&str>, program: &str, script: &str, config: &str,
    schema: &str, prerequisites: &Vec<&str>, verifier: &str,
    executor: &str, verifier_program: &str, projects: &str, expected_projects: &str,
) -> (matches: bool)
    ensures matches == preparation(command@, program@, script@, config@,
        schema@, prerequisites@, verifier@, executor@, verifier_program@, projects@, expected_projects@),
{
    command.len() == 4
        && <&str>::exec_eq(command[0], program) && <&str>::exec_eq(command[1], script)
        && <&str>::exec_eq(command[2], ".") && <&str>::exec_eq(command[3], config)
        && <&str>::exec_eq(schema, "chrono-bootstrap/v1")
        && prerequisites.len() == 1 && <&str>::exec_eq(prerequisites[0], verifier)
        && <&str>::exec_eq(executor, verifier_program) && <&str>::exec_eq(projects, expected_projects)
}

}
