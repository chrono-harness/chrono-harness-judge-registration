//! Registration admits the declared prefixes and base for each canonical source file.
use vstd::prelude::*;

verus! {

/// The source file owner supplies its canonical-name verdict along with the declared syntax.
pub trait ReferenceDeclaration {
    spec fn fields_view(&self) -> (bool, Seq<char>, Seq<char>, u8);

    fn fields(&self) -> (out: (bool, &str, &str, u8))
        ensures self.fields_view() == (out.0, out.1@, out.2@, out.3);
}

pub open spec fn prefix_valid(text: Seq<char>, base: u8) -> bool {
    let bytes = vstd::utf8::encode_utf8(text);
    bytes.len() > 0 && (bytes.last() == 34 || (base == 2 && bytes.last() == 47))
}

pub open spec fn declared_reference_valid<T: ReferenceDeclaration>(form: &T) -> bool {
    let (canonical, before, after, base) = form.fields_view();
    canonical && prefix_valid(before, base) && prefix_valid(after, base) && base <= 2
}

fn valid_prefix(text: &str, base: u8) -> (out: bool)
    ensures out == prefix_valid(text@, base),
{
    let bytes = text.as_bytes();
    bytes.len() > 0 && (bytes[bytes.len() - 1] == 34
        || (base == 2 && bytes[bytes.len() - 1] == 47))
}

pub open spec fn reference_declarations_valid<T: ReferenceDeclaration>(forms: Seq<T>) -> bool {
    forms.len() <= 250 && forall|i: int| 0 <= i < forms.len() ==> declared_reference_valid(#[trigger] &forms[i])
}

pub fn validate_reference_syntax<T: ReferenceDeclaration>(forms: &[T]) -> (out: Result<(), String>)
    ensures out.is_ok() == reference_declarations_valid(forms@),
        match out { Err(error) => error@ == "E_SOURCE_CONTINUITY: invalid reference syntax declaration"@, Ok(()) => true },
{
    if forms.len() > 250 {
        return Err("E_SOURCE_CONTINUITY: invalid reference syntax declaration".to_string());
    }
    let mut i = 0;
    while i < forms.len()
        invariant 0 <= i <= forms.len() <= 250,
            forall|j: int| 0 <= j < i ==> declared_reference_valid(#[trigger] &forms[j]),
        decreases forms.len() - i,
    {
        let (canonical, before, after, base) = forms[i].fields();
        if !canonical || !valid_prefix(before, base) || !valid_prefix(after, base) || base > 2 {
            return Err("E_SOURCE_CONTINUITY: invalid reference syntax declaration".to_string());
        }
        i += 1;
    }
    Ok(())
}

}
