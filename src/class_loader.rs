use std::{collections::HashMap, rc::Rc};

use crate::{
    class_file::ClassFile,
    class_parser::{self, ClassParserError},
    jvm_model::JvmError,
};

#[derive(Debug)]
pub struct LoadedClass {
    pub class: Rc<ClassFile>,
    pub is_initialised: bool,
}

#[derive(Debug)]
pub struct ClassLoader {
    loaded_classes: HashMap<String, Rc<ClassFile>>,
    contexts: Vec<String>,
}
impl ClassLoader {
    pub fn new(contexts: Vec<String>) -> Self {
        Self {
            loaded_classes: HashMap::new(),
            contexts,
        }
    }

    pub fn get(&mut self, class_name: &str) -> Result<LoadedClass, JvmError> {
        let initialised_class = self.loaded_classes.get(class_name);
        if let Some(class) = initialised_class {
            let loaded_class = LoadedClass {
                class: class.clone(),
                is_initialised: true,
            };
            return Ok(loaded_class);
        }

        let class = Rc::new(self.find_class_file(class_name)?);
        self.loaded_classes
            .insert(class_name.to_owned(), class.clone());
        let loaded_class = LoadedClass {
            class: class.clone(),
            is_initialised: false,
        };

        Ok(loaded_class)
    }

    fn find_class_file(&self, class_name: &str) -> Result<ClassFile, JvmError> {
        for path in &self.contexts {
            let class_path = format!("{path}/{class_name}.class");
            let class_result = class_parser::parse(&class_path);
            match class_result {
                Ok(class) => {
                    return Ok(class);
                }
                Err(ClassParserError::ErrorReadingFile(_)) => {
                    continue;
                }
                Err(err) => {
                    return Err(JvmError::ClassParserError(err));
                }
            }
        }

        let class_result = class_parser::parse(&format!("{class_name}.class"));
        match class_result {
            Ok(class) => Ok(class),
            Err(ClassParserError::ErrorReadingFile(err)) => Err(JvmError::ClassLoaderError(
                format!("Cannot find '{class_name}':{err}"),
            )),
            Err(err) => Err(JvmError::ClassParserError(err)),
        }
    }
}
