use super::*;

pub fn integer_add(
    thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _class_loader: &mut ClassLoader,
) -> JvmResult<()> {
    let frame = thread.peek().unwrap();

    let value_b = pop_int(frame)?;
    let value_a = pop_int(frame)?;

    let result_value = JvmValue::Int(value_a.wrapping_add(value_b));
    frame.operand_stack.push(result_value);

    Ok(())
}
