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
    jvm_model::{JvmError, JvmResult},
    verifier,
};

#[derive(Debug, Clone)]
pub enum ClassSource {
    Directory(String),
    Jar(String),
}

#[derive(Debug)]
pub struct LoadedClass {
    pub class: Rc<ClassFile>,
    pub is_initialised: bool,
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
    loaded_classes: HashMap<String, Rc<ClassFile>>,
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

    pub fn get(&mut self, class_name: &str) -> JvmResult<LoadedClass> {
        let initialised_class = self.loaded_classes.get(class_name);
        if let Some(class) = initialised_class {
            let loaded_class = LoadedClass {
                class: class.clone(),
                is_initialised: true,
            };
            return Ok(loaded_class);
        }

        let class = self.find_class_file(class_name)?;
        self.loaded_classes
            .insert(class_name.to_owned(), class.clone());
        let loaded_class = LoadedClass {
            class: class.clone(),
            is_initialised: false,
        };

        Ok(loaded_class)
    }

    fn find_class_file(&mut self, class_name: &str) -> JvmResult<Rc<ClassFile>> {
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
    ) -> JvmResult<Option<Rc<ClassFile>>> {
        match class_result {
            Ok(class) => Self::verify(class, class_name).map(|c| Option::Some(Rc::new(c))),
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
