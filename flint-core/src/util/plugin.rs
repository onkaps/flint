use super::{toml::Config, PLUGINS, PLUGIN_MAP};
use crate::widgets::logs::{add_log, LogKind};
use serde_json::to_string_pretty;

use directories::ProjectDirs;
use mlua::{Function, Lua, LuaSerdeExt, Value};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, HashMap},
    path::PathBuf,
    sync::Arc,
};

#[derive(Serialize, Deserialize, Debug, Ord, PartialOrd, Eq, PartialEq, Clone)]
pub struct PluginDetails {
    pub id: String,
    pub extensions: Vec<String>,
    pub version: String,
    pub author: String,
    pub category: String,
}

#[derive(Serialize, Deserialize, Debug, Ord, PartialOrd, Eq, PartialEq, Clone)]
pub struct Plugin {
    pub details: PluginDetails,
    pub path: PathBuf,
}

pub fn get_plugins_dir() -> PathBuf {
    if cfg!(debug_assertions) {
        return PathBuf::from("./flint-core/src/plugins");
    } else if let Some(proj_dirs) = ProjectDirs::from("com", "Flint", "flint") {
        let plugins_path = proj_dirs.data_dir().to_path_buf().join("plugins");
        if !plugins_path.exists() {
            std::fs::create_dir_all(&plugins_path).expect("Failed to create plugins directory");
            std::fs::create_dir_all(&plugins_path.join("test"))
                .expect("Failed to create test directory");
            std::fs::create_dir_all(&plugins_path.join("lint"))
                .expect("Failed to create lint directory");
        }
        plugins_path
    } else {
        panic!("Unable to determine project directories");
    }
}

pub fn list_plugins() -> BTreeSet<Plugin> {
    let lua = Lua::new();

    let mut plugins = BTreeSet::new();
    let plugins_dir = get_plugins_dir().join("lint");
    if let Ok(entries) = std::fs::read_dir(plugins_dir) {
        for entry in entries {
            if let Ok(entry) = entry {
                let file_path = entry.path();
                let contents = match std::fs::read_to_string(&file_path) {
                    Ok(contents) => contents,
                    Err(err) => {
                        eprintln!("Error reading file {}: {}", file_path.display(), err);
                        continue;
                    }
                };

                match lua.load(contents).exec() {
                    Ok(_) => {
                        let details: Function = lua.globals().get("Details").unwrap();
                        let lua_val = details.call::<mlua::Value>(()).unwrap();
                        let details: PluginDetails = lua.from_value(lua_val).unwrap();
                        plugins.insert(Plugin {
                            details,
                            path: file_path,
                        });
                    }
                    Err(err) => {
                        eprintln!("Error loading lua file {}: {}", file_path.display(), err);
                        continue;
                    }
                }
            }
        }
    }
    plugins
}

pub fn get_plugin_map() -> &'static HashMap<String, BTreeSet<Plugin>> {
    PLUGIN_MAP.get_or_init(|| {
        let plugins = PLUGINS.get_or_init(|| list_plugins());
        let mut m = HashMap::new();
        for plugin in plugins {
            for extension in &plugin.details.extensions {
                m.entry(extension.clone())
                    .or_insert_with(BTreeSet::new)
                    .insert(plugin.clone());
            }
        }
        m
    })
}

pub fn run_plugin<'a>(
    plugin: &Plugin,
    toml: &Arc<Config>,
) -> Result<HashMap<String, String>, String> {
    let lua = Lua::new();
    add_helper_globals(&lua);
    let common_config = lua
        .to_value(&toml.common)
        .expect("unable to convert common config to lua value");
    let plugin_config = toml
        .linters
        .get(&plugin.details.id)
        .expect("unable to find config for a plugin");
    let plugin_config = lua
        .to_value(plugin_config)
        .expect("unable to convert plugin config to lua value");
    let plugin_config = plugin_config
        .as_table()
        .expect("unable to convert plugin config lua value to table");

    plugin_config
        .set("common", common_config)
        .expect("unable to set common table to config table");

    let contents = match std::fs::read_to_string(&plugin.path) {
        Ok(contents) => contents,
        Err(_) => {
            return Err("Error reading plugin code".into());
        }
    };

    let (validate, generate) = match lua.load(contents).exec() {
        Ok(_) => {
            let validate: Function = lua
                .globals()
                .get("Validate")
                .expect("could not find validate function in plugin file");
            let generate: Function = lua
                .globals()
                .get("Generate")
                .expect("could not find generate function in plugin file");
            (validate, generate)
        }
        Err(_) => {
            return Err("Error loading lua file".into());
        }
    };

    let validate_success = validate
        .call::<mlua::Value>(plugin_config)
        .expect("error running validate function");

    let validate_success: bool = lua
        .from_value(validate_success)
        .expect("unable to convert validation result to boolean");
    if !validate_success {
        return Err("Plugin config validation failed".into());
    }

    let generate_results = generate
        .call::<mlua::Value>(plugin_config)
        .expect("error running generate function");
    let generate_results: HashMap<String, String> = lua
        .from_value(generate_results)
        .expect("unable to convert generation result to String");

    Ok(generate_results)
}

fn add_helper_globals(lua: &Lua) {
    let log = lua.create_table().unwrap();
    let create_log_fn = |kind: LogKind| {
        lua.create_function(move |_, message: String| {
            add_log(kind, message);
            Ok(())
        })
        .unwrap()
    };

    let debug_print = lua
        .create_function(|_, value: Value| match to_string_pretty(&value) {
            Ok(json) => {
                add_log(LogKind::Debug, json);
                Ok(())
            }
            Err(err) => Err(mlua::Error::external(err)),
        })
        .unwrap();

    let to_json = lua
        .create_function(|_, value: Value| match to_string_pretty(&value) {
            Ok(json) => Ok(json),
            Err(err) => Err(mlua::Error::external(err)),
        })
        .unwrap();

    log.set("info", create_log_fn(LogKind::Info)).unwrap();
    log.set("error", create_log_fn(LogKind::Error)).unwrap();
    log.set("warn", create_log_fn(LogKind::Warn)).unwrap();
    log.set("success", create_log_fn(LogKind::Success)).unwrap();
    log.set("debug", debug_print).unwrap();
    lua.globals().set("to_json", to_json).unwrap();
    lua.globals().set("log", log).unwrap();
}
