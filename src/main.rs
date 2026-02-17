use crate::{
    class_loader::{ClassLoader, ClassSource},
    jvm::JVM,
    jvm_model::JvmValue,
};

mod bytecode;
mod class_file;
mod class_loader;
mod class_parser;
mod jvm;
mod jvm_model;
mod method_call_cache;
mod native_method_resolver;
mod object_instantiation_cache;
mod verifier;

fn main() {
    let class =
        verifier::verify_class_file(class_parser::parse("test_classes/ObjectTest.class").unwrap())
            .unwrap();
    println!("File:\n{class:?}");

    let class_loader = ClassLoader::new(vec![
        ClassSource::Jar("java_libraries/rt.jar".to_owned()),
        ClassSource::Directory("test_classes".to_owned()),
    ])
    .unwrap();
    let mut jvm = JVM::new(class_loader);
    let result = jvm.run(
        "ObjectTest".to_owned(),
        "callInstanceMethod".to_owned(),
        "(I)I".to_owned(),
        vec![JvmValue::Int(10)],
    );

    if let Err(err) = result {
        println!("Error: {err}");
        jvm.get_threads()[0].peek().unwrap().debug_print();
        return;
    }

    let jvm_value = result.unwrap().unwrap();

    println!("{jvm_value:?}");
}
