use crate::{jvm::JVM, jvm_model::JvmValue};

mod bytecode;
mod class_file;
mod class_loader;
mod class_parser;
mod jvm;
mod jvm_model;
mod verifier;

fn main() {
    let class =
        verifier::verify_class_file(class_parser::parse("test_classes/TestSum.class").unwrap())
            .unwrap();
    println!("File:\n{class:?}");

    let mut jvm = JVM::new(vec!["test_classes".to_owned()]);
    let result = jvm
        .run(
            "TestSum".to_owned(),
            "arrayTest".to_owned(),
            vec![JvmValue::Int(100), JvmValue::Int(0), JvmValue::Int(3)],
        )
        .unwrap()
        .unwrap();

    println!("{result:?}");
}
