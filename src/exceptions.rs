use std::{num::NonZeroUsize, rc::Rc};

use crate::{
    class_loader::ClassLoader,
    jvm_heap::JvmHeap,
    jvm_model::{FrameReturn, HeapObject, JvmClass, JvmError, JvmResult, JvmThread, JvmValue},
};

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
        if frame.bytecode_index >= start_pc && frame.bytecode_index < end_pc {
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
                frame.return_value = None;
                frame.should_return = FrameReturn::NotReturning;

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
