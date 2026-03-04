use crate::{
    class_loader::ClassLoader,
    jvm_heap::JvmHeap,
    jvm_model::{JvmResult, JvmThread, JvmValue},
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
