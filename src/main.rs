use crate::{
    class_loader::{ClassLoader, ClassSource},
    jvm::JVM,
    jvm_heap::JvmHeap,
    jvm_model::JvmValue,
};

mod bytecode;
mod class_file;
mod class_loader;
mod class_parser;
mod field_access_cache;
mod field_initialisation;
mod jvm;
mod jvm_heap;
mod jvm_model;
mod method_call_cache;
mod native_method_resolver;
mod object_creation_cache;
mod string_pool;
mod v_table;
mod verifier;

fn main() {
    let class =
        verifier::verify_class_file(class_parser::parse("test_classes/ObjectTest.class").unwrap())
            .unwrap();
    println!("File:\n{class:?}");

    let class_loader = ClassLoader::new(vec![
        ClassSource::Jar("java_libraries/rt.jar".to_owned()),
        ClassSource::Directory("test_classes/".to_owned()),
    ])
    .unwrap();
    let heap = JvmHeap::new(1000, 1000);
    let mut jvm = JVM::new(class_loader, heap);
    let result = jvm.run(
        "TestString".to_owned(),
        "main".to_owned(),
        "(I)I".to_owned(),
        vec![JvmValue::Int(1)],
    );

    if let Err(err) = result {
        println!("\n\tERROR: {err}\n");
        let frame = jvm.get_threads()[0].peek().unwrap().clone();
        frame.debug_print();
        let method_index = frame.class.class_file.methods[frame.method_index].name_index;
        let descriptor_index = frame.class.class_file.methods[frame.method_index].descriptor_index;
        let method = frame
            .class
            .class_file
            .constant_pool
            .get_utf8(method_index)
            .unwrap();
        let desc = frame
            .class
            .class_file
            .constant_pool
            .get_utf8(descriptor_index)
            .unwrap();
        println!("=>{method}{desc}");

        return;
    }

    let jvm_value = result.unwrap().unwrap();

    println!("{jvm_value:?}");
}
