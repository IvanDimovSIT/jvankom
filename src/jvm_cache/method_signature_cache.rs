use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MethodSignatureId {
    name_id: u32,
    descriptor_id: u32,
}

#[derive(Debug)]
pub struct MethodSignatureCache {
    method_names: HashMap<String, u32>,
    method_descriptors: HashMap<String, u32>,
}
impl MethodSignatureCache {
    pub fn new() -> Self {
        Self {
            method_names: HashMap::new(),
            method_descriptors: HashMap::new(),
        }
    }

    pub fn get_id(&mut self, name: &str, descriptor: &str) -> MethodSignatureId {
        let name_id = if let Some(id) = self.method_names.get(name) {
            *id
        } else {
            let id = self.method_names.len() as u32;
            self.method_names.insert(name.to_owned(), id);
            id
        };
        let descriptor_id = if let Some(id) = self.method_descriptors.get(descriptor) {
            *id
        } else {
            let id = self.method_descriptors.len() as u32;
            self.method_descriptors.insert(descriptor.to_owned(), id);
            id
        };

        MethodSignatureId {
            name_id,
            descriptor_id,
        }
    }
}
