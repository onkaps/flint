use super::{AppResult, AppWidget};
use crate::{
    util::{get_plugin_map, plugin::Plugin, toml::read_toml_config},
    widgets::logs::{add_log, LogKind, LogsWidget},
};
use flint_macros::ui;
use ratatui::prelude::*;
use ratatui::widgets::WidgetRef;
use std::{collections::BTreeSet, sync::Arc};
use threadpool::ThreadPool;

pub struct GenerateWidget {
    plugins: Vec<Plugin>,
    thread_pool: ThreadPool,
    logs_widget: LogsWidget,
}

impl Default for GenerateWidget {
    fn default() -> Self {
        Self {
            plugins: Vec::new(),
            thread_pool: ThreadPool::new(16),
            logs_widget: LogsWidget::default(),
        }
    }
}

impl AppWidget for GenerateWidget {
    fn setup(&mut self) -> AppResult<()> {
        let toml = Arc::new(read_toml_config("./flint.toml")?);
        let plugin_ids = toml.linters.keys().collect::<Vec<&String>>();

        self.plugins = get_plugin_map()
            .values()
            .flat_map(|plugin_set| plugin_set.iter())
            .collect::<BTreeSet<&Plugin>>()
            .into_iter()
            .filter(|plugin| plugin_ids.contains(&&plugin.details.id))
            .cloned()
            .collect();

        for plugin in &self.plugins {
            let plugin = plugin.clone();
            let toml_clone = toml.clone();

            self.thread_pool.execute(move || {
                let result = plugin.run(&toml_clone);
                match result {
                    Ok(res) => {
                        // TODO: Ask user if we want to overwrite files
                        for (file_name, contents) in res {
                            std::fs::write(file_name, contents).unwrap();
                        }
                        add_log(
                            LogKind::Success,
                            format!("Generated {} config successfully", plugin.details.id),
                        );
                    }
                    Err(err) => {
                        add_log(LogKind::Error, err.to_string());
                    }
                }
            });
        }

        Ok(())
    }
}

impl WidgetRef for GenerateWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        ui!((area, buf) => {
            { self.logs_widget }
        });
    }
}
