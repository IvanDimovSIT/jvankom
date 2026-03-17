use crate::{
    class_loader::{ClassLoader, ClassSource},
    jvm::Jvm,
    jvm_heap::JvmHeap,
};

mod bytecode;
mod class_cache;
mod class_file;
mod class_loader;
mod class_parser;
mod exceptions;
mod field_initialisation;
mod jvm;
mod jvm_cache;
mod jvm_heap;
mod jvm_model;
mod native_method_resolver;
mod object_initalisation;
mod v_table;
mod verifier;

fn main() {
    let class =
        verifier::verify_class_file(class_parser::parse("test_classes/PrintTest.class").unwrap())
            .unwrap();
    println!("File:\n{class:?}");

    let class_loader = ClassLoader::new(vec![
        ClassSource::Jar("java_libraries/rt.jar".to_owned()),
        ClassSource::Jar("java_libraries/jvankomrt.jar".to_owned()),
        ClassSource::Directory("test_classes/".to_owned()),
    ])
    .unwrap();
    let heap = JvmHeap::new(1000, 1000);
    let mut jvm = Jvm::new(class_loader, heap);
    let result = jvm.run_main(
        "ListTest".to_owned(),
        vec!["Hello".to_owned(), "World!".to_owned()],
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
            .expect_utf8(method_index);
        let desc = frame
            .class
            .class_file
            .constant_pool
            .expect_utf8(descriptor_index);
        println!("=>{method}{desc}");

        return;
    }

    show_cache_storage(&jvm);
}

fn show_cache_storage(jvm: &Jvm) {
    let (used, total) = jvm.get_cache_storage_efficieny();
    println!(
        "JVM cache storage efficiency: {}/{}, {}%",
        used,
        total,
        100.0 * used as f64 / total as f64
    )
}
