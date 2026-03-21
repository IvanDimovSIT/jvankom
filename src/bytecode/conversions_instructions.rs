use super::*;

pub fn int_to_long_instruction(context: JvmContext) -> JvmResult<()> {
    generic_conversion_instruction(context, pop_int, |int| JvmValue::Long(int as i64))
}

pub fn int_to_char_instruction(context: JvmContext) -> JvmResult<()> {
    generic_conversion_instruction(context, pop_int, JvmValue::Int)
}

#[inline]
fn generic_conversion_instruction<P, T, C>(
    context: JvmContext,
    pop_fn: P,
    conversion_fn: C,
) -> JvmResult<()>
where
    P: FnOnce(&mut JvmStackFrame) -> JvmResult<T>,
    C: FnOnce(T) -> JvmValue,
{
    let frame = context.current_thread.top_frame();
    let value = pop_fn(frame)?;
    let result_value = conversion_fn(value);
    frame.operand_stack.push(result_value);

    Ok(())
}
