use crate::jvm_model::{JvmHeap, JvmResult, JvmThread, JvmValue};

pub fn object_constructor(
    _thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _params: Vec<JvmValue>,
) -> JvmResult<()> {
    Ok(())
}

pub fn register_natives(
    _thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _params: Vec<JvmValue>,
) -> JvmResult<()> {
    Ok(())
}
