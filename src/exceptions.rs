use std::{num::NonZeroUsize, rc::Rc};

use crate::{
    class_loader::ClassLoader,
    field_initialisation::{determine_non_static_field_types, initialise_object_fields},
    jvm::JVM,
    jvm_heap::JvmHeap,
    jvm_model::{
        FrameReturn, HeapObject, JvmClass, JvmError, JvmResult, JvmStackFrame, JvmThread, JvmValue,
    },
};

const EXCEPTION_CONSTRUCTOR_NAME: &str = "<init>";
const EXCEPTION_CONSTRUCTOR_DESC: &str = "()V";

/// throws a NullPointerException, $size is the size of the instruction
#[macro_export]
macro_rules! throw_null_pointer_exception {
    ($frame:expr, $context:expr, $size:expr) => {{
        const _CHECK_SIZE: () = assert!($size > 0);

        $frame.program_counter -= $size - 1; // rewind
        return $crate::exceptions::throw_jvm_exception(
            $context.current_thread,
            $context.heap,
            $context.class_loader,
            $crate::jvm_model::NULL_POINTER_EXCEPTION_NAME,
        );
    }};
}

/// throws a ArrayIndexOutOfBoundsException, $size is the size of the instruction
#[macro_export]
macro_rules! throw_array_index_out_of_bounds_exception {
    ($frame:expr, $context:expr, $size:expr) => {{
        const _CHECK_SIZE: () = assert!($size > 0);

        $frame.program_counter -= $size - 1; // rewind
        return $crate::exceptions::throw_jvm_exception(
            $context.current_thread,
            $context.heap,
            $context.class_loader,
            $crate::jvm_model::ARRAY_INDEX_OUT_OF_BOUNDS_EXCEPTION_NAME,
        );
    }};
}

///  throws a NegativeArraySizeException, $size is the size of the instruction
#[macro_export]
macro_rules! throw_negative_array_size_exception {
    ($frame:expr, $context:expr, $size:expr) => {{
        const _CHECK_SIZE: () = assert!($size > 0);

        $frame.program_counter -= $size - 1; // rewind
        return $crate::exceptions::throw_jvm_exception(
            $context.current_thread,
            $context.heap,
            $context.class_loader,
            $crate::jvm_model::NEGATIVE_ARRAY_SIZE_EXCEPTION_NAME,
        );
    }};
}

///  throws a ArrayStoreException, $size is the size of the instruction
#[macro_export]
macro_rules! throw_array_store_exception {
    ($frame:expr, $context:expr, $size:expr) => {{
        const _CHECK_SIZE: () = assert!($size > 0);

        $frame.program_counter -= $size - 1; // rewind
        return $crate::exceptions::throw_jvm_exception(
            $context.current_thread,
            $context.heap,
            $context.class_loader,
            $crate::jvm_model::ARRAY_STORE_EXCEPTION_NAME,
        );
    }};
}

/// handles exceptions - the PC must not include increments from multi-byte instructions
pub fn handle_exception(
    thread: &mut JvmThread,
    heap: &mut JvmHeap,
    class_loader: &mut ClassLoader,
) -> JvmResult<()> {
    let frame = thread.peek().unwrap();
    debug_assert_eq!(FrameReturn::Exception, frame.should_return);
    let reference = match frame.return_value.unwrap() {
        JvmValue::Reference(Some(r)) => r,
        _ => unreachable!("Should only be reference"),
    };
    let (exception_class, ex_fields) = match heap.get(reference) {
        HeapObject::Object { class, fields } => (class, fields),
        _ => unreachable!("Should only be non-array object"),
    };

    let bytecode =
        frame.class.class_file.methods[frame.method_index].get_bytecode(frame.bytecode_index);

    for ex_entry in &bytecode.exception_table {
        let catch_type = ex_entry.catch_type as usize;
        let start_pc = ex_entry.start_pc as usize;
        let end_pc = ex_entry.end_pc as usize;
        let handler_pc = ex_entry.handler_pc as usize;
        // compensate for instruction pc increment (must not include multi-byte increments)
        let pre_increment_pc = frame.program_counter - 1;
        if pre_increment_pc >= start_pc && pre_increment_pc < end_pc {
            let handler_class = if let Some(index) = NonZeroUsize::new(catch_type) {
                let class_name = frame
                    .class
                    .class_file
                    .constant_pool
                    .get_class_name(index)
                    .expect("Should be validated");
                Some(class_loader.get(class_name)?)
            } else {
                None
            };

            if handler_class.is_none()
                || JvmClass::is_sublcass_of(&handler_class.unwrap(), exception_class)
            {
                frame.operand_stack.clear();
                frame
                    .operand_stack
                    .push(JvmValue::Reference(Some(reference)));
                frame.program_counter = handler_pc;
                frame.unset_exception();

                return Ok(());
            }
        }
    }

    if thread.len() == 1 {
        thread.pop();
        Err(JvmError::UnhandledException {
            reference,
            class_name: exception_class
                .class_file
                .get_class_name()
                .unwrap()
                .to_owned(),
            fields: ex_fields.clone(),
        }
        .bx())
    } else if thread.has_frames() {
        thread.pop();
        let frame = thread.peek().unwrap();
        frame.return_value = Some(JvmValue::Reference(Some(reference)));
        frame.should_return = FrameReturn::Exception;
        handle_exception(thread, heap, class_loader)
    } else {
        Ok(())
    }
}

/// The PC must not include increments from multi-byte instructions
/// The PC must be at the instruction that threw the exception + 1 (it is compensated by -1)
pub fn throw_jvm_exception(
    thread: &mut JvmThread,
    heap: &mut JvmHeap,
    class_loader: &mut ClassLoader,
    exception_type: &str,
) -> JvmResult<()> {
    let constructor_frame_index = thread.len();
    let throwing_frame_index = constructor_frame_index - 1;
    let exception_class = class_loader.get(exception_type)?;
    if !exception_class.state.borrow().is_initialised {
        JVM::initialise_class(thread, &exception_class, class_loader, exception_type)?;
    }
    if exception_class.state.borrow().non_static_fields.is_none() {
        let fields = determine_non_static_field_types(&exception_class)?;
        exception_class.state.borrow_mut().non_static_fields = Some(fields);
    }
    let exception_object = if let Some(obj) = &exception_class.state.borrow().default_object {
        obj.clone()
    } else {
        let mut state = exception_class.state.borrow_mut();
        let obj = initialise_object_fields(
            exception_class.clone(),
            state
                .non_static_fields
                .as_ref()
                .expect("Fields not initialised"),
        );
        state.default_object = Some(obj.clone());
        obj
    };
    let exception_ref = heap.allocate(exception_object);
    let exception_ref_value = JvmValue::Reference(Some(exception_ref));
    let constructor_frame = call_exception_constructor(exception_class, exception_ref_value)?;
    thread.insert(constructor_frame_index, constructor_frame);

    let throwing_frame = thread.peek_at(throwing_frame_index);
    throwing_frame.set_exception(exception_ref);

    Ok(())
}

fn call_exception_constructor(
    exception_class: Rc<JvmClass>,
    exception_ref_value: JvmValue,
) -> JvmResult<JvmStackFrame> {
    let (method_index, bytecode_index) = if let Some(index) = exception_class
        .class_file
        .get_method_and_bytecode_index(EXCEPTION_CONSTRUCTOR_NAME, EXCEPTION_CONSTRUCTOR_DESC)
    {
        index
    } else {
        return Err(JvmError::MethodNotFound {
            class_name: exception_class
                .class_file
                .get_class_name()
                .expect("expected class name")
                .to_owned(),
            method_name: EXCEPTION_CONSTRUCTOR_NAME.to_owned(),
        }
        .bx());
    };
    if bytecode_index.is_none() {
        return Err(JvmError::ExpectedNonNativeMethod {
            method_name: EXCEPTION_CONSTRUCTOR_NAME.to_owned(),
            method_descriptor: EXCEPTION_CONSTRUCTOR_DESC.to_owned(),
        }
        .bx());
    }
    let stack_frame = JvmStackFrame::new(
        exception_class.clone(),
        method_index,
        bytecode_index.unwrap(),
        vec![exception_ref_value],
    );

    Ok(stack_frame)
}
