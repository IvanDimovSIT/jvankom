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
mod object_instantiation_cache;
mod verifier;

fn main() {
    let class = verifier::verify_class_file(
        class_parser::parse("test_classes/IntegerMathTest.class").unwrap(),
    )
    .unwrap();
    println!("File:\n{class:?}");

    let class_loader = ClassLoader::new(vec![ClassSource::Jar(
        "test_classes/CrossCallTest.jar".to_owned(),
    )])
    .unwrap();
    let mut jvm = JVM::new(class_loader);
    let result = jvm
        .run(
            "CrossCall1Test".to_owned(),
            "callOtherClass".to_owned(),
            "(I)I".to_owned(),
            vec![JvmValue::Int(10)],
        )
        .unwrap()
        .unwrap();

    println!("{result:?}");
}
