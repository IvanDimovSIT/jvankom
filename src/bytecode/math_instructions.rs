use super::*;

pub fn integer_add(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();

    let value_b = pop_int(frame)?;
    let value_a = pop_int(frame)?;

    let result_value = JvmValue::Int(value_a.wrapping_add(value_b));
    frame.operand_stack.push(result_value);

    Ok(())
}

pub fn integer_subtract(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();

    let value_b = pop_int(frame)?;
    let value_a = pop_int(frame)?;

    let result_value = JvmValue::Int(value_a.wrapping_sub(value_b));
    frame.operand_stack.push(result_value);

    Ok(())
}

pub fn integer_muliply(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();

    let value_b = pop_int(frame)?;
    let value_a = pop_int(frame)?;

    let result_value = JvmValue::Int(value_a * value_b);
    frame.operand_stack.push(result_value);

    Ok(())
}

pub fn integer_negate(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    let value = pop_int(frame)?;
    let result_value = JvmValue::Int(-value);
    frame.operand_stack.push(result_value);

    Ok(())
}

pub fn integer_divide(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();

    let value_b = pop_int(frame)?;
    let value_a = pop_int(frame)?;
    if value_b == 0 {
        todo!("Throw ArithmeticException");
    }

    let result_value = JvmValue::Int(value_a / value_b);
    frame.operand_stack.push(result_value);

    Ok(())
}

pub fn integer_remainder(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();

    let value_b = pop_int(frame)?;
    let value_a = pop_int(frame)?;
    if value_b == 0 {
        todo!("Throw ArithmeticException");
    }

    let result_value = JvmValue::Int(value_a % value_b);
    frame.operand_stack.push(result_value);

    Ok(())
}
