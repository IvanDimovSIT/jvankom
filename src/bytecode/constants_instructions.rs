use std::rc::Rc;

use crate::{
    class_cache::CacheEntry,
    class_file::ConstantValue,
    initialise_class_and_rewind_runtime,
    jvm_cache::string_pool::{self, StringPool},
    jvm_heap::JvmHeap,
    jvm_model::{CLASS_CLASS_NAME, JvmClass, STRING_CLASS_NAME},
    object_initalisation::{create_class_object, create_string_object},
};

use super::*;

pub fn nop_instruction(_context: JvmContext) -> JvmResult<()> {
    Ok(())
}

pub fn null_const_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.top_frame();
    frame.operand_stack.push(JvmValue::Reference(None));

    Ok(())
}

pub fn integer_const_instruction<const VALUE: i32>(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.top_frame();
    frame.operand_stack.push(JvmValue::Int(VALUE));

    Ok(())
}

pub fn bipush_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.top_frame();
    let read_value = read_u8_from_bytecode(frame);
    // sign-extend the value
    let push_value = (read_value as i8) as i32;
    frame.operand_stack.push(JvmValue::Int(push_value));

    Ok(())
}

pub fn sipush_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.top_frame();
    // sign-extend the value
    let push_value = (read_u16_from_bytecode(frame) as i16) as i32;

    frame.operand_stack.push(JvmValue::Int(push_value));

    Ok(())
}

pub fn ldc2w_instruction(context: JvmContext) -> JvmResult<()> {
    let frame = context.current_thread.top_frame();
    let bytecode_value = read_u16_from_bytecode(frame);

    let constant_index = validate_cp_index(bytecode_value)?;

    let value = match frame.class.class_file.constant_pool.get(constant_index) {
        ConstantValue::Long(long) => JvmValue::Long(*long),
        ConstantValue::Double(double) => JvmValue::Double(*double),
        _ => return Err(JvmError::InvalidConstantPoolIndex.bx()),
    };

    frame.operand_stack.push(value);

    Ok(())
}

pub fn ldc_instruction(context: JvmContext) -> JvmResult<()> {
    const INSTRUCTION_SIZE: usize = 2;
    generic_ldc_instruction::<INSTRUCTION_SIZE, _>(context, |frame| {
        let bytecode_value = read_u8_from_bytecode(frame) as u16;
        validate_cp_index(bytecode_value)
    })
}

pub fn ldc_w_instruction(context: JvmContext) -> JvmResult<()> {
    const INSTRUCTION_SIZE: usize = 3;
    generic_ldc_instruction::<INSTRUCTION_SIZE, _>(context, |frame| {
        let bytecode_value = read_u16_from_bytecode(frame);
        validate_cp_index(bytecode_value)
    })
}

fn generic_ldc_instruction<const INSTRUCTION_SIZE: usize, F>(
    context: JvmContext,
    read_index_fn: F,
) -> JvmResult<()>
where
    F: FnOnce(&mut JvmStackFrame) -> JvmResult<NonZeroUsize>,
{
    let frame = context.current_thread.top_frame();
    let constant_index = read_index_fn(frame)?;

    let value = match frame.class.class_file.constant_pool.get(constant_index) {
        ConstantValue::Int(int) => JvmValue::Int(*int),
        ConstantValue::Float(float) => JvmValue::Float(*float),
        ConstantValue::Class { name_index } => {
            let class_class = context.class_loader.get(CLASS_CLASS_NAME)?;

            if !class_class.state.borrow().is_initialised {
                initialise_class_and_rewind_runtime!(
                    frame,
                    context,
                    &class_class,
                    INSTRUCTION_SIZE
                );
            }

            let class_name = frame
                .class
                .class_file
                .constant_pool
                .expect_utf8(*name_index);

            let obj = create_class_object(&class_class, class_name)?;
            let class_obj_ref = context.heap.allocate(obj);

            JvmValue::Reference(Some(class_obj_ref))
        }
        ConstantValue::String { utf8_index } => {
            if let Some(string_reference) = frame
                .class
                .state
                .borrow()
                .cache
                .get_string_pool_ref(constant_index.get() as u16)
            {
                JvmValue::Reference(Some(string_reference))
            } else {
                let string_class = context.class_loader.get(STRING_CLASS_NAME)?;

                // initialise class and rewind
                if !string_class.state.borrow().is_initialised {
                    initialise_class_and_rewind_runtime!(
                        frame,
                        context,
                        &string_class,
                        INSTRUCTION_SIZE
                    );
                }

                create_string_from_constant(
                    frame,
                    &mut context.cache.string_pool,
                    context.heap,
                    *utf8_index,
                    constant_index.get() as u16,
                    &string_class,
                )?
            }
        }
        ConstantValue::MethodRef {
            class_index,
            name_and_type_index,
        } => unimplemented!(),
        ConstantValue::InterfaceMethodRef {
            class_index,
            name_and_type_index,
        } => unimplemented!(),
        ConstantValue::MethodHandle {
            reference_kind,
            reference_index,
        } => unimplemented!(),
        ConstantValue::MethodType { descriptor_index } => unimplemented!(),
        _ => return Err(JvmError::InvalidConstantPoolIndex.bx()),
    };

    frame.operand_stack.push(value);

    Ok(())
}

fn create_string_from_constant(
    frame: &mut JvmStackFrame,
    string_pool: &mut StringPool,
    heap: &mut JvmHeap,
    utf8_index: NonZeroUsize,
    constant_index: u16,
    string_class: &Rc<JvmClass>,
) -> JvmResult<JvmValue> {
    let text = frame.class.class_file.constant_pool.expect_utf8(utf8_index);
    let string_ref = if let Some(string_ref) = string_pool.find_string(text) {
        string_ref
    } else {
        let mut string_obj = create_string_object(string_class)?;
        string_pool.initialise_string_fields(text, &mut string_obj, heap);
        let reference = heap.allocate(string_obj);
        string_pool.register(text, reference);

        reference
    };
    let entry = Rc::new(CacheEntry::StringPoolRef(string_ref));
    frame
        .class
        .state
        .borrow_mut()
        .cache
        .register(constant_index, entry);

    Ok(JvmValue::Reference(Some(string_ref)))
}
