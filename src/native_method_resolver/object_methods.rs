use std::{
    hash::{DefaultHasher, Hasher},
    ptr::hash,
};

use crate::{
    bytecode::expect_reference,
    class_loader::ClassLoader,
    exceptions::throw_jvm_exception,
    jvm::Jvm,
    jvm_heap::JvmHeap,
    jvm_model::{
        CLASS_CLASS_NAME, JvmResult, JvmThread, JvmValue, NULL_POINTER_EXCEPTION_NAME,
        OBJECT_CLASS_NAME,
    },
    object_initalisation::create_class_object,
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

pub fn get_class(
    thread: &mut JvmThread,
    heap: &mut JvmHeap,
    class_loader: &mut ClassLoader,
    _params: Vec<JvmValue>,
) -> JvmResult<()> {
    let frame = thread.top_frame();
    let class_class = class_loader.get(CLASS_CLASS_NAME)?;
    let obj = create_class_object(&class_class, OBJECT_CLASS_NAME)?;
    let obj_ref = heap.allocate(obj);
    frame.operand_stack.push(JvmValue::Reference(Some(obj_ref)));

    if class_class.state.borrow().is_initialised {
        Jvm::initialise_class(thread, &class_class, class_loader, CLASS_CLASS_NAME)?;
    }

    Ok(())
}
