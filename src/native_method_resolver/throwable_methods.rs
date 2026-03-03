use crate::bytecode::{expect_int, expect_reference};

use super::*;

pub fn fill_in_stack_trace(
    thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    params: Vec<JvmValue>,
) -> JvmResult<()> {
    let this_ref = expect_reference(params[0])?;
    expect_int(params[1])?;
    //TODO: fill in stack trace
    thread
        .peek()
        .unwrap()
        .operand_stack
        .push(JvmValue::Reference(this_ref));

    Ok(())
}
