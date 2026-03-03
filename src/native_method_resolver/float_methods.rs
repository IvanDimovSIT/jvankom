use crate::{
    bytecode::expect_float,
    class_loader::ClassLoader,
    jvm_heap::JvmHeap,
    jvm_model::{JvmResult, JvmThread, JvmValue},
};

pub fn float_to_raw_int_bits(
    thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _class_loader: &mut ClassLoader,
    params: Vec<JvmValue>,
) -> JvmResult<()> {
    let float = expect_float(params[0])?;
    let int = f32::to_bits(float).cast_signed();
    let frame = thread.peek().unwrap();
    frame.operand_stack.push(JvmValue::Int(int));

    Ok(())
}
