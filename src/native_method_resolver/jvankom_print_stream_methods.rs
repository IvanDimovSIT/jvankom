#[cfg(test)]
use std::sync::Mutex;

use crate::{
    bytecode::expect_reference,
    class_loader::ClassLoader,
    exceptions::throw_jvm_exception,
    jvm::Jvm,
    jvm_cache::string_pool::StringPool,
    jvm_heap::JvmHeap,
    jvm_model::{
        DescriptorType, HeapObject, JVANKOM_PRINT_STEAM_NAME, JvmResult, JvmThread, JvmValue,
        NULL_POINTER_EXCEPTION_NAME,
    },
};

pub fn construct(
    thread: &mut JvmThread,
    heap: &mut JvmHeap,
    class_loader: &mut ClassLoader,
    _params: Vec<JvmValue>,
) -> JvmResult<()> {
    let frame = thread.top_frame();
    let class = class_loader.get(JVANKOM_PRINT_STEAM_NAME)?;
    let print_steam = HeapObject::Object {
        class: class.clone(),
        fields: vec![],
    };
    let reference = heap.allocate(print_steam);
    frame
        .operand_stack
        .push(JvmValue::Reference(Some(reference)));
    if !class.state.borrow().is_initialised {
        Jvm::initialise_class(thread, &class, class_loader, JVANKOM_PRINT_STEAM_NAME)?;
    }

    Ok(())
}

pub fn native_write_string(
    thread: &mut JvmThread,
    heap: &mut JvmHeap,
    class_loader: &mut ClassLoader,
    params: Vec<JvmValue>,
) -> JvmResult<()> {
    let string_ref = expect_reference(params[0])?;
    let string = if let Some(str_ref) = string_ref {
        heap.get(str_ref)
    } else {
        return throw_jvm_exception(thread, heap, class_loader, NULL_POINTER_EXCEPTION_NAME);
    };
    let data_jvm_value = match string {
        HeapObject::Object { class, fields } => {
            let data_index = StringPool::find_value_field(&class.state.borrow());
            fields[data_index]
        }
        _ => todo!("Throw exception"),
    };
    let data_ref = expect_reference(data_jvm_value)?;
    let data_obj = if let Some(data_not_null) = data_ref {
        heap.get(data_not_null)
    } else {
        return throw_jvm_exception(thread, heap, class_loader, NULL_POINTER_EXCEPTION_NAME);
    };
    let characters = match data_obj {
        HeapObject::CharacterArray(items) => items,
        _ => todo!("Throw exception"),
    };
    print_chars(characters);

    Ok(())
}

fn print_chars(chars: &[u16]) {
    let string: String = chars.iter().map(|c| (*c) as u8 as char).collect();
    #[cfg(test)]
    {
        *crate::native_method_resolver::PRINT_LOG.lock().unwrap() += &string;
    }

    println!("{string}");
}
