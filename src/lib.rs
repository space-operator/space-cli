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

    #[derive(TemplateOnce)]
    #[template(path = "build.zig")]
    pub struct BuildZig {
        pub name: String,
    }

    #[derive(TemplateOnce)]
    #[template(path = "main.zig")]
    pub struct MainZig;
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Type {
    #[serde(rename = "WASM")]
    Wasm,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Data {
    pub node_id: String,
    pub version: String,
    pub display_name: String,
    pub description: String,
    pub width: usize,
    pub height: usize,
    #[serde(rename = "backgroundColor")]
    pub background_color: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Target {
    pub name: String,
    pub type_bounds: Vec<String>,
    pub required: bool,
    #[serde(rename = "defaultValue")]
    pub default_value: String,
    pub tooltip: String,
    pub passthrough: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Source {
    pub name: String,
    pub r#type: String,
    #[serde(rename = "defaultValue")]
    pub default_value: String,
    pub tooltip: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Format {
    pub r#type: Type,
    pub data: Data,
    pub targets: Vec<Target>,
    pub sources: Vec<Source>,
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
    pub name: String,
    pub r#type: Type,
    pub sources: Vec<Source>,
    pub targets: Vec<Target>,
    pub unique_node_id: String,
    pub data: Data,
    #[serde(rename = "isPublic")]
    pub is_public: bool,
    pub storage_path: String,
    pub source_code: String,
    #[serde(rename = "priceOneTime")]
    pub price_one_time: f64,
    #[serde(rename = "pricePerRun")]
    pub price_per_run: f64,
    pub license_type: String,
}

impl Node {
    pub fn new(
        name: String,
        storage_path: String,
        source_code: String,
        format: Format,
        is_public: bool,
        price_one_time: f64,
        price_per_run: f64,
        license_type: String,
    ) -> Self {
        let lowercase = name.to_lowercase();
        Self {
            name,
            r#type: format.r#type,
            sources: format.sources,
            targets: format.targets,
            unique_node_id: format!("{}.{}", lowercase, format.data.version),
            data: format.data,
            is_public,
            storage_path,
            source_code,
            price_one_time,
            price_per_run,
            license_type,
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
        client
            .post(&url)
            .header("Authorization", &self.config.authorization)
            .header("Content-Type", mime_type.essence_str())
            .body(bytes)
            .send()
            .await?;
        Ok(())
    }
}
