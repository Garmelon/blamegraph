use std::{fs, path::Path};

use jiff::tz::TimeZone;
use serde::Serialize;

use crate::{data::Commit, graph::common};

use super::series::Series;

#[derive(Serialize)]
pub struct Graph {
    title: String,
    commits: Vec<Commit>,
    time: Vec<i64>,
    series: Vec<Series>,
}

impl Graph {
    pub fn new(
        title: &str,
        mut commits: Vec<Commit>,
        mut time: Vec<i64>,
        mut series: Vec<Series>,
    ) -> Self {
        commits.reverse();
        time.reverse();
        for series in &mut series {
            series.reverse();
        }

        Self {
            title: title.to_string(),
            commits,
            time,
            series,
        }
    }

    pub fn make_equidistant(&mut self, tz: TimeZone) {
        common::make_equidistant(&tz, &mut self.time);
    }

    pub fn save_json(&self, path: &Path) -> anyhow::Result<()> {
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(path, serde_json::to_vec(self)?)?;
        Ok(())
    }

    pub fn save_html(&self, path: &Path) -> anyhow::Result<()> {
        const UPLOT_CSS: &str = include_str!("../../static/uPlot.css");

        const UPLOT_JS: &str = include_str!("../../static/uPlot.js");
        const UPLOT_STACK_JS: &str = include_str!("../../static/uPlot_stack.js");
        const GRAPH_TEMPLATE: &str = include_str!("../../static/graph_template.html");

        let data = serde_json::to_string(self)?;
        let html = GRAPH_TEMPLATE
            .replace("/* replace with uplot css */", UPLOT_CSS)
            .replace("/* replace with uplot js */", UPLOT_JS)
            .replace("/* replace with uplot stack js */", UPLOT_STACK_JS)
            .replace("$replace_with_data$", &data);

        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(path, html)?;
        Ok(())
    }
}
