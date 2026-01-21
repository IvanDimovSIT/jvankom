mod class_file;
mod class_parser;

fn main() {
    let result = class_parser::parse("test_classes/Test.class").unwrap();

    println!("this class:{}", result.get_class_name().unwrap());
    println!("super class:{}", result.get_super_class_name().unwrap());

    println!("{result:?}");
}
