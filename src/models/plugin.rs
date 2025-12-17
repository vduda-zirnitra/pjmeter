// Data models
#[derive(Debug, Clone)]
pub struct Plugin {
    pub _name: String,
    pub _version: String,
    pub jars: Vec<String>,
}
