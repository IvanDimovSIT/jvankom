use std::{
    collections::HashMap,
    error::Error,
    fmt::Display,
    fs::{self, File},
    io::Read,
    rc::Rc,
};

use zip::ZipArchive;

use crate::{
    class_file::ClassFile,
    class_parser::{self, ClassParserError, UnverifiedClassFile},
    jvm_model::{JvmClass, JvmError, JvmResult},
    verifier,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassSource {
    Directory(String),
    Jar(String),
}

#[derive(Debug, Clone)]
pub struct ClassLoaderError {
    invalid_sources: Vec<ClassSource>,
}
impl Display for ClassLoaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Invalid sources: {}",
            self.invalid_sources
                .iter()
                .map(|s| format!("{s:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}
impl Error for ClassLoaderError {}

#[derive(Debug)]
pub struct ClassLoader {
    loaded_classes: HashMap<String, Rc<JvmClass>>,
    jars: Vec<ZipArchive<File>>,
    directories: Vec<String>,
}
impl ClassLoader {
    pub fn new(contexts: Vec<ClassSource>) -> Result<Self, ClassLoaderError> {
        let mut directories = vec![];
        let mut jars = vec![];
        let mut invalid_sources = vec![];

        for source in contexts {
            match source {
                ClassSource::Directory(path) => {
                    if Self::check_directory_path(&path) {
                        directories.push(path)
                    } else {
                        invalid_sources.push(ClassSource::Directory(path));
                    }
                }
                ClassSource::Jar(path) => {
                    if let Ok(f) = File::open(&path) {
                        if let Ok(jar) = ZipArchive::new(f) {
                            jars.push(jar);
                        } else {
                            invalid_sources.push(ClassSource::Jar(path));
                        }
                    } else {
                        invalid_sources.push(ClassSource::Jar(path));
                    }
                }
            }
        }

        if invalid_sources.is_empty() {
            Ok(Self {
                loaded_classes: HashMap::new(),
                directories,
                jars,
            })
        } else {
            Err(ClassLoaderError { invalid_sources })
        }
    }

    pub fn get_all_loaded_classes(&self) -> impl Iterator<Item = &Rc<JvmClass>> {
        self.loaded_classes.values()
    }

    pub fn get_loaded_count(&self) -> usize {
        self.loaded_classes.len()
    }

    pub fn get(&mut self, class_name: &str) -> JvmResult<Rc<JvmClass>> {
        let initialised_class = self.loaded_classes.get(class_name);
        if let Some(class) = initialised_class {
            return Ok(class.clone());
        }

        let class = self.find_class_file(class_name)?;

        self.loaded_classes
            .insert(class_name.to_owned(), class.clone());

        Ok(class)
    }

    fn find_class_file(&mut self, class_name: &str) -> JvmResult<Rc<JvmClass>> {
        for jar in &mut self.jars {
            if let Ok(mut file) = jar.by_name(&format!("{class_name}.class")) {
                let mut data = Vec::with_capacity(1024);
                if file.read_to_end(&mut data).is_err() {
                    continue;
                }
                let class_result = class_parser::parse_from_bytes(&data);
                if let Some(class_file) = Self::handle_parse_result(class_result, class_name)? {
                    return Ok(class_file);
                }
            }
        }

        for path in &self.directories {
            let class_path = format!("{path}/{class_name}.class");
            let class_result = class_parser::parse(&class_path);
            if let Some(class_file) = Self::handle_parse_result(class_result, class_name)? {
                return Ok(class_file);
            }
        }

        let class_result = class_parser::parse(&format!("{class_name}.class"));
        if let Some(class_file) = Self::handle_parse_result(class_result, class_name)? {
            Ok(class_file)
        } else {
            Err(JvmError::ClassLoaderError(format!("Cannot find '{class_name}'")).bx())
        }
    }

    fn verify(
        unverified_class_file: UnverifiedClassFile,
        class_name: &str,
    ) -> JvmResult<ClassFile> {
        match verifier::verify_class_file(unverified_class_file) {
            Ok(class_file) => Ok(class_file),
            Err(error) => Err(JvmError::ClassVerificationError {
                error,
                verified_class: class_name.to_owned(),
            }
            .bx()),
        }
    }

    fn handle_parse_result(
        class_result: Result<UnverifiedClassFile, ClassParserError>,
        class_name: &str,
    ) -> JvmResult<Option<Rc<JvmClass>>> {
        match class_result {
            Ok(class) => Self::verify(class, class_name).map(|c| Some(JvmClass::new(c))),
            Err(ClassParserError::ErrorReadingFile(_)) => Ok(None),
            Err(error) => Err(JvmError::ClassParserError {
                parsed_class: class_name.to_owned(),
                error,
            }
            .bx()),
        }
    }

    fn check_directory_path(path: &str) -> bool {
        fs::metadata(path).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_load_from_directory() {
        let mut class_loader =
            ClassLoader::new(vec![ClassSource::Directory("test_classes".to_owned())]).unwrap();

        let loaded_class = class_loader.get("Test").unwrap();
        assert!(!loaded_class.state.borrow().is_initialised);
        assert_eq!(1, class_loader.get_loaded_count());
        assert_eq!("Test", loaded_class.class_file.get_class_name().unwrap());
    }

    #[test]
    pub fn test_load_from_jar() {
        let mut class_loader = ClassLoader::new(vec![ClassSource::Jar(
            "test_classes/CrossCallTest.jar".to_owned(),
        )])
        .unwrap();

        let loaded_class = class_loader.get("CrossCall2Test").unwrap();
        assert!(!loaded_class.state.borrow().is_initialised);
        assert_eq!(1, class_loader.get_loaded_count());
        assert_eq!(
            "CrossCall2Test",
            loaded_class.class_file.get_class_name().unwrap()
        );
    }

    #[test]
    pub fn test_class_not_found() {
        let mut class_loader =
            ClassLoader::new(vec![ClassSource::Directory("test_classes".to_owned())]).unwrap();

        let loaded_class = class_loader.get("NonExistingClass");
        match loaded_class {
            Ok(_) => panic!("Class should not exist"),
            Err(err) => match *err {
                JvmError::ClassLoaderError(class) => {
                    debug_assert_eq!("Cannot find 'NonExistingClass'", class)
                }
                _ => panic!("Expected ClassLoaderError"),
            },
        }
    }

    #[test]
    pub fn test_jar_not_found() {
        let class_loader_result = ClassLoader::new(vec![ClassSource::Jar(
            "test_classes/InvlaidJarFile.jar".to_owned(),
        )]);

        match class_loader_result {
            Ok(_) => panic!("expected error"),
            Err(ClassLoaderError { invalid_sources }) => {
                assert_eq!(1, invalid_sources.len());
                assert_eq!(
                    ClassSource::Jar("test_classes/InvlaidJarFile.jar".to_owned(),),
                    invalid_sources[0]
                );
            }
        }
    }
}
