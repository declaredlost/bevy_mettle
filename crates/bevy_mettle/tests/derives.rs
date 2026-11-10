//! What the derives reject: each case fails to compile, as its `.stderr` says.
#![cfg(feature = "ability")]

#[test]
fn derives_reject_what_they_cant_key() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/derives/*.rs");
}
