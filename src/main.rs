use clap::{Parser, Subcommand};
use dialoguer::{FuzzySelect, Input};
use glob::glob;
use indicatif::ProgressBar;
use platform_dirs::AppDirs;
use postgrest::Postgrest;
use sailfish::TemplateOnce;
use space::{eyre, template, Config, Format, Language, Node, Result, StorageClient};
use std::{borrow::Cow, fs::File, io::Write, path::PathBuf, time::Duration};
use uuid::Uuid;

#[derive(Parser)]
struct Args {
    /// Subcommand to run
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Login by store token locally
    Login,
    /// Create a new WASM project
    New(New),
    /// Upload WASM project to Space Operator
    Upload,
    /// Generate JSON from dialogue
    Generate,
    /// Manually upload WASM, source code and json to Space Operator
    Manual(Manual),
}

#[derive(Parser)]
struct Manual {
    /// Path to WASM binary
    wasm: PathBuf,
    /// Path to node declaration
    json: PathBuf,
    /// Path to source code
    source_code: PathBuf,
}

#[derive(Parser)]
struct New {
    /// Project name
    name: String,
}

fn config_path() -> Result<PathBuf> {
    let app_dirs = AppDirs::new(Some("space"), false).ok_or(eyre!("Config location is invalid"))?;
    std::fs::create_dir_all(&app_dirs.config_dir)?;
    Ok(app_dirs.config_dir.join("space.toml"))
}

fn read_config() -> Result<Config> {
    let config_file = config_path()?;
    let raw = std::fs::read_to_string(config_file)?;
    Ok(toml::from_str(&raw)?)
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let args = Args::parse();

    // Parse arguments
    match args.command {
        Command::Login => {
            // Get defaults
            let defaults = read_config().unwrap_or_default();

            let authorization = Input::<String>::new()
                .with_prompt("Authorization token")
                .report(false)
                .interact_text()?;

            // Create config file
            let config_file = config_path()?;
            let message = format!("Wrote settings to {}", config_file.display());

            // Serialize to toml
            let mut file = File::create(config_file)?;
            let config = Config {
                apikey: defaults.apikey,
                endpoint: defaults.endpoint,
                authorization,
            };
            let toml = toml::to_string(&config)?;

            // Write to file
            file.write_all(toml.as_bytes())?;
            println!("{message}");
        }
        Command::New(New { name }) => {
            // Ask for language
            let languages = vec!["rust", "zig"];

            let index = FuzzySelect::new()
                .items(&languages)
                .with_prompt("Language")
                .default(0)
                .report(false)
                .interact()?;

            match languages[index] {
                "rust" => {
                    // Create folders
                    std::fs::create_dir_all(format!("{name}/src"))?;
                    std::fs::create_dir_all(format!("{name}/.cargo"))?;

                    // Create Cargo.toml
                    let metadata = template::CargoToml { name: name.clone() }.render_once()?;
                    std::fs::write(format!("{name}/Cargo.toml"), metadata)?;

                    // Create lib.rs
                    let main = template::LibRs.render_once()?;
                    std::fs::write(format!("{name}/src/lib.rs"), main)?;

                    // Create config.toml
                    let config = template::ConfigToml.render_once()?;
                    std::fs::write(format!("{name}/.cargo/config.toml"), config)?;
                }
                "zig" => {
                    // Create folders
                    std::fs::create_dir_all(format!("{name}/src"))?;

                    // Create main.zig
                    let main = template::MainZig.render_once()?;
                    std::fs::write(format!("{name}/src/main.zig"), main)?;

                    // Create build.zig
                    let build = template::BuildZig { name: name.clone() }.render_once()?;
                    std::fs::write(format!("{name}/build.zig"), build)?;
                }
                _ => return Err(eyre!("Invalid language chosen")),
            }

            println!("Created new project `{name}`");
        }
        Command::Upload => {
            // Find root config file then change it
            let directory = find_root(std::env::current_dir()?)?;
            std::env::set_current_dir(directory)?;
            let language = find_language(std::env::current_dir()?)?;

            // Upload based on language
            match language {
                Language::Zig => {
                    // Build project in release mode
                    duct::cmd!("zig", "build").run()?;

                    // Find the files then upload
                    let wasm = glob("zig-out/lib/*.wasm")?
                        .next()
                        .ok_or(eyre!("WASM not found"))??;
                    let source_code = PathBuf::from("src/main.zig");
                    upload(wasm, source_code, None).await?;
                }
                Language::Rust => {
                    // Build project in release mode
                    duct::cmd!("cargo", "build", "--release", "--target", "wasm32-wasi").run()?;

                    // Find the files then upload
                    let wasm = glob("target/wasm32-wasi/release/*.wasm")?
                        .next()
                        .ok_or(eyre!("WASM not found"))??;
                    let source_code = PathBuf::from("src/lib.rs");
                    upload(wasm, source_code, None).await?;
                }
            };
        }
        Command::Manual(Manual { wasm, source_code, json }) => upload(wasm, source_code, Some(json)).await?,
        Command::Generate => {
            let format = read_format(None)?;
            let json = serde_json::to_string_pretty(&format)?;
            println!("{json}");
        }
    }

    // Return success
    Ok(())
}

fn find_root(mut current: PathBuf) -> Result<PathBuf> {
    let candidates = ["Cargo.toml", "build.zig"];
    let file_exists = std::fs::read_dir(&current)?.any(|path| match path {
        Ok(file) => candidates
            .into_iter()
            .any(|it| file.file_name().to_string_lossy() == it),
        Err(_) => false,
    });

    match file_exists {
        true => Ok(current),
        false => {
            if current == PathBuf::from("/") {
                return Err(eyre!("Project root not found"));
            }
            current.pop();
            find_root(current)
        }
    }
}

fn find_language(current: PathBuf) -> Result<Language> {
    for entry in std::fs::read_dir(&current)? {
        match entry?.file_name().to_string_lossy() {
            Cow::Borrowed("Cargo.toml") => return Ok(Language::Rust),
            Cow::Borrowed("build.zig") => return Ok(Language::Zig),
            _ => continue,
        }
    }
    Err(eyre!("Language not found"))
}

fn titlecase(input: &str) -> String {
    let mut chars = input.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c
            .to_uppercase()
            .chain(chars.flat_map(|t| t.to_lowercase()))
            .collect(),
    }
}

fn read_list(prefix: &str) -> Result<Vec<(String, String)>> {
    let items = vec![
        "bool",
        "u8",
        "u16",
        "u32",
        "u64",
        "u128",
        "i8",
        "i16",
        "i32",
        "i64",
        "f32",
        "f64",
        "pubkey",
        "keypair",
        "signature",
        "string",
        "array",
        "object",
        "json",
        "file",
    ];
    let mut values = Vec::new();
    loop {
        if let Some((name, r#type)) = values.last() {
            println!("{}: {} -> {}", titlecase(prefix), name, r#type);
        }

        let value = Input::<String>::new()
            .with_prompt(format!("Name of {prefix}"))
            .allow_empty(true)
            .report(false)
            .interact_text()?;

        if value.is_empty() {
            break;
        }

        let r#type = FuzzySelect::new()
            .items(&items)
            .with_prompt(format!("Type for {value}"))
            .default(0)
            .report(false)
            .interact()?;

        values.push((value, items[r#type].to_string()));
    }
    Ok(values)
}

fn read_format(wasm: Option<&PathBuf>) -> Result<Format> {
    // Start dialogue
    let suggested_name = match wasm {
        Some(path) => path
            .file_stem()
            .and_then(|it| it.to_str())
            .unwrap_or_default()
            .to_string(),
        None => String::new(),
    };

    let name = Input::<String>::new()
        .with_prompt("Name")
        .with_initial_text(titlecase(&suggested_name))
        .interact_text()?;

    let version = Input::<String>::new()
        .with_prompt("Version")
        .with_initial_text("0.1")
        .interact_text()?;

    let description = Input::<String>::new()
        .with_prompt("Description")
        .interact_text()?;

    let inputs = read_list("input")?;
    let outputs = read_list("output")?;

    Ok(Format::new(
        name.clone(),
        version.clone(),
        description,
        inputs,
        outputs,
    ))
}

async fn upload(wasm: PathBuf, source_code: PathBuf, json: Option<PathBuf>) -> Result<()> {
    // Get config
    let config = read_config()?;
    let client = StorageClient::new(&config.endpoint, &config.authorization);

    // Verify that web assembly exists
    if !wasm.exists() {
        return Err(eyre!("{} doesn't exist", wasm.display()));
    }

    // Verify that source code exists
    if !source_code.exists() {
        return Err(eyre!("{} doesn't exist", source_code.display()));
    }

    // Unique identifier
    let base_path = Uuid::new_v4();

    // Get json from dialogue or file
    let (format, json) = match json {
        Some(path) => {
            let json = std::fs::read_to_string(path)?;
            let format: Format = serde_json::from_str(&json)?;
            (format, json)
        },
        None => {
            let format = read_format(Some(&wasm))?;
            let json = serde_json::to_string_pretty(&format)?;
            (format, json)
        },
    };

    // Public or private
    let booleans = vec!["true", "false"];

    let index = FuzzySelect::new()
        .items(&booleans)
        .with_prompt("Public")
        .default(0)
        .report(false)
        .interact()?;
    let is_public = booleans[index].parse::<bool>()?;
    println!("Public: {is_public}");

    // License
    let licenses = vec!["MIT", "Apache 2.0"];

    let raw_licenses = vec!["MIT", "Apache"];

    let index = FuzzySelect::new()
        .items(&licenses)
        .with_prompt("License")
        .default(0)
        .report(false)
        .interact()?;
    let license = raw_licenses[index].to_string();
    println!("License: {license}");

    // One-time payment
    let price_one_time = Input::<f64>::new()
        .with_prompt("One-time payment")
        .with_initial_text("0")
        .interact_text()?;

    // Price per run
    let price_per_run = Input::<f64>::new()
        .with_prompt("Price per run")
        .with_initial_text("0")
        .interact_text()?;

    // Upload the files
    let spinner = ProgressBar::new_spinner().with_message(format!(
        "Uploading {}@{}...",
        format.data.display_name, format.data.version
    ));
    spinner.enable_steady_tick(Duration::from_millis(10));

    // Web assembly
    let wasm_name = wasm
        .file_name()
        .and_then(|it| it.to_str())
        .ok_or(eyre!("Invalid WASM path"))?;
    let bytes = std::fs::read(&wasm)?;
    let storage_path = format!("{base_path}/{wasm_name}");
    client
        .from("node-files")
        .upload(&storage_path, bytes)
        .await?;

    // Source code
    let source_code_name = source_code
        .file_name()
        .and_then(|it| it.to_str())
        .ok_or(eyre!("Invalid source code path"))?;
    let bytes = std::fs::read(&source_code)?;
    let source_code = format!("{base_path}/{source_code_name}");
    client
        .from("node-files")
        .upload(&source_code, bytes)
        .await?;

    // JSON
    let path = format!(
        "{base_path}/{}.json",
        format.data.display_name.to_lowercase().replace(" ", "_")
    );
    client
        .from("node-files")
        .upload(&path, json.into_bytes())
        .await?;

    // Insert into database
    let client = Postgrest::new(format!("{}/rest/v1", config.endpoint))
        .insert_header("apikey", config.apikey)
        .insert_header("authorization", config.authorization);
    let node = Node::new(
        format.data.display_name.clone(),
        storage_path,
        source_code,
        format.clone(),
        is_public,
        price_one_time,
        price_per_run,
        license,
    );
    client
        .from("nodes")
        .insert(serde_json::to_string(&node)?)
        .execute()
        .await?;

    spinner.finish_and_clear();
    println!(
        "Finished uploading {}@{}!",
        format.data.display_name, format.data.version
    );

    // Open with browser
    open::that(format!(
        "https://spaceoperator.com/dashboard/nodes/{}",
        node.unique_node_id
    ))?;

    Ok(())
}
