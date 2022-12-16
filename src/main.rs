use clap::{Parser, Subcommand};
use dialoguer::{FuzzySelect, Input};
use indicatif::ProgressBar;
use platform_dirs::AppDirs;
use space::{eyre, Config, Format, Result, StorageClient};
use std::{fs::File, io::Write, path::PathBuf, time::Duration};
use titlecase::titlecase;
use uuid::Uuid;

#[derive(Parser)]
struct Args {
    /// Subcommand to run
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Authenticate and store locally
    Init,
    /// Upload WASM node to Space Operator
    Upload(Upload),
}

#[derive(Parser)]
struct Upload {
    /// Path to WASM binary
    wasm: PathBuf,
    /// Path to sourceccode
    source_code: PathBuf,
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

fn main() -> Result<()> {
    color_eyre::install()?;
    let args = Args::parse();

    // Parse arguments
    match args.command {
        Command::Init => {
            // Get defaults
            let defaults = read_config().unwrap_or_default();
            
            let endpoint = Input::<String>::new()
                .with_prompt("Supabase endpoint")
                .with_initial_text(defaults.endpoint)
                .interact_text()?;

            let api_key = Input::<String>::new()
                .with_prompt("API key")
                .with_initial_text(defaults.api_key)
                .interact_text()?;
            
            // Create config file
            let config_file = config_path()?;
            let message = format!("Wrote settings to {}", config_file.display());

            // Serialize to toml
            let mut file = File::create(config_file)?;
            let config = Config::new(endpoint, api_key);
            let toml = toml::to_string(&config)?;

            // Write to file
            file.write_all(toml.as_bytes())?;
            println!("{message}");
        }
        Command::Upload(upload) => {
            // Get config
            let config = read_config()?;
            let client = StorageClient::new(&config.endpoint, &config.api_key);

            // Verify that web assembly exists
            if !upload.wasm.exists() {
                return Err(eyre!("{} doesn't exist", upload.wasm.display()));
            }

            // Verify that source code exists
            if !upload.source_code.exists() {
                return Err(eyre!("{} doesn't exist", upload.source_code.display()));
            }

            // Start dialogue
            let suggested_name = upload
                .wasm
                .file_stem()
                .and_then(|it| it.to_str())
                .unwrap_or_default();

            let name = Input::<String>::new()
                .with_prompt("Name")
                .with_initial_text(titlecase(suggested_name))
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

            let format = Format::new(name.clone(), version.clone(), description, inputs, outputs);
            let json = serde_json::to_string_pretty(&format)?;

            // Unique identifier
            let base_path = Uuid::new_v4();

            // Upload the files
            let spinner =
                ProgressBar::new_spinner().with_message(format!("Uploading {name}@{version}..."));
            spinner.enable_steady_tick(Duration::from_millis(10));

            // Web assembly
            let wasm_name = upload.wasm.display();
            let bytes = std::fs::read(&upload.wasm)?;
            let path = format!("{base_path}/{wasm_name}");
            client.from("node-files").upload(&path, &bytes)?;

            // Source code
            let source_code_name = upload.source_code.display();
            let bytes = std::fs::read(&upload.source_code)?;
            let path = format!("{base_path}/{source_code_name}");
            client.from("node-files").upload(&path, &bytes)?;

            // JSON
            let path = format!("{base_path}/space.json");
            client.from("node-files").upload(&path, json.as_bytes())?;

            spinner.finish_and_clear();
            println!("Finished uploading {name}@{version}!");
        }
    }

    // Return success
    Ok(())
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
            .report(false)
            .interact()?;

        values.push((value, items[r#type].to_string()));
    }
    Ok(values)
}
