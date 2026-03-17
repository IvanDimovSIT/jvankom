use crate::{
    bytecode::expect_reference,
    class_loader::ClassLoader,
    exceptions::throw_jvm_exception,
    jvm_heap::JvmHeap,
    jvm_model::{
        CLASS_CLASS_NAME, HeapObject, JvmError, JvmResult, JvmThread, JvmValue,
        NULL_POINTER_EXCEPTION_NAME, OBJECT_CLASS_NAME, STRING_CLASS_NAME,
    },
    object_initalisation::create_class_object,
};

pub fn register_natives(
    _thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _class_loader: &mut ClassLoader,
    _params: Vec<JvmValue>,
) -> JvmResult<()> {
    Ok(())
}

pub fn desired_assertion_status0(
    thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _class_loader: &mut ClassLoader,
    params: Vec<JvmValue>,
) -> JvmResult<()> {
    expect_reference(params[0])?;

    let frame = thread.peek().unwrap();
    frame.operand_stack.push(JvmValue::Int(0));

    Ok(())
}

pub fn get_primitive_class(
    thread: &mut JvmThread,
    _heap: &mut JvmHeap,
    _class_loader: &mut ClassLoader,
    params: Vec<JvmValue>,
) -> JvmResult<()> {
    expect_reference(params[0])?;

    let frame = thread.peek().unwrap();
    frame.operand_stack.push(JvmValue::Reference(None));

    Ok(())
}

pub fn get_component_type(
    thread: &mut JvmThread,
    heap: &mut JvmHeap,
    class_loader: &mut ClassLoader,
    params: Vec<JvmValue>,
) -> JvmResult<()> {
    let nullable_ref = expect_reference(params[0])?;
    let this_ref = if let Some(reference) = nullable_ref {
        reference
    } else {
        return throw_jvm_exception(thread, heap, class_loader, NULL_POINTER_EXCEPTION_NAME);
    };
    let class_class = match heap.get(this_ref) {
        HeapObject::Object { class, fields: _ } => class,
        _ => return Err(JvmError::ExpectedNonArrayObject.bx()),
    };
    debug_assert_eq!(class_class.class_file.get_class_name(), CLASS_CLASS_NAME);

    let class_obj = create_class_object(class_class, OBJECT_CLASS_NAME)?;
    let class_ref = heap.allocate(class_obj);
    thread
        .top_frame()
        .operand_stack
        .push(JvmValue::Reference(Some(class_ref)));

    Ok(())
}
