//! `forb` - the Formatorbit CLI.
//!
//! This file is a thin entry point; the actual functionality lives in the
//! modules below:
//!
//! - [`cli`] - clap argument definitions (`Cli`) and argument helpers
//! - [`run`] - top-level orchestration (config resolution, input, output)
//! - [`input`] - `@path` / `@-` / URL input acquisition
//! - [`commands`] - standalone subcommands (`--formats`, `--analytics`, ...)
//! - [`render`] - conversion-value display and per-input DOT/Mermaid graphs
//! - [`man`] - man-page generation/installation and pager output
//! - [`pretty`] / [`pipe`] / [`tokenizer`] - pretty-printing and tee mode
//! - [`graph`] - static format-graph builders (`--graph`)
//! - [`analytics`] / [`config`] / [`updates`] - local analytics, config file,
//!   and update checks

mod analytics;
mod cli;
mod commands;
mod config;
mod graph;
mod input;
mod man;
mod pipe;
mod pretty;
mod render;
mod run;
mod tokenizer;
mod updates;

fn main() {
    run::run();
}
