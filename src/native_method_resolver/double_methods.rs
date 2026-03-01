use crate::{
    bytecode::{expect_double, expect_long},
    jvm_heap::JvmHeap,
    jvm_model::{JvmResult, JvmThread, JvmValue},
};

pub fn double_to_raw_long_bits(
    thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    params: Vec<JvmValue>,
) -> JvmResult<()> {
    let double = expect_double(params[0])?;
    let long = f64::to_bits(double).cast_signed();
    let frame = thread.peek().unwrap();
    frame.operand_stack.push(JvmValue::Long(long));

    Ok(())
}

pub fn long_bits_to_double(
    thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    params: Vec<JvmValue>,
) -> JvmResult<()> {
    let long = expect_long(params[0])?;
    let double: f64 = f64::from_bits(i64::cast_unsigned(long));
    let frame = thread.peek().unwrap();
    frame.operand_stack.push(JvmValue::Double(double));

    Ok(())
}
