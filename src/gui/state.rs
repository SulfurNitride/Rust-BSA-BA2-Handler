//! Application state management

use crate::archive::{
    detect_game_version, extract_archive_files_batch, list_archive_files, ArchiveFileEntry,
    Ba2Builder, Ba2Format, BsaBuilder, GameVersion,
};
use crate::gui::{MainWindow, TreeNode};
use anyhow::{bail, Result};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel, Weak};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tracing::error;
use walkdir::WalkDir;

/// Thread-safe state handle
pub type StateHandle = Arc<Mutex<AppState>>;

/// Internal tree node for building hierarchy
#[derive(Debug, Clone)]
pub(crate) struct InternalNode {
    path: String,
    name: String,
    depth: i32,
    is_folder: bool,
    expanded: bool,
    selected: bool,
    partially_selected: bool,
    children: Vec<usize>, // Indices of children in the flat list
    parent: Option<usize>,
}

/// Application state
pub struct AppState {
    /// Archive path (if loaded)
    pub archive_path: Option<PathBuf>,
    /// Raw archive entries
    pub entries: Vec<ArchiveFileEntry>,
    /// Hierarchical tree nodes
    pub tree: Vec<InternalNode>,
    /// Search filter
    pub search_filter: String,
    /// Cancellation flag
    pub cancelled: Arc<AtomicBool>,
    /// Detected game version
    pub game_version: Option<GameVersion>,
    /// True when a folder is loaded for packing (vs an archive for extraction)
    pub pack_mode: bool,
    /// The folder being packed
    pub source_folder: Option<PathBuf>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            archive_path: None,
            entries: Vec::new(),
            tree: Vec::new(),
            search_filter: String::new(),
            cancelled: Arc::new(AtomicBool::new(false)),
            game_version: None,
            pack_mode: false,
            source_folder: None,
        }
    }

    /// Load an archive and build tree
    pub fn load_archive(&mut self, path: &Path) -> Result<()> {
        self.archive_path = Some(path.to_path_buf());
        self.entries = list_archive_files(path)?;
        self.game_version = detect_game_version(path);
        self.pack_mode = false;
        self.source_folder = None;

        let root_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Archive".to_string());
        let paths: Vec<String> = self.entries.iter().map(|e| e.path.clone()).collect();
        self.build_tree_from_paths(paths, root_name);
        Ok(())
    }

    /// Load a folder for packing and build tree
    pub fn load_folder(&mut self, path: &Path) -> Result<()> {
        self.pack_mode = true;
        self.source_folder = Some(path.to_path_buf());
        self.archive_path = None;
        self.entries.clear();

        let mut paths = Vec::new();
        for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                if let Ok(rel) = entry.path().strip_prefix(path) {
                    paths.push(rel.to_string_lossy().to_string());
                }
            }
        }

        if paths.is_empty() {
            bail!("Folder is empty: {}", path.display());
        }

        let root_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Folder".to_string());
        self.build_tree_from_paths(paths, root_name);
        Ok(())
    }

    /// Build hierarchical tree from a list of file paths
    fn build_tree_from_paths(&mut self, paths: Vec<String>, root_name: String) {
        self.tree.clear();

        let mut children_map: HashMap<String, Vec<(String, String, bool)>> = HashMap::new();

        let mut folders: HashSet<String> = HashSet::new();
        for file_path in &paths {
            let path = file_path.replace('/', "\\");
            let parts: Vec<&str> = path.split('\\').collect();
            for i in 0..parts.len() - 1 {
                let folder_path = parts[..=i].join("\\");
                folders.insert(folder_path);
            }
        }

        for folder in &folders {
            let parts: Vec<&str> = folder.split('\\').collect();
            let name = parts.last().unwrap_or(&"").to_string();
            let parent_path = if parts.len() > 1 {
                parts[..parts.len() - 1].join("\\")
            } else {
                String::new()
            };
            children_map
                .entry(parent_path)
                .or_default()
                .push((name, folder.clone(), true));
        }

        for file_path in &paths {
            let path = file_path.replace('/', "\\");
            let parts: Vec<&str> = path.split('\\').collect();
            let name = parts.last().unwrap_or(&"").to_string();
            let parent_path = if parts.len() > 1 {
                parts[..parts.len() - 1].join("\\")
            } else {
                String::new()
            };
            children_map
                .entry(parent_path)
                .or_default()
                .push((name, path, false));
        }

        for children in children_map.values_mut() {
            children.sort_by(|a, b| match (a.2, b.2) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.0.to_lowercase().cmp(&b.0.to_lowercase()),
            });
        }

        self.tree.push(InternalNode {
            path: String::new(),
            name: root_name,
            depth: 0,
            is_folder: true,
            expanded: true,
            selected: true,
            partially_selected: false,
            children: Vec::new(),
            parent: None,
        });

        self.build_tree_dfs(&children_map, "", 0);
    }

    /// Recursively add children of `parent_path` in depth-first order
    fn build_tree_dfs(
        &mut self,
        children_map: &HashMap<String, Vec<(String, String, bool)>>,
        parent_path: &str,
        parent_idx: usize,
    ) {
        let children = match children_map.get(parent_path) {
            Some(c) => c.clone(),
            None => return,
        };

        for (name, full_path, is_folder) in children {
            let depth = full_path.split('\\').count() as i32;
            let idx = self.tree.len();

            self.tree.push(InternalNode {
                path: full_path.clone(),
                name,
                depth,
                is_folder,
                expanded: true,
                selected: true,
                partially_selected: false,
                children: Vec::new(),
                parent: Some(parent_idx),
            });

            self.tree[parent_idx].children.push(idx);

            // Recurse into folders
            if is_folder {
                self.build_tree_dfs(children_map, &full_path, idx);
            }
        }
    }

    /// Toggle folder expansion
    pub fn toggle_expand(&mut self, index: usize) {
        if index < self.tree.len() && self.tree[index].is_folder {
            self.tree[index].expanded = !self.tree[index].expanded;
        }
    }

    /// Toggle selection (propagate to children/parents)
    pub fn toggle_select(&mut self, index: usize) {
        if index < self.tree.len() {
            let new_state = !self.tree[index].selected;
            self.set_selected_recursive(index, new_state);
            self.update_parent_selection(index);
        }
    }

    fn set_selected_recursive(&mut self, index: usize, selected: bool) {
        self.tree[index].selected = selected;
        self.tree[index].partially_selected = false;
        let children = self.tree[index].children.clone();
        for child_idx in children {
            self.set_selected_recursive(child_idx, selected);
        }
    }

    fn update_parent_selection(&mut self, index: usize) {
        if let Some(parent_idx) = self.tree[index].parent {
            let children = &self.tree[parent_idx].children;
            let all_selected = children
                .iter()
                .all(|&i| self.tree[i].selected && !self.tree[i].partially_selected);
            let any_selected = children
                .iter()
                .any(|&i| self.tree[i].selected || self.tree[i].partially_selected);

            self.tree[parent_idx].selected = all_selected;
            self.tree[parent_idx].partially_selected = any_selected && !all_selected;

            // Recurse up
            self.update_parent_selection(parent_idx);
        }
    }

    /// Select all
    pub fn select_all(&mut self) {
        for node in &mut self.tree {
            node.selected = true;
            node.partially_selected = false;
        }
    }

    /// Select none
    pub fn select_none(&mut self) {
        for node in &mut self.tree {
            node.selected = false;
            node.partially_selected = false;
        }
    }

    /// Set search filter
    pub fn set_search(&mut self, filter: String) {
        self.search_filter = filter;
    }

    /// Check if a path matches the search filter (with wildcard support)
    fn matches_search(&self, node: &InternalNode) -> bool {
        if self.search_filter.is_empty() {
            return true;
        }

        let search = self.search_filter.to_lowercase();
        let text = node.path.to_lowercase();

        // Simple wildcard matching (* = any characters)
        if search.contains('*') {
            let parts: Vec<&str> = search.split('*').collect();
            let mut pos = 0;

            for (i, part) in parts.iter().enumerate() {
                if part.is_empty() {
                    continue;
                }

                if let Some(found) = text[pos..].find(part) {
                    if i == 0 && found != 0 {
                        // First part must match at start if no leading *
                        return false;
                    }
                    pos += found + part.len();
                } else {
                    return false;
                }
            }

            // If no trailing *, must match at end
            if !search.ends_with('*') && pos != text.len() {
                return false;
            }

            true
        } else {
            text.contains(&search)
        }
    }

    /// Check if a node or any of its descendants match the search
    fn node_or_descendants_match(&self, index: usize) -> bool {
        let node = &self.tree[index];

        if self.matches_search(node) {
            return true;
        }

        // Check children
        for &child_idx in &node.children {
            if self.node_or_descendants_match(child_idx) {
                return true;
            }
        }

        false
    }

    /// Check if node is visible (parent expanded + matches search)
    fn is_visible(&self, index: usize) -> bool {
        let node = &self.tree[index];

        // Check search filter - node or descendants must match
        if !self.search_filter.is_empty() && !self.node_or_descendants_match(index) {
            return false;
        }

        // Check parent expansion
        if let Some(parent_idx) = node.parent {
            if !self.tree[parent_idx].expanded {
                return false;
            }
            // Recurse to check all ancestors
            return self.is_visible(parent_idx);
        }

        true
    }

    /// Convert to Slint model — only includes visible nodes to avoid
    /// sending tens of thousands of hidden elements to Slint's layout engine.
    pub fn to_slint_model(&self) -> ModelRc<TreeNode> {
        let nodes: Vec<TreeNode> = self
            .tree
            .iter()
            .enumerate()
            .filter(|(idx, _)| self.is_visible(*idx))
            .map(|(idx, node)| TreeNode {
                path: SharedString::from(&node.path),
                name: SharedString::from(&node.name),
                depth: node.depth,
                is_folder: node.is_folder,
                expanded: node.expanded,
                selected: node.selected,
                partially_selected: node.partially_selected,
                visible: true,
                has_children: !node.children.is_empty(),
                index: idx as i32,
            })
            .collect();

        ModelRc::new(VecModel::from(nodes))
    }

    /// Get selected file paths for extraction
    pub fn get_selected_files(&self) -> Vec<String> {
        self.tree
            .iter()
            .filter(|n| !n.is_folder && n.selected)
            .map(|n| n.path.clone())
            .collect()
    }

    /// Count selected files
    pub fn selected_count(&self) -> usize {
        self.tree
            .iter()
            .filter(|n| !n.is_folder && n.selected)
            .count()
    }

    /// Total file count
    pub fn total_count(&self) -> usize {
        self.tree.iter().filter(|n| !n.is_folder).count()
    }

    /// Reset cancel flag
    pub fn reset_cancel(&self) {
        self.cancelled.store(false, Ordering::SeqCst);
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Set up all UI callbacks
pub fn setup_callbacks(window: &MainWindow, state: StateHandle) {
    // Populate game versions in the ComboBox
    let names: Vec<SharedString> = GameVersion::all()
        .iter()
        .map(|v| SharedString::from(v.display_name()))
        .collect();
    window.set_game_versions(ModelRc::new(VecModel::from(names)));
    window.set_selected_game_version(GameVersion::default().index());

    setup_open_file(window, state.clone());
    setup_open_folder(window, state.clone());
    setup_extract(window, state.clone());
    setup_pack(window, state.clone());
    setup_select_all(window, state.clone());
    setup_select_none(window, state.clone());
    setup_search(window, state.clone());
    setup_toggle_expand(window, state.clone());
    setup_toggle_select(window, state);
}

fn setup_open_file(window: &MainWindow, state: StateHandle) {
    let window_weak = window.as_weak();
    window.on_open_file(move || {
        let window = window_weak.unwrap();

        let path = rfd::FileDialog::new()
            .add_filter("Archives", &["bsa", "ba2"])
            .add_filter("BSA Files", &["bsa"])
            .add_filter("BA2 Files", &["ba2"])
            .pick_file();

        if let Some(path) = path {
            window.set_is_processing(true);
            window.set_status_text(SharedString::from(format!(
                "Loading {}...",
                path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            )));

            let window_weak_thread = window.as_weak();
            let state = state.clone();

            std::thread::spawn(move || {
                let mut new_state = AppState::new();
                let result = new_state.load_archive(&path);

                let _ = window_weak_thread.upgrade_in_event_loop(move |w: MainWindow| {
                    match result {
                        Ok(()) => {
                            let title = format!(
                                "{} - BSA/BA2 Tool",
                                path.file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_default()
                            );
                            let total = new_state.total_count();
                            let selected = new_state.selected_count();
                            let model = new_state.to_slint_model();

                            *state.lock().unwrap() = new_state;

                            w.set_window_title(SharedString::from(&title));
                            w.set_pack_mode(false);
                            w.set_tree_nodes(model);
                            w.set_status_text(SharedString::from(format!(
                                "{} files, {} selected",
                                total, selected
                            )));
                        }
                        Err(e) => {
                            error!("Failed to load archive: {}", e);
                            w.set_status_text(SharedString::from(format!("Error: {}", e)));
                        }
                    }
                    w.set_is_processing(false);
                });
            });
        }
    });
}

fn setup_open_folder(window: &MainWindow, state: StateHandle) {
    let window_weak = window.as_weak();
    window.on_open_folder(move || {
        let window = window_weak.unwrap();

        let path = rfd::FileDialog::new().pick_folder();

        if let Some(path) = path {
            window.set_is_processing(true);
            window.set_status_text(SharedString::from(format!(
                "Scanning {}...",
                path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            )));

            let window_weak_thread = window.as_weak();
            let state = state.clone();

            std::thread::spawn(move || {
                let mut new_state = AppState::new();
                let result = new_state.load_folder(&path);

                let _ = window_weak_thread.upgrade_in_event_loop(move |w: MainWindow| {
                    match result {
                        Ok(()) => {
                            let title = format!(
                                "{} (Pack) - BSA/BA2 Tool",
                                path.file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_default()
                            );
                            let total = new_state.total_count();
                            let selected = new_state.selected_count();
                            let model = new_state.to_slint_model();

                            *state.lock().unwrap() = new_state;

                            w.set_window_title(SharedString::from(&title));
                            w.set_pack_mode(true);
                            w.set_tree_nodes(model);
                            w.set_status_text(SharedString::from(format!(
                                "{} files, {} selected — choose game version and click Pack",
                                total, selected
                            )));
                        }
                        Err(e) => {
                            error!("Failed to load folder: {}", e);
                            w.set_status_text(SharedString::from(format!("Error: {}", e)));
                        }
                    }
                    w.set_is_processing(false);
                });
            });
        }
    });
}

fn setup_extract(window: &MainWindow, state: StateHandle) {
    let window_weak = window.as_weak();
    window.on_extract(move || {
        let window = window_weak.unwrap();

        let state_ref = state.lock().unwrap();
        let archive_path = match &state_ref.archive_path {
            Some(p) => p.clone(),
            None => {
                window.set_status_text(SharedString::from("No archive loaded"));
                return;
            }
        };

        let selected_files = state_ref.get_selected_files();
        if selected_files.is_empty() {
            window.set_status_text(SharedString::from("No files selected"));
            return;
        }

        let cancelled = state_ref.cancelled.clone();
        drop(state_ref);

        // Ask for output folder
        let output_folder = rfd::FileDialog::new().pick_folder();
        let output_folder = match output_folder {
            Some(f) => f,
            None => return,
        };

        state.lock().unwrap().reset_cancel();
        window.set_is_processing(true);
        window.set_progress(0.0);

        let window_weak_thread = window.as_weak();
        let files = selected_files.clone();

        std::thread::spawn(move || {
            let total = files.len();
            let extracted = std::sync::atomic::AtomicUsize::new(0);
            let idx = std::sync::atomic::AtomicUsize::new(0);

            let result = extract_archive_files_batch(&archive_path, &files, |path, data| {
                if cancelled.load(Ordering::SeqCst) {
                    anyhow::bail!("Cancelled");
                }

                let output_path = output_folder.join(path.replace('\\', "/"));
                if let Some(parent) = output_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                if fs::write(&output_path, &data).is_ok() {
                    extracted.fetch_add(1, Ordering::Relaxed);
                }

                let current = idx.fetch_add(1, Ordering::Relaxed) + 1;
                // Only update UI every 500 files to avoid flooding the event loop
                if current.is_multiple_of(500) || current == total {
                    let progress = current as f32 / total as f32;
                    let _ = window_weak_thread.upgrade_in_event_loop(move |w: MainWindow| {
                        w.set_progress(progress);
                        w.set_status_text(SharedString::from(format!(
                            "Extracting: {}/{}",
                            current, total
                        )));
                    });
                }

                Ok(())
            });

            let extracted = extracted.load(Ordering::Relaxed);
            let _ = window_weak_thread.upgrade_in_event_loop(move |w: MainWindow| {
                w.set_is_processing(false);
                w.set_progress(1.0);
                match result {
                    Ok(_) => {
                        w.set_status_text(SharedString::from(format!(
                            "Extracted {} of {} files",
                            extracted, total
                        )));
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        if msg == "Cancelled" {
                            w.set_status_text(SharedString::from(format!(
                                "Cancelled — extracted {} of {} files",
                                extracted, total
                            )));
                        } else {
                            w.set_status_text(SharedString::from(format!(
                                "Error: {} (extracted {} files)",
                                msg, extracted
                            )));
                        }
                    }
                }
            });
        });
    });
}

fn setup_pack(window: &MainWindow, state: StateHandle) {
    let window_weak = window.as_weak();
    window.on_pack(move || {
        let window = window_weak.unwrap();

        let state_ref = state.lock().unwrap();
        let source_folder = match &state_ref.source_folder {
            Some(p) => p.clone(),
            None => {
                window.set_status_text(SharedString::from("No folder loaded"));
                return;
            }
        };

        let selected_files = state_ref.get_selected_files();
        if selected_files.is_empty() {
            window.set_status_text(SharedString::from("No files selected"));
            return;
        }

        let cancelled = state_ref.cancelled.clone();
        drop(state_ref);

        let game_version = GameVersion::from_index(window.get_selected_game_version());

        // Determine file extension for save dialog
        let ext = if game_version.is_ba2() { "ba2" } else { "bsa" };
        let filter_name = if game_version.is_ba2() {
            "BA2 Archive"
        } else {
            "BSA Archive"
        };

        let output_path = rfd::FileDialog::new()
            .add_filter(filter_name, &[ext])
            .set_file_name(format!("archive.{}", ext))
            .save_file();
        let output_path = match output_path {
            Some(p) => p,
            None => return,
        };

        state.lock().unwrap().reset_cancel();
        window.set_is_processing(true);
        window.set_progress(0.0);

        let window_weak_thread = window.as_weak();

        std::thread::spawn(move || {
            let result = pack_files(
                &source_folder,
                &selected_files,
                &output_path,
                game_version,
                &cancelled,
                &window_weak_thread,
            );

            let _ = window_weak_thread.upgrade_in_event_loop(move |w: MainWindow| {
                w.set_is_processing(false);
                w.set_progress(1.0);
                match result {
                    Ok(count) => {
                        w.set_status_text(SharedString::from(format!(
                            "Packed {} files into {}",
                            count,
                            output_path.display()
                        )));
                    }
                    Err(e) => {
                        w.set_status_text(SharedString::from(format!("Pack error: {}", e)));
                    }
                }
            });
        });
    });
}

/// Pack selected files from a source folder into an archive
fn pack_files(
    source_folder: &Path,
    selected_files: &[String],
    output_path: &Path,
    game_version: GameVersion,
    cancelled: &Arc<AtomicBool>,
    window_weak: &Weak<MainWindow>,
) -> Result<usize> {
    let total = selected_files.len();

    if game_version.is_ba2() {
        let ba2_version = game_version.ba2_version().unwrap_or_default();
        let compression = game_version.ba2_compression();

        // Detect DX10 from output name
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

        for (idx, file_path) in selected_files.iter().enumerate() {
            if cancelled.load(Ordering::SeqCst) {
                bail!("Cancelled");
            }
            // file_path uses backslash from tree; convert to forward slash for disk read
            let disk_path = source_folder.join(file_path.replace('\\', "/"));
            let data = fs::read(&disk_path)?;
            builder.add_file(file_path, data);

            let progress = (idx + 1) as f32 / total as f32;
            let path = file_path.clone();
            let _ = window_weak.upgrade_in_event_loop(move |w: MainWindow| {
                w.set_progress(progress * 0.5); // first half = reading
                w.set_status_text(SharedString::from(format!("Reading: {}", path)));
            });
        }

        let window_weak2 = window_weak.clone();
        builder.build_with_progress(output_path, move |current, btotal, name| {
            let progress = 0.5 + (current as f32 / btotal as f32) * 0.5;
            let name = name.to_string();
            let _ = window_weak2.upgrade_in_event_loop(move |w: MainWindow| {
                w.set_progress(progress);
                w.set_status_text(SharedString::from(format!("Packing: {}", name)));
            });
        })?;
    } else if game_version.is_tes3() {
        bail!("Morrowind TES3 BSA writing is not supported");
    } else {
        // BSA (TES4)
        let bsa_version = game_version.bsa_version().unwrap();
        let compress = game_version.supports_compression();

        let mut builder = BsaBuilder::new()
            .with_version(bsa_version)
            .with_compression(compress);

        for (idx, file_path) in selected_files.iter().enumerate() {
            if cancelled.load(Ordering::SeqCst) {
                bail!("Cancelled");
            }
            let disk_path = source_folder.join(file_path.replace('\\', "/"));
            let data = fs::read(&disk_path)?;
            builder.add_file(file_path, data);

            let progress = (idx + 1) as f32 / total as f32;
            let path = file_path.clone();
            let _ = window_weak.upgrade_in_event_loop(move |w: MainWindow| {
                w.set_progress(progress * 0.5);
                w.set_status_text(SharedString::from(format!("Reading: {}", path)));
            });
        }

        let window_weak2 = window_weak.clone();
        builder.build_with_progress(output_path, move |current, btotal, name| {
            let progress = 0.5 + (current as f32 / btotal as f32) * 0.5;
            let name = name.to_string();
            let _ = window_weak2.upgrade_in_event_loop(move |w: MainWindow| {
                w.set_progress(progress);
                w.set_status_text(SharedString::from(format!("Packing: {}", name)));
            });
        })?;
    }

    Ok(total)
}

fn setup_select_all(window: &MainWindow, state: StateHandle) {
    let window_weak = window.as_weak();
    window.on_select_all(move || {
        let window = window_weak.unwrap();
        let mut state = state.lock().unwrap();
        state.select_all();
        window.set_tree_nodes(state.to_slint_model());
        window.set_status_text(SharedString::from(format!(
            "{} files selected",
            state.selected_count()
        )));
    });
}

fn setup_select_none(window: &MainWindow, state: StateHandle) {
    let window_weak = window.as_weak();
    window.on_select_none(move || {
        let window = window_weak.unwrap();
        let mut state = state.lock().unwrap();
        state.select_none();
        window.set_tree_nodes(state.to_slint_model());
        window.set_status_text(SharedString::from("0 files selected"));
    });
}

fn setup_search(window: &MainWindow, state: StateHandle) {
    let window_weak = window.as_weak();
    window.on_search_changed(move |text: SharedString| {
        let window = window_weak.unwrap();
        let mut state = state.lock().unwrap();
        state.set_search(text.to_string());
        window.set_tree_nodes(state.to_slint_model());
    });
}

fn setup_toggle_expand(window: &MainWindow, state: StateHandle) {
    let window_weak = window.as_weak();
    window.on_toggle_expand(move |index| {
        let window = window_weak.unwrap();
        let mut state = state.lock().unwrap();
        state.toggle_expand(index as usize);
        window.set_tree_nodes(state.to_slint_model());
    });
}

fn setup_toggle_select(window: &MainWindow, state: StateHandle) {
    let window_weak = window.as_weak();
    window.on_toggle_select(move |index| {
        let window = window_weak.unwrap();
        let mut state = state.lock().unwrap();
        state.toggle_select(index as usize);
        window.set_tree_nodes(state.to_slint_model());
        window.set_status_text(SharedString::from(format!(
            "{} files selected",
            state.selected_count()
        )));
    });
}
