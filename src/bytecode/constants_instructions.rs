use super::*;

pub fn nop_instruction(_context: JvmContext) -> JvmResult<()> {
    Ok(())
}

pub fn integer_const_instruction<const VALUE: i32>(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    frame.operand_stack.push(JvmValue::Int(VALUE));

    Ok(())
}

pub fn bipush_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    let bytecode =
        frame.class.class_file.methods[frame.method_index].get_bytecode(frame.bytecode_index);
    // sign-extend the value
    let push_value = (bytecode.code[frame.program_counter] as i8) as i32;
    frame.program_counter += 1;

    frame.operand_stack.push(JvmValue::Int(push_value));

    Ok(())
}

pub fn sipush_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.peek().unwrap();
    // sign-extend the value
    let push_value = (read_u16_from_bytecode(frame) as i16) as i32;

    frame.operand_stack.push(JvmValue::Int(push_value));

    Ok(())
}
