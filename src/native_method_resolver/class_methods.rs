use crate::{
    bytecode::expect_reference,
    jvm_heap::JvmHeap,
    jvm_model::{JvmResult, JvmThread, JvmValue},
};

pub fn register_natives(
    _thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _params: Vec<JvmValue>,
) -> JvmResult<()> {
    Ok(())
}

pub fn desired_assertion_status0(
    thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    params: Vec<JvmValue>,
) -> JvmResult<()> {
    expect_reference(params[0])?;

    let frame = thread.peek().unwrap();
    frame.operand_stack.push(JvmValue::Int(0));

    Ok(())
}

pub fn get_primitive_class(
    thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    params: Vec<JvmValue>,
) -> JvmResult<()> {
    expect_reference(params[0])?;

    let frame = thread.peek().unwrap();
    frame.operand_stack.push(JvmValue::Reference(None));

    Ok(())
}
