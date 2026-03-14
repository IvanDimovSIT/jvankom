use std::{
    hash::{DefaultHasher, Hasher},
    ptr::hash,
    u32,
};

use crate::{
    bytecode::expect_reference,
    class_loader::ClassLoader,
    exceptions::throw_jvm_exception,
    jvm_heap::JvmHeap,
    jvm_model::{JvmResult, JvmThread, JvmValue, NULL_POINTER_EXCEPTION_NAME},
};

pub fn object_constructor(
    _thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _class_loader: &mut ClassLoader,
    _params: Vec<JvmValue>,
) -> JvmResult<()> {
    Ok(())
}

pub fn register_natives(
    _thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _class_loader: &mut ClassLoader,
    _params: Vec<JvmValue>,
) -> JvmResult<()> {
    Ok(())
}

pub fn hash_code(
    thread: &mut JvmThread,
    heap: &mut JvmHeap,
    class_loader: &mut ClassLoader,
    params: Vec<JvmValue>,
) -> JvmResult<()> {
    let frame = thread.top_frame();
    let reference = expect_reference(params[0])?;
    if let Some(obj_ref) = reference {
        let mut hasher = DefaultHasher::new();
        hash(&obj_ref, &mut hasher);
        let hash_result = (hasher.finish() & u32::MAX as u64) as i32;
        frame.operand_stack.push(JvmValue::Int(hash_result));
    } else {
        return throw_jvm_exception(thread, heap, class_loader, NULL_POINTER_EXCEPTION_NAME);
    };

    Ok(())
}
