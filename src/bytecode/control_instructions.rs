use crate::jvm_model::FrameReturn;

use super::*;

pub fn lookup_switch_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.top_frame();
    let instruction_start = (frame.program_counter - 1) as i32;
    let padding_size = (4 - ((frame.program_counter) % 4)) % 4;
    frame.program_counter += padding_size;
    debug_assert!(frame.program_counter.is_multiple_of(4));
    let default = read_i32_from_bytecode(frame);
    let npairs = read_i32_from_bytecode(frame);
    let mut pairs = Vec::with_capacity(npairs as usize);
    for _ in 0..npairs {
        let match_key = read_i32_from_bytecode(frame);
        let offset = read_i32_from_bytecode(frame);
        pairs.push((match_key, offset));
    }

    let key = pop_int(frame)?;
    let offset = pairs
        .into_iter()
        .find(|(match_key, _offset)| key == *match_key)
        .map(|(_match_key, offset)| offset)
        .unwrap_or(default);

    let target_address = instruction_start + offset;
    frame.program_counter = target_address as usize;

    Ok(())
}

pub fn table_switch_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.top_frame();
    let instruction_start = (frame.program_counter - 1) as i32;
    let padding_size = (4 - ((frame.program_counter) % 4)) % 4;
    frame.program_counter += padding_size;
    debug_assert!(frame.program_counter.is_multiple_of(4));
    let default = read_i32_from_bytecode(frame);
    let low = read_i32_from_bytecode(frame);
    let high = read_i32_from_bytecode(frame);
    let jump_table_size = high - low + 1;
    let mut jump_table = Vec::with_capacity(jump_table_size as usize);
    for _ in 0..jump_table_size {
        jump_table.push(read_i32_from_bytecode(frame));
    }

    let index = pop_int(frame)?;
    let target_address = if index < low || index > high {
        instruction_start + default
    } else {
        instruction_start + jump_table[(index - low) as usize]
    };
    debug_assert!(
        target_address >= 0
            && target_address
                < frame.class.class_file.methods[frame.method_index]
                    .get_bytecode(frame.bytecode_index)
                    .code
                    .len() as i32
    );

    frame.program_counter = target_address as usize;

    Ok(())
}

pub fn goto_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.top_frame();
    let offset_base = frame.program_counter as isize - 1;
    let offset = read_i16_from_bytecode(frame) as isize;
    frame.program_counter = (offset_base + offset) as usize;

    Ok(())
}

pub fn return_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.top_frame();
    frame.should_return = FrameReturn::Returning;
    Ok(())
}

#[inline]
fn generic_return_instruction<F, T>(context: JvmContext, expect_generic: F) -> JvmResult<()>
where
    F: FnOnce(JvmValue) -> JvmResult<T>,
{
    let frame = context.current_thread.top_frame();
    if let Some(value_to_return) = frame.operand_stack.pop() {
        expect_generic(value_to_return)?;
        frame.should_return = FrameReturn::Returning;
        frame.return_value = Some(value_to_return);
    } else {
        return Err(JvmError::NoOperandFound.bx());
    }

    Ok(())
}

pub fn integer_return_instruction(context: JvmContext) -> JvmResult<()> {
    generic_return_instruction(context, expect_int)
}

pub fn object_return_instruction(context: JvmContext) -> JvmResult<()> {
    generic_return_instruction(context, expect_reference)
}

pub fn double_return_instruction(context: JvmContext) -> JvmResult<()> {
    generic_return_instruction(context, expect_double)
}
