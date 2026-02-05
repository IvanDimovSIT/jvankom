use super::*;

pub fn integer_add(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();

    let value_b = pop_int(frame)?;
    let value_a = pop_int(frame)?;

    let result_value = JvmValue::Int(value_a.wrapping_add(value_b));
    frame.operand_stack.push(result_value);

    Ok(())
}
