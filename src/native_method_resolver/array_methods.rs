use std::num::NonZeroUsize;

use crate::{
    bytecode::{expect_int, expect_reference},
    class_loader::ClassLoader,
    jvm_heap::JvmHeap,
    jvm_model::{
        HeapObject, JvmResult, JvmThread, JvmValue, OBJECT_CLASS_NAME, ObjectArray, ObjectArrayType,
    },
};

pub fn new_array(
    thread: &mut JvmThread,
    heap: &mut JvmHeap,
    class_loader: &mut ClassLoader,
    params: Vec<JvmValue>,
) -> JvmResult<()> {
    let _class_ref = expect_reference(params[0])?;
    let count = expect_int(params[1])?;

    let array = HeapObject::ObjectArray(ObjectArray {
        array: vec![None; count as usize],
        dimension: NonZeroUsize::new(1).unwrap(),
        object_array_type: ObjectArrayType::Class(class_loader.get_object_class()?),
    });
    let array_ref = heap.allocate(array);
    thread
        .top_frame()
        .operand_stack
        .push(JvmValue::Reference(Some(array_ref)));

    Ok(())
}
