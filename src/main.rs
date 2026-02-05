//! BSA/BA2 Archive Tool
//!
//! A GUI + CLI application for packing and unpacking Bethesda BSA/BA2 archives
//! with support for multiple game formats.

mod archive;
mod gui;

use archive::{
    extract_archive_files_batch, list_archive_files, Ba2Builder, Ba2Format, BsaBuilder, GameVersion,
};
use gui::state::{setup_callbacks, AppState};
use gui::MainWindow;
use slint::ComponentHandle;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing_subscriber::EnvFilter;
use walkdir::WalkDir;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // No args (just the binary name) → launch GUI
    if args.len() < 2 {
        return run_gui();
    }

    // CLI mode
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .init();

    let command = args[1].as_str();
    match command {
        "unpack" | "extract" => cli_unpack(&args[2..]),
        "pack" => cli_pack(&args[2..]),
        "list" | "ls" => cli_list(&args[2..]),
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        other => {
            eprintln!("Unknown command: {}", other);
            print_help();
            std::process::exit(1);
        }
    }
}

fn run_gui() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let window = MainWindow::new()?;
    let state = Arc::new(Mutex::new(AppState::new()));
    setup_callbacks(&window, state);
    window.run()?;
    Ok(())
}

fn print_help() {
    eprintln!(
        "BSA/BA2 Archive Tool

USAGE:
    bsa-ba2-tool                              Launch GUI
    bsa-ba2-tool unpack <archive> [output]    Extract archive to folder
    bsa-ba2-tool pack <folder> <output> <game>  Pack folder into archive
    bsa-ba2-tool list <archive>               List files in archive

GAME VERSIONS:"
    );
    for v in GameVersion::all() {
        eprintln!("    {:<14} {}", v.cli_name(), v.display_name());
    }
    eprintln!(
        "
EXAMPLES:
    bsa-ba2-tool unpack Skyrim.bsa ./output
    bsa-ba2-tool pack ./my_mod my_mod.bsa skyrimse
    bsa-ba2-tool pack ./textures textures.ba2 fo4ng-v7
    bsa-ba2-tool list archive.ba2"
    );
}

fn cli_list(args: &[String]) -> anyhow::Result<()> {
    if args.is_empty() {
        eprintln!("Usage: bsa-ba2-tool list <archive>");
        std::process::exit(1);
    }

    let archive_path = Path::new(&args[0]);
    let files = list_archive_files(archive_path)?;

    for entry in &files {
        println!("{}", entry.path);
    }
    eprintln!("{} files", files.len());
    Ok(())
}

fn cli_unpack(args: &[String]) -> anyhow::Result<()> {
    if args.is_empty() {
        eprintln!("Usage: bsa-ba2-tool unpack <archive> [output_folder]");
        std::process::exit(1);
    }

    let archive_path = PathBuf::from(&args[0]);
    let output_folder = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        // Default: archive name without extension
        let stem = archive_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "output".to_string());
        archive_path.parent().unwrap_or(Path::new(".")).join(stem)
    };

    let files = list_archive_files(&archive_path)?;
    let total = files.len();
    eprintln!("Extracting {} files from {}", total, archive_path.display());

    std::fs::create_dir_all(&output_folder)?;

    let file_paths: Vec<String> = files.iter().map(|e| e.path.clone()).collect();
    let extracted = std::sync::atomic::AtomicUsize::new(0);
    let idx = std::sync::atomic::AtomicUsize::new(0);

    extract_archive_files_batch(&archive_path, &file_paths, |path, data| {
        let out_path = output_folder.join(path.replace('\\', "/"));
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&out_path, &data)?;
        extracted.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let current = idx.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;

        if current.is_multiple_of(500) || current == total {
            eprint!("\r  {}/{} files extracted", current, total);
        }
        Ok(())
    })?;

    eprintln!();
    eprintln!(
        "Done: {} of {} files extracted to {}",
        extracted.load(std::sync::atomic::Ordering::Relaxed),
        total,
        output_folder.display()
    );
    Ok(())
}

fn cli_pack(args: &[String]) -> anyhow::Result<()> {
    if args.len() < 3 {
        eprintln!("Usage: bsa-ba2-tool pack <folder> <output> <game>");
        eprintln!("Run 'bsa-ba2-tool help' for game version list");
        std::process::exit(1);
    }

    let source_folder = PathBuf::from(&args[0]);
    let output_path = PathBuf::from(&args[1]);
    let game_version = match GameVersion::from_cli_name(&args[2]) {
        Some(v) => v,
        None => {
            eprintln!("Unknown game version: {}", args[2]);
            eprintln!("Valid options:");
            for v in GameVersion::all() {
                eprintln!("  {:<14} {}", v.cli_name(), v.display_name());
            }
            std::process::exit(1);
        }
    };

    if game_version.is_tes3() {
        anyhow::bail!("Morrowind TES3 BSA writing is not supported");
    }

    // Collect files
    let mut file_paths: Vec<String> = Vec::new();
    for entry in WalkDir::new(&source_folder)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() {
            if let Ok(rel) = entry.path().strip_prefix(&source_folder) {
                file_paths.push(rel.to_string_lossy().to_string());
            }
        }
    }

    if file_paths.is_empty() {
        anyhow::bail!("No files found in {}", source_folder.display());
    }

    let total = file_paths.len();
    eprintln!(
        "Packing {} files as {} -> {}",
        total,
        game_version.display_name(),
        output_path.display()
    );

    if game_version.is_ba2() {
        let ba2_version = game_version.ba2_version().unwrap_or_default();
        let compression = game_version.ba2_compression();

        let name_lower = output_path
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let format = if name_lower.contains("textures") {
            Ba2Format::DX10
        } else {
            Ba2Format::General
        };

        let mut builder = Ba2Builder::new()
            .with_version(ba2_version)
            .with_compression(compression)
            .with_format(format);

        for (idx, rel_path) in file_paths.iter().enumerate() {
            let disk_path = source_folder.join(rel_path.replace('\\', "/"));
            let data = std::fs::read(&disk_path)?;
            builder.add_file(rel_path, data);

            if (idx + 1) % 100 == 0 || idx + 1 == total {
                eprint!("\r  Reading: {}/{}", idx + 1, total);
            }
        }
        eprintln!();

        eprintln!("  Building archive...");
        builder.build_with_progress(&output_path, |current, btotal, _| {
            if current % 100 == 0 || current == btotal {
                eprint!("\r  Compressing: {}/{}", current, btotal);
            }
        })?;
        eprintln!();
    } else {
        // BSA
        let bsa_version = game_version.bsa_version().unwrap();
        let compress = game_version.supports_compression();

        let mut builder = BsaBuilder::new()
            .with_version(bsa_version)
            .with_compression(compress);

        for (idx, rel_path) in file_paths.iter().enumerate() {
            let disk_path = source_folder.join(rel_path.replace('\\', "/"));
            let data = std::fs::read(&disk_path)?;
            builder.add_file(rel_path, data);

            if (idx + 1) % 100 == 0 || idx + 1 == total {
                eprint!("\r  Reading: {}/{}", idx + 1, total);
            }
        }
        eprintln!();

        eprintln!("  Building archive...");
        builder.build_with_progress(&output_path, |current, btotal, _| {
            if current % 100 == 0 || current == btotal {
                eprint!("\r  Compressing: {}/{}", current, btotal);
            }
        })?;
        eprintln!();
    }

    eprintln!(
        "Done: {} files packed into {}",
        total,
        output_path.display()
    );
    Ok(())
}
