// Imports
pub use color_eyre::eyre::{eyre, Result};

// Templates
pub mod template {
    use sailfish::TemplateOnce;

    #[derive(TemplateOnce)]
    #[template(path = "Cargo.toml")]
    pub struct CargoToml {
        pub name: String,
    }

    #[derive(TemplateOnce)]
    #[template(path = "lib.rs")]
    pub struct LibRs;
    
    #[derive(TemplateOnce)]
    #[template(path = "config.toml")]
    pub struct ConfigToml;
}

// Upload form
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub apikey: String,
    pub endpoint: String,
    pub authorization: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            apikey: String::from("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6Imh5amJvYmxramVldmt6YXFzeXhlIiwicm9sZSI6ImFub24iLCJpYXQiOjE2NTQwMTEyNTgsImV4cCI6MTk2OTU4NzI1OH0.L20s98fiTqfPWyTTSe-zjgoovQYhkJGKE7K8h9_-drY"),
            endpoint: String::from("https://hyjboblkjeevkzaqsyxe.supabase.co"),
            authorization: String::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Type {
    #[serde(rename = "WASM")]
    Wasm,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Data {
    node_id: String,
    version: String,
    display_name: String,
    description: String,
    width: usize,
    height: usize,
    #[serde(rename = "backgroundColor")]
    background_color: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Target {
    name: String,
    type_bounds: Vec<String>,
    required: bool,
    #[serde(rename = "defaultValue")]
    default_value: String,
    tooltip: String,
    passthrough: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Source {
    name: String,
    r#type: String,
    #[serde(rename = "defaultValue")]
    default_value: String,
    tooltip: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Format {
    r#type: Type,
    data: Data,
    targets: Vec<Target>,
    sources: Vec<Source>,
}

impl Format {
    pub fn new(
        name: String,
        version: String,
        description: String,
        inputs: Vec<(String, String)>,
        outputs: Vec<(String, String)>,
    ) -> Self {
        let targets = inputs
            .into_iter()
            .map(|(name, r#type)| Target {
                name,
                type_bounds: vec![r#type],
                required: true,
                default_value: String::from(""),
                tooltip: String::from(""),
                passthrough: false,
            })
            .collect::<Vec<_>>();

        let sources = outputs
            .into_iter()
            .map(|(name, r#type)| Source {
                name,
                r#type,
                default_value: String::from(""),
                tooltip: String::from(""),
            })
            .collect::<Vec<_>>();

        Self {
            r#type: Type::Wasm,
            data: Data {
                node_id: name.to_lowercase(),
                version,
                display_name: name,
                description,
                width: 150,
                height: 125
                    + [targets.len(), sources.len()]
                    .into_iter()
                    .max()
                    .unwrap_or(0)
                    * 50,
                background_color: String::from("#ffd9b3"),
            },
            targets,
            sources,
        }
    }

    pub fn parse(input: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(input)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Node {
    name: String,
    r#type: Type,
    sources: Vec<Source>,
    targets: Vec<Target>,
    data: Data,
}

impl Node {
    pub fn from_format(name: String, format: Format) -> Self {
        Self {
            name,
            r#type: format.r#type,
            sources: format.sources,
            targets: format.targets,
            data: format.data,
        }
    }
}

pub struct StorageClient {
    endpoint: String,
    authorization: String,
}

impl StorageClient {
    /// Creates new client. endpoint should be https://hyjboblkjeevkzaqsyxe.supabase.co
    pub fn new(endpoint: &str, authorization: &str) -> Self {
        Self {
            endpoint: format!("{endpoint}/storage/v1"),
            authorization: authorization.to_string(),
        }
    }

    /// Bucket to look for files, such as node-files
    pub fn from(&self, bucket: &str) -> StorageBuilder {
        StorageBuilder {
            config: &self,
            bucket: bucket.to_string(),
        }
    }
}

pub struct StorageBuilder<'a> {
    config: &'a StorageClient,
    bucket: String,
}

impl StorageBuilder<'_> {
    /// Upload file from path
    pub async fn upload(self, path: &str, bytes: Vec<u8>) -> Result<()> {
        let url = format!("{}/object/{}/{}", self.config.endpoint, self.bucket, path);
        let mime_type = mime_guess::from_path(path).first_or_octet_stream();
        let client = reqwest::Client::new();
        client.post(&url)
            .header("Authorization", &self.config.authorization)
            .header("Content-Type", mime_type.essence_str())
            .body(bytes)
            .send()
            .await?;
        Ok(())
    }
}

