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
mod verifier;

fn main() {
    let class = verifier::verify_class_file(
        class_parser::parse("test_classes/TestMethodCall.class").unwrap(),
    )
    .unwrap();
    println!("File:\n{class:?}");

    let class_loader =
        ClassLoader::new(vec![ClassSource::Directory("test_classes".to_owned())]).unwrap();
    let mut jvm = JVM::new(class_loader);
    let result = jvm
        .run(
            "TestMethodCall".to_owned(),
            "mainCall".to_owned(),
            "(II)I".to_owned(),
            vec![JvmValue::Int(1000), JvmValue::Int(100)],
        )
        .unwrap()
        .unwrap();

    println!("{result:?}");
}
