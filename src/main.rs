use clap::{Parser, Subcommand};
use dialoguer::{FuzzySelect, Input};
use indicatif::ProgressBar;
use platform_dirs::AppDirs;
use sailfish::TemplateOnce;
use space::{eyre, template, Config, Format, Result, StorageClient};
use std::{fs::File, io::Write, path::PathBuf, time::Duration};
use uuid::Uuid;
use glob::glob;

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
    /// Create a new WASM project
    New(New),
    /// Upload project
    Upload,
    /// Manually deploy WASM and source code to Space Operator
    Deploy(Deploy),
}

#[derive(Parser)]
struct Deploy {
    /// Path to WASM binary
    wasm: PathBuf,
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
        Command::Init => {
            // Get defaults
            let defaults = read_config().unwrap_or_default();

            let endpoint = Input::<String>::new()
                .with_prompt("Supabase")
                .with_initial_text(defaults.endpoint)
                .interact_text()?;

            let authorization = Input::<String>::new()
                .with_prompt("Authorization")
                .with_initial_text(defaults.authorization)
                .interact_text()?;

            // Create config file
            let config_file = config_path()?;
            let message = format!("Wrote settings to {}", config_file.display());

            // Serialize to toml
            let mut file = File::create(config_file)?;
            let config = Config {
                endpoint,
                authorization,
            };
            let toml = toml::to_string(&config)?;

            // Write to file
            file.write_all(toml.as_bytes())?;
            println!("{message}");
        }
        Command::New(New { name }) => {
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
            
            println!("Created new project `{name}`");
        }
        Command::Upload => {
            // Find root with Cargo.toml then change it
            let directory = find_root(std::env::current_dir()?)?;
            std::env::set_current_dir(directory)?;

            // Build project in release mode
            duct::cmd!("cargo", "build", "--release").run()?;

            // Find the files then upload
            let wasm = glob("target/wasm32-wasi/release/*.wasm")?.next().ok_or(eyre!("WASM not found"))??;
            let source_code = PathBuf::from("src/lib.rs");
            upload(wasm, source_code).await?;
        }
        Command::Deploy(Deploy { wasm, source_code }) => upload(wasm, source_code).await?,
    }

    // Return success
    Ok(())
}

fn titlecase(input: &str) -> String {
    let mut chars = input.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().chain(chars.flat_map(|t| t.to_lowercase())).collect(),
    }
}

fn find_root(mut current: PathBuf) -> Result<PathBuf> {
    let file_exists = std::fs::read_dir(&current)?.any(|path| match path {
        Ok(file) => file.file_name().to_string_lossy() == "Cargo.toml",
        Err(_) => false,
    });
    
    match file_exists {
        true => Ok(current),
        false => {
            current.pop();
            find_root(current)
        },
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
            .report(false)
            .interact()?;

        values.push((value, items[r#type].to_string()));
    }
    Ok(values)
}

async fn upload(wasm: PathBuf, source_code: PathBuf) -> Result<()> {
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

    // Start dialogue
    let suggested_name = wasm
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
    let wasm_name = wasm.display();
    let bytes = std::fs::read(&wasm)?;
    let path = format!("{base_path}/{wasm_name}");
    client.from("node-files").upload(&path, bytes).await?;

    // Source code
    let source_code_name = source_code.display();
    let bytes = std::fs::read(&source_code)?;
    let path = format!("{base_path}/{source_code_name}");
    client.from("node-files").upload(&path, bytes).await?;

    // JSON
    let path = format!("{base_path}/space.json");
    client.from("node-files").upload(&path, json.into_bytes()).await?;

    spinner.finish_and_clear();
    println!("Finished uploading {name}@{version}!");
    
    Ok(())
}
