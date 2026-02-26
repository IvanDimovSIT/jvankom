use super::*;

pub fn increment_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    let bytecode =
        frame.class.class_file.methods[frame.method_index].get_bytecode(frame.bytecode_index);

    let index_value = bytecode.code[frame.program_counter] as usize;
    frame.program_counter += 1;
    debug_assert!(frame.program_counter < bytecode.code.len());

    let const_value = bytecode.code[frame.program_counter] as i8;
    frame.program_counter += 1;
    debug_assert!(frame.program_counter < bytecode.code.len());

    if index_value >= frame.local_variables.len() {
        return Err(JvmError::NoLocalVariableFound.bx());
    }

    match &mut frame.local_variables[index_value] {
        JvmValue::Int(v) => *v = v.wrapping_add(const_value as i32),
        _ => {
            return Err(JvmError::TypeError {
                expected: JvmType::Int,
                found: frame.local_variables[index_value].get_type(),
            }
            .bx());
        }
    }

    Ok(())
}

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
