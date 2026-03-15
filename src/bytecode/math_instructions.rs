use crate::throw_arithmetic_exception;

use super::*;

pub fn increment_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.top_frame();
    let index_value = read_u8_from_bytecode(frame) as usize;
    let const_value = read_u8_from_bytecode(frame) as i8;

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

pub fn integer_add_instruction(context: JvmContext) -> JvmResult<()> {
    generic_two_operand_instruction(context, pop_int, |a, b| JvmValue::Int(a.wrapping_add(b)))
}

pub fn integer_subtract_instruction(context: JvmContext) -> JvmResult<()> {
    generic_two_operand_instruction(context, pop_int, |a, b| JvmValue::Int(a.wrapping_sub(b)))
}

pub fn integer_muliply_instruction(context: JvmContext) -> JvmResult<()> {
    generic_two_operand_instruction(context, pop_int, |a, b| JvmValue::Int(a * b))
}

pub fn integer_and_instruction(context: JvmContext) -> JvmResult<()> {
    generic_two_operand_instruction(context, pop_int, |a, b| JvmValue::Int(a & b))
}

pub fn integer_or_instruction(context: JvmContext) -> JvmResult<()> {
    generic_two_operand_instruction(context, pop_int, |a, b| JvmValue::Int(a | b))
}

pub fn integer_xor_instruction(context: JvmContext) -> JvmResult<()> {
    generic_two_operand_instruction(context, pop_int, |a, b| JvmValue::Int(a ^ b))
}

pub fn integer_logical_shift_right_instruction(context: JvmContext) -> JvmResult<()> {
    generic_two_operand_instruction(context, pop_int, |a, b| {
        let shift_amount = b & 0b11111;
        JvmValue::Int((a as u32 >> shift_amount) as i32)
    })
}

pub fn integer_shift_left_instruction(context: JvmContext) -> JvmResult<()> {
    generic_shift_instruction(context, pop_int, |a, b| {
        JvmValue::Int(a << (b & 0b11111) as u32)
    })
}

pub fn integer_shift_right_instruction(context: JvmContext) -> JvmResult<()> {
    generic_shift_instruction(context, pop_int, |a, b| {
        JvmValue::Int(a >> (b & 0b11111) as u32)
    })
}

pub fn integer_negate_instruction(context: JvmContext) -> JvmResult<()> {
    generic_one_operand_instruction(context, pop_int, |x| JvmValue::Int(-x))
}

pub fn integer_divide_instruction(context: JvmContext) -> JvmResult<()> {
    generic_dvision_instruction(context, pop_int, |a, b| JvmValue::Int(a / b))
}

pub fn integer_remainder_instruction(context: JvmContext) -> JvmResult<()> {
    generic_dvision_instruction(context, pop_int, |a, b| JvmValue::Int(a % b))
}

pub fn long_and_instruction(context: JvmContext) -> JvmResult<()> {
    generic_two_operand_instruction(context, pop_long, |a, b| JvmValue::Long(a & b))
}

pub fn long_add_instruction(context: JvmContext) -> JvmResult<()> {
    generic_two_operand_instruction(context, pop_long, |a, b| JvmValue::Long(a + b))
}

pub fn shift_left_long_instruction(context: JvmContext) -> JvmResult<()> {
    generic_shift_instruction(context, pop_long, |a, b| {
        JvmValue::Long(a << (b & 0b111111) as u32)
    })
}

#[inline]
fn generic_shift_instruction<P, T, M>(context: JvmContext, pop_fn: P, math_fn: M) -> JvmResult<()>
where
    P: FnOnce(&mut JvmStackFrame) -> JvmResult<T>,
    M: FnOnce(T, i32) -> JvmValue,
{
    let frame = context.current_thread.top_frame();

    let value_2 = pop_int(frame)?;
    let value_1 = pop_fn(frame)?;

    let result_value = math_fn(value_1, value_2);
    frame.operand_stack.push(result_value);

    Ok(())
}

#[inline]
fn generic_two_operand_instruction<P, T, M>(
    context: JvmContext,
    pop_fn: P,
    math_fn: M,
) -> JvmResult<()>
where
    P: Fn(&mut JvmStackFrame) -> JvmResult<T>,
    M: FnOnce(T, T) -> JvmValue,
{
    let frame = context.current_thread.top_frame();

    let value_b = pop_fn(frame)?;
    let value_a = pop_fn(frame)?;

    let result_value = math_fn(value_a, value_b);
    frame.operand_stack.push(result_value);

    Ok(())
}

#[inline]
fn generic_one_operand_instruction<P, T, M>(
    context: JvmContext,
    pop_fn: P,
    math_fn: M,
) -> JvmResult<()>
where
    P: FnOnce(&mut JvmStackFrame) -> JvmResult<T>,
    M: FnOnce(T) -> JvmValue,
{
    let frame = context.current_thread.top_frame();
    let value = pop_fn(frame)?;
    let result_value = math_fn(value);
    frame.operand_stack.push(result_value);

    Ok(())
}

/// same as generic_two_operand_instruction but checks if b != 0
fn generic_dvision_instruction<P, T, M>(context: JvmContext, pop_fn: P, math_fn: M) -> JvmResult<()>
where
    P: Fn(&mut JvmStackFrame) -> JvmResult<T>,
    M: FnOnce(T, T) -> JvmValue,
    T: PartialEq + Default,
{
    let frame = context.current_thread.top_frame();

    let value_b = pop_fn(frame)?;
    let value_a = pop_fn(frame)?;
    if value_b == T::default() {
        throw_arithmetic_exception!(frame, context, 1);
    }

    let result_value = math_fn(value_a, value_b);
    frame.operand_stack.push(result_value);

    Ok(())
}
