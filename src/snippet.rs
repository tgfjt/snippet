use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snippet {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

#[cfg(test)]
impl Snippet {
    pub fn minimal(name: &str, desc: &str) -> Self {
        Snippet {
            name: name.to_string(),
            description: desc.to_string(),
            command: None,
            tags: Vec::new(),
            body: None,
        }
    }

    pub fn full(
        name: &str,
        desc: &str,
        command: Option<&str>,
        tags: &[&str],
        body: Option<&str>,
    ) -> Self {
        Snippet {
            name: name.to_string(),
            description: desc.to_string(),
            command: command.map(|s| s.to_string()),
            tags: tags.iter().map(|s| s.to_string()).collect(),
            body: body.map(|s| s.to_string()),
        }
    }
}
