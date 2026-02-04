use super::*;

pub fn pop_instruction(
    thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _class_loader: &mut ClassLoader,
) -> JvmResult<()> {
    let frame = thread.peek().unwrap();
    let poped_value = frame.operand_stack.pop();
    debug_assert!(poped_value.is_some());
    debug_assert!(!matches!(poped_value.unwrap(), JvmValue::Unusable));

    Ok(())
}
