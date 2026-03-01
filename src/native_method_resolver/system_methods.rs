use crate::{
    jvm_heap::JvmHeap,
    jvm_model::{JvmResult, JvmThread, JvmValue},
};

pub fn register_natives(
    _thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _params: Vec<JvmValue>,
) -> JvmResult<()> {
    Ok(())
}
