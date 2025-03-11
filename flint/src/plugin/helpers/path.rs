use directories::UserDirs;
use mlua::{Lua, Table};

use crate::{app::AppResult, get_flag};

pub fn path_helpers(lua: &Lua) -> AppResult<Table> {
    let path = lua.create_table()?;

    let cwd = lua.create_function(|lua, ()| {
        let cwd = get_flag!(current_dir);
        Ok(lua.create_string(cwd.to_string_lossy().as_ref())?)
    })?;

    let path_resolve = lua.create_function(|lua, paths: mlua::Variadic<String>| {
        use std::path::{Path, PathBuf};

        let cwd = get_flag!(current_dir);
        let mut result = PathBuf::new();
        let mut absolute = false;

        // Process each path segment similar to Node.js path.resolve
        for path in paths.iter() {
            let path_obj = Path::new(path);

            // If path is absolute, reset result and set absolute flag
            if path_obj.is_absolute() {
                result = PathBuf::from(path);
                absolute = true;
            } else if path.starts_with("~")
                && path.len() > 1
                && (path.len() == 1 || path.chars().nth(1) == Some('/'))
            {
                // Handle home directory with ~
                match UserDirs::new() {
                    Some(user_dirs) => {
                        let home = user_dirs.home_dir();
                        if path.len() > 1 {
                            result = home.join(&path[2..]);
                        } else {
                            result = home.to_path_buf();
                        }
                        absolute = true;
                    }
                    None => (),
                }
            } else {
                // For relative paths, append to result
                if !absolute {
                    // If this is the first path and it's relative, start from cwd
                    if result.as_os_str().is_empty() {
                        result = cwd.clone();
                    }
                }
                result = result.join(path);
            }
        }

        // If no paths provided, return cwd
        if result.as_os_str().is_empty() {
            result = cwd.clone();
        }

        // Normalize the path
        if let Ok(canonicalized) = result.canonicalize() {
            result = canonicalized;
        }

        Ok(lua.create_string(result.to_string_lossy().as_ref())?)
    })?;
    let path_join = lua.create_function(|lua, paths: mlua::Variadic<String>| {
        use std::path::Path;

        // If there are no path segments, return empty string
        if paths.len() == 0 {
            return Ok(lua.create_string("")?);
        }

        // Node.js path.join() just combines segments with the platform-specific separator
        // and normalizes the result, but it doesn't resolve to absolute paths
        let mut result = String::new();

        for (i, path) in paths.iter().enumerate() {
            // Skip empty segments (but preserve them at the beginning)
            if path.is_empty() && i > 0 {
                continue;
            }

            // Add separator between segments
            if i > 0 && !result.is_empty() && !result.ends_with(std::path::MAIN_SEPARATOR) {
                result.push(std::path::MAIN_SEPARATOR);
            }

            // Add the path segment
            result.push_str(path);
        }

        // Normalize the path (remove unnecessary separators/dots)
        let normalized = Path::new(&result).to_string_lossy();

        Ok(lua.create_string(normalized.as_ref())?)
    })?;
    path.set("join", path_join)?;
    path.set("resolve", path_resolve)?;
    path.set("cwd", cwd)?;

    Ok(path)
}
