use crate::{jvm::JVM, jvm_model::JvmValue};

mod bytecode;
mod class_file;
mod class_loader;
mod class_parser;
mod jvm;
mod jvm_model;

fn main() {
    let result = class_parser::parse("test_classes/Test.class").unwrap();

    println!("this class:{}", result.get_class_name().unwrap());
    println!("super class:{}", result.get_super_class_name().unwrap());

    println!("Test.class:{result:?}");

    let result = class_parser::parse("test_classes/TestSum.class").unwrap();

    println!("this class:{}", result.get_class_name().unwrap());
    println!("super class:{}", result.get_super_class_name().unwrap());

    println!("TestSum.class:{result:?}");

    let mut jvm = JVM::new(vec!["test_classes".to_owned()]);
    jvm.run("Test".to_owned(), "hello".to_owned(), vec![])
        .unwrap();
}
