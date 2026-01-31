use crate::jvm::JVM;

mod bytecode;
mod class_file;
mod class_loader;
mod class_parser;
mod jvm;
mod jvm_model;
mod verifier;

fn main() {
    let result = class_parser::parse("test_classes/Test.class").unwrap();
    let class = verifier::verify_class_file(result).unwrap();

    println!("this class:{}", class.get_class_name().unwrap());
    println!("super class:{}", class.get_super_class_name().unwrap());

    println!("Test.class:{class:?}");

    let result = class_parser::parse("test_classes/TestSum.class").unwrap();

    println!("this class:{}", class.get_class_name().unwrap());
    println!("super class:{}", class.get_super_class_name().unwrap());

    println!("TestSum.class:{result:?}");

    let mut jvm = JVM::new(vec!["test_classes".to_owned()]);
    jvm.run("Test".to_owned(), "hello".to_owned(), vec![])
        .unwrap();
}
