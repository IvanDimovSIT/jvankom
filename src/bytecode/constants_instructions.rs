use super::*;

pub fn nop_instruction(
    _thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _class_loader: &mut ClassLoader,
) -> JvmResult<()> {
    Ok(())
}

pub fn integer_const_instruction<const VALUE: i32>(
    thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _class_loader: &mut ClassLoader,
) -> JvmResult<()> {
    let frame = thread.peek().unwrap();
    frame.operand_stack.push(JvmValue::Int(VALUE));

    Ok(())
}
