use std::env;
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process;
use std::time::UNIX_EPOCH;

use chrono::{DateTime, Local};
use clap::Parser;
use users::{get_group_by_gid, get_user_by_uid};

// COLORS: https://encrypted-tbn0.gstatic.com/images?q=tbn:ANd9GcT75fjCYt2l_dPGNNJcUj-nCjMSEgaCK1blGJcNR83oz8k47qFsWgF1Hw&s=10

const DATE_COLOR_TODAY: &str = "\x1b[37m";
const DATE_COLOR_1DAY: &str = "\x1b[38;5;39m";
const DATE_COLOR_1MONTH: &str = "\x1b[38;5;33m";
const HEADER_BACKGROUND: &str = "\x1b[4m\x1b[47m\x1b[30m"; // UNDERLINE, BLACK ON WHITE
const COLOR_RESET: &str = "\x1b[0m";

#[derive(Parser)]
#[command(
    name = "myls",
    about = "Custom ls -l alternative with enhanced formatting",
    long_about = "Custom ls -l alternative with enhanced formatting and customization.\nDisplays file information with zebra striping and colors."
)]
struct Args {
    /// Files or directories to list (default: current directory)
    #[arg(default_value = ".")]
    paths: Vec<String>,

    /// Show hidden files (starting with .) when listing a directory
    #[arg(short, long)]
    all: bool,

    /// Maximum length of file name to display. If 0 (default), no limit is applied.
    #[arg(long, default_value = "0")]
    max_name_length: usize,

    /// Color files based on their suffix, in the format "suffix=color", separated by commas.
    /// Example: --file-colors ".py=38;5;220m,.html=38;5;208m"
    #[arg(long, value_parser = parse_file_colors)]
    file_colors: Option<HashMap<String, String>>,

    /// Shows folder icons
    #[arg(short, long)]
    icons: bool,

    /// Display the version number
    #[arg(short, long)]
    version: bool
}

fn parse_file_colors(s: &str) -> Result<HashMap<String, String>, String> {
    let mut map = HashMap::new();
    for kv in s.split(',') {
        let parts: Vec<&str> = kv.split('=').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid format: {}", kv));
        }
        map.insert(parts[0].to_string(), parts[1].to_string());
    }
    Ok(map)
}

fn main() {
    let exit_code = run();
    process::exit(exit_code);
}

fn run() -> i32 {
    let args = Args::parse();

    if args.version {
        println!("myls {}", env!("CARGO_PKG_VERSION"));
        return 0;
    }

    let paths: Vec<PathBuf> = if args.paths.len() == 1 && args.paths[0] == "." {
        vec![env::current_dir().unwrap_or_else(|_| PathBuf::from("."))]
    } else {
        args.paths.iter().map(|p| PathBuf::from(p)).collect()
    };

    let paths: Vec<&Path> = paths.iter().map(|p| p.as_path()).collect();

    let mut raw_infos: Vec<RawInfo> = Vec::new();

    for path in &paths {
        if !path.exists() {
            eprintln!("Error: {} does not exist", path.display());
            return 1;
        }

        // Single dir mode: list dir contents, after dir info itself
        if path.is_dir() && paths.len() == 1 {
            if let Some(mut main_dir_info) = get_file_info(path) {
                main_dir_info.is_main_dir = true;
                raw_infos.push(main_dir_info);
            }
            raw_infos.extend(list_directory(path, args.all));
        }
        // Normal mode: list details of given files and dirs
        else {
            if let Some(file_info) = get_file_info(path) {
                raw_infos.push(file_info);
            }
        }
    }

    // Process the raw data into information needed for printing
    let mut processed_infos: Vec<ProcessedInfo> = raw_infos
        .into_iter()
        .map(|raw_info| ProcessedInfo::new(raw_info, args.icons, args.max_name_length))
        .collect();

    // Sort: main dir first, then directories (and links to directories), then by name
    processed_infos.sort_by(|a, b| {
        a.sort_keys.cmp(&b.sort_keys)
    });

    let max_owner_colsize = processed_infos
        .iter()
        .map(|pi| pi.username.len() + pi.groupname.len())
        .max()
        .unwrap_or(0)
        + 1;

    // Adds padding and colors to the output.
    let mut displayable_infos: Vec<DisplayableInfo> = processed_infos
        .into_iter()
        .enumerate()
        .map(|(i, pinfo)| {
            DisplayableInfo::new(
                i,
                pinfo,
                max_owner_colsize,
                args.file_colors.as_ref().unwrap_or(&HashMap::new()),
            )
        })
        .collect();

    // Print header with inverted colors for more contrast
    let header = format!(
        "{:>4} {:>7} {:>width$} {:>10} NAME",
        "PERM",
        "SIZE",
        "OWNER",
        "MODIFIED",
        width = max_owner_colsize
    );
    println!("{}{}{}", HEADER_BACKGROUND, header, COLOR_RESET);

    // If the input is a single directory, print its own info before the content list
    if !displayable_infos.is_empty() && displayable_infos[0].is_main_dir {
        let main_dir_info = displayable_infos.remove(0);
        println!(
            "{} {} {} {} {}",
            main_dir_info.permission_col,
            main_dir_info.size_col,
            main_dir_info.owner_col,
            main_dir_info.date_col,
            main_dir_info.name_col
        );
        if !displayable_infos.is_empty() {
            println!("{}", "-".repeat(60));
        }
    }

    // Print each file with formatted output
    for dinfo in displayable_infos {
        println!(
            "{} {} {} {} {}",
            dinfo.permission_col, dinfo.size_col, dinfo.owner_col, dinfo.date_col, dinfo.name_col
        );
    }

    0
}

// #[derive(Debug)]
struct RawInfo {
    path: PathBuf,
    permissions: u32,
    size: u64,
    owner_uid: u32,
    group_gid: u32,
    modified_time: DateTime<Local>,
    is_directory: bool,
    is_executable: bool,
    is_symlink: bool,
    is_main_dir: bool,
}

struct ProcessedInfo {
    rinfo: RawInfo,
    permissions: String,
    size: String,
    size_unit: String,
    username: String,
    groupname: String,
    name: String,
    target_name: String,
    is_executable: bool,
    sort_keys: (u8, String),
}

impl ProcessedInfo {
    const KB: u64 = 1024;
    const MB: u64 = Self::KB * 1024;
    const GB: u64 = Self::MB * 1024;

    fn new(raw_info: RawInfo, show_icons: bool, max_name_length: usize) -> Self {
        // Format permissions as octal string.
        let permissions = format!("{:03o}", raw_info.permissions);

        let (size, size_unit) = Self::get_size_and_unit(&raw_info);

        let username = get_user_by_uid(raw_info.owner_uid)
            .map(|u| u.name().to_string_lossy().to_string())
            .unwrap_or_else(|| raw_info.owner_uid.to_string());

        let groupname = get_group_by_gid(raw_info.group_gid)
            .map(|g| g.name().to_string_lossy().to_string())
            .unwrap_or_else(|| raw_info.group_gid.to_string());

        let target = if raw_info.is_symlink {
            raw_info.path.read_link().ok()
        } else {
            None
        };

        let targets_folder = target
            .as_ref()
            .map(|t| t.exists() && t.is_dir())
            .unwrap_or(false);

        // Enshorten names if needed.
        let base_name = raw_info.path.file_name().unwrap().to_string_lossy();
        let name = if max_name_length > 0 {
            Self::pstr(&base_name, max_name_length)
        } else {
            base_name.to_string()
        };

        let target_name = if let Some(ref target) = target {
            let target_str = target.display().to_string();
            if max_name_length > 0 {
                Self::pstr(&target_str, max_name_length)
            } else {
                target_str
            }
        } else {
            String::new()
        };

        // Format names with folder emoji if directory.
        let folder_icon = if !show_icons {
            "■"
        } else if raw_info.is_main_dir {
            "📂"
        } else {
            "📁"
        };
        
        let name = if raw_info.is_directory {
            format!("{} {}", folder_icon, name)
        } else {
            name
        };

        let target_name = if !target_name.is_empty() && targets_folder {
            format!("{} {}", folder_icon, target_name)
        } else {
            target_name
        };

        // Disconsider directories and folder links as executables.
        let is_executable = raw_info.is_executable
            && !raw_info.is_directory
            && (!target.is_some() || !targets_folder);

        let sort_name = raw_info.path.file_name().unwrap().to_string_lossy().to_lowercase();
        let sort_keys = if raw_info.is_main_dir {
            (0, sort_name)
        } else if raw_info.is_directory || targets_folder {
            (1, sort_name)
        } else {
            (2, sort_name)
        };

        ProcessedInfo {
            rinfo: raw_info,
            permissions,
            size,
            size_unit,
            username,
            groupname,
            name,
            target_name,
            is_executable,
            sort_keys,
        }
    }

    fn get_size_and_unit(raw_info: &RawInfo) -> (String, String) {
        if raw_info.is_directory || raw_info.is_symlink {
            return (String::new(), String::new());
        }

        if raw_info.size < Self::KB {
            (raw_info.size.to_string(), "B".to_string())
        } else if raw_info.size < Self::MB {
            ((raw_info.size / Self::KB).to_string(), "K".to_string())
        } else if raw_info.size < Self::GB {
            (format!("{:.1}", raw_info.size as f64 / Self::MB as f64), "M".to_string())
        } else {
            (format!("{:.1}", raw_info.size as f64 / Self::GB as f64), "G".to_string())
        }
    }

    fn pstr(string: &str, maxlength: usize) -> String {
        if string.len() > maxlength + 5 {
            let half_index = maxlength / 2;
            format!(
                "{}(...){}", 
                &string[..half_index], 
                &string[string.len() - half_index..]
            )
        } else {
            string.to_string()
        }
    }
}

struct DisplayableInfo {
    permission_col: String,
    size_col: String,
    owner_col: String,
    date_col: String,
    name_col: String,
    is_main_dir: bool,
}

impl DisplayableInfo {
    // ANSI color codes for zebra striping
    const ZEBRA_EVEN: &'static str = "\x1b[48;5;236m"; // Dark gray background
    const ZEBRA_ODD: &'static str = "\x1b[48;5;235m";  // Slightly darker gray background
    const GREEN: &'static str = "\x1b[32m";            // Green text for executables
    const YELLOW: &'static str = "\x1b[33m";           // Yellow text for mega size
    const RED: &'static str = "\x1b[31m";              // Red text for giga size

    fn new(
        row_index: usize,
        processed_info: ProcessedInfo,
        max_owner_colsize: usize,
        file_colors: &HashMap<String, String>,
    ) -> Self {
        // Apply zebra striping
        let reset_color = format!(
            "{}{}",
            COLOR_RESET,
            if row_index % 2 == 0 {
                Self::ZEBRA_EVEN
            } else {
                Self::ZEBRA_ODD
            }
        );

        let permission_col = format!("{}{:>4}", reset_color, processed_info.permissions);
        let size_col = Self::fmt_size(&processed_info, &reset_color);
        let owner_col = format!(
            "{:<width$}",
            Self::fmt_owner(&processed_info),
            width = max_owner_colsize
        );
        let date_col = Self::fmt_modified_time(&processed_info, &reset_color);
        let name_col = format!(
            "{}{}",
            Self::fmt_name(&processed_info, file_colors),
            COLOR_RESET
        );

        DisplayableInfo {
            permission_col,
            size_col,
            owner_col,
            date_col,
            name_col,
            is_main_dir: processed_info.rinfo.is_main_dir,
        }
    }

    fn fmt_size(pinfo: &ProcessedInfo, reset_color: &str) -> String {
        if pinfo.size.is_empty() {
            return "      -".to_string();
        }

        let unit_color = match pinfo.size_unit.as_str() {
            "B" | "K" => Self::GREEN,
            "M" => Self::YELLOW,
            _ => Self::RED,
        };

        format!(
            "{:>6}{}{}{}",
            pinfo.size, unit_color, pinfo.size_unit, reset_color
        )
    }

    fn fmt_owner(pinfo: &ProcessedInfo) -> String {
        format!("{}:{}", pinfo.username, pinfo.groupname)
    }

    fn fmt_modified_time(pinfo: &ProcessedInfo, reset_color: &str) -> String {
        let now = Local::now();
        let mdays = (now - pinfo.rinfo.modified_time).num_days();

        let (color, fmt) = if mdays > 364 {
            (DATE_COLOR_1MONTH, "%d/%m/%Y")
        } else if mdays > 30 {
            (DATE_COLOR_1MONTH, "%d/%m")
        } else if mdays > 0 {
            (DATE_COLOR_1DAY, "%d/%m")
        } else {
            (DATE_COLOR_TODAY, "%H:%M")
        };

        format!(
            "{}{:>10}{}",
            color,
            pinfo.rinfo.modified_time.format(fmt),
            reset_color
        )
    }

    fn fmt_name(
        pinfo: &ProcessedInfo,
        file_colors: &HashMap<String, String>,
    ) -> String {
        let mut fname = pinfo.name.clone();

        // Apply green color to executable entries (except directories and folder links)
        if pinfo.is_executable {
            fname = format!("{}{}{}", Self::GREEN, fname, COLOR_RESET);
        } else if !file_colors.is_empty() {
            // Apply color to file names containing special suffixes
            // Use the original file name (without icons) for suffix checking
            let original_name = pinfo.rinfo.path.file_name().unwrap().to_string_lossy();
            for (suffix, color) in file_colors {
                if original_name.ends_with(suffix) {
                    fname = format!("\x1b[{}{}{}", color, fname, COLOR_RESET);
                    break;
                }
            }
        }

        if !pinfo.target_name.is_empty() {
            fname = format!("{} -> {}", fname, pinfo.target_name);
        }

        fname
    }
}

fn get_file_info(path: &Path) -> Option<RawInfo> {
    let metadata = match path.symlink_metadata() {
        Ok(metadata) => metadata,
        Err(e) => {
            eprintln!("Error accessing {}: {}", path.display(), e);
            return None;
        }
    };

    let modified_time = metadata
        .modified()
        .ok()
        .and_then(|time| {
            let duration = time.duration_since(UNIX_EPOCH).ok()?;
            DateTime::from_timestamp(duration.as_secs() as i64, 0)
                .map(|dt| dt.with_timezone(&Local))
        })
        .unwrap_or_else(|| Local::now());

    Some(RawInfo {
        path: path.to_path_buf(),
        permissions: metadata.permissions().mode() & 0o777,
        size: metadata.len(),
        owner_uid: metadata.uid(),
        group_gid: metadata.gid(),
        modified_time,
        is_directory: metadata.is_dir(),
        is_executable: metadata.permissions().mode() & 0o100 != 0,
        is_symlink: metadata.file_type().is_symlink(),
        is_main_dir: false,
    })
}

fn list_directory(directory: &Path, show_hidden: bool) -> Vec<RawInfo> {
    let mut raw_infos = Vec::new();

    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("Permission denied: {}: {}", directory.display(), e);
            return raw_infos;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                eprintln!("Error reading directory entry: {}", e);
                continue;
            }
        };

        let path = entry.path();
        let file_name = path.file_name().unwrap().to_string_lossy();

        if show_hidden || !file_name.starts_with('.') {
            if let Some(raw_info) = get_file_info(&path) {
                raw_infos.push(raw_info);
            }
        }
    }

    raw_infos
}