//! Command line surface. Every --help stays short and carries one example.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(
    name = "svgdoc",
    version,
    about = "Compile and check SVG diagrams for technical documentation.",
    disable_help_subcommand = true,
    after_help = "Example:\n  svgdoc validate docs/fig/orders.svg --format json"
)]
pub struct Cli {
    #[command(flatten)]
    pub global: Global,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Format {
    Json,
    Text,
}

#[derive(Args, Debug)]
pub struct Global {
    /// Output form [default: text on a terminal, json otherwise]
    #[arg(long, global = true, value_enum, value_name = "json|text")]
    pub format: Option<Format>,
    /// Config file to use [default: nearest svgdoc.toml above the input file]
    #[arg(long, global = true, value_name = "PATH", conflicts_with = "no_config")]
    pub config: Option<PathBuf>,
    /// Ignore any config file
    #[arg(long, global = true)]
    pub no_config: bool,
    /// Rule profile: minimal, docs or strict [default: docs]
    #[arg(long, global = true, value_name = "NAME")]
    pub profile: Option<String>,
    /// Suppress stderr logs
    #[arg(long, global = true)]
    pub quiet: bool,
    /// Plain text output; NO_COLOR is also honoured
    #[arg(long, global = true)]
    pub no_color: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Page {
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum FailOn {
    Error,
    Warning,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum SchemaName {
    Spec,
    Config,
    Findings,
    Scene,
    Ops,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Compile a spec to SVG and validate the result
    #[command(after_help = "Example:\n  svgdoc render fig/orders.yaml -o fig/orders.svg")]
    Render {
        /// Diagram spec, YAML or JSON; - reads stdin
        #[arg(value_name = "SPEC")]
        spec: String,
        /// Where to write the SVG; - writes it to stdout in place of the report
        #[arg(short = 'o', value_name = "OUT")]
        output: String,
        /// Theme: docs-light or docs-dark [default: the spec's, else docs-light]
        #[arg(long, value_name = "NAME")]
        theme: Option<String>,
        /// Skip the checks on the result
        #[arg(long)]
        no_validate: bool,
        /// Replace an existing output file
        #[arg(long)]
        force: bool,
        /// Convert text to outlines: exact everywhere, but not searchable
        #[arg(long)]
        text_as_paths: bool,
    },
    /// Run rules on one or more SVG files
    #[command(after_help = "Example:\n  svgdoc validate 'docs/**/*.svg' --fail-on warning")]
    Validate {
        /// SVG files or globs; - reads stdin
        #[arg(required = true, value_name = "FILE")]
        files: Vec<String>,
        /// Run only these rules
        #[arg(long, value_delimiter = ',', value_name = "ID,...")]
        rules: Vec<String>,
        /// Skip these rules
        #[arg(long, value_delimiter = ',', value_name = "ID,...")]
        skip: Vec<String>,
        /// Lowest severity that gives exit code 1
        #[arg(long, value_enum, default_value = "error", value_name = "error|warning")]
        fail_on: FailOn,
        /// Cap on listed findings; summary counts stay complete
        #[arg(long, default_value_t = 50, value_name = "N")]
        max_findings: usize,
    },
    /// Describe an SVG's elements as data
    #[command(after_help = "Example:\n  svgdoc inspect fig/orders.svg --select '#node-api' --fields id,tag,bbox,text")]
    Inspect {
        /// SVG file; - reads stdin
        #[arg(value_name = "FILE")]
        file: String,
        /// Keep only matching elements and their descendants: #id, tag or tag[attr="value"]
        #[arg(long, value_name = "SELECTOR")]
        select: Option<String>,
        /// Deepest nesting level to return; the root <svg> is 0
        #[arg(long, value_name = "N")]
        depth: Option<usize>,
        /// Fields to return
        #[arg(long, value_delimiter = ',', default_value = "id,tag,bbox,text", value_name = "id,tag,bbox,text,fill,stroke,refs")]
        fields: Vec<String>,
    },
    /// Rasterize to PNG
    #[command(after_help = "Example:\n  svgdoc preview fig/orders.svg -o /tmp/orders.png --annotate")]
    Preview {
        /// SVG file; - reads stdin
        #[arg(value_name = "FILE")]
        file: String,
        /// Where to write the PNG; - writes it to stdout in place of the report
        #[arg(short = 'o', value_name = "OUT")]
        output: String,
        /// Image width in px; larger values are cut to 2000 [default: 1200]
        #[arg(long, value_name = "PX")]
        width: Option<u32>,
        /// Page colour behind a transparent drawing
        #[arg(long, value_enum, default_value = "light", value_name = "light|dark")]
        background: Page,
        /// Outline each finding's box and print its rule id beside it
        #[arg(long)]
        annotate: bool,
        /// Show only the matching elements and their surroundings
        #[arg(long, value_name = "SELECTOR")]
        crop: Option<String>,
        /// Replace an existing output file
        #[arg(long)]
        force: bool,
    },
    /// Apply targeted changes by id
    #[command(after_help = "Example:\n  svgdoc edit fig/orders.svg --set '#node-api > rect.fill=#2563eb' --dry-run")]
    Edit {
        /// SVG file; - reads stdin
        #[arg(value_name = "FILE")]
        file: String,
        /// Set one attribute on every element the selector matches; repeatable
        #[arg(long, value_name = "SELECTOR.attr=value")]
        set: Vec<String>,
        /// JSON operations: set, remove, move, rename-id, add-node (see: svgdoc schema ops)
        #[arg(long, value_name = "ops.json|-")]
        ops: Option<String>,
        /// Report the diff and the resulting findings; write nothing
        #[arg(long)]
        dry_run: bool,
        /// Write the result to a new file; - writes the SVG to stdout
        #[arg(short = 'o', value_name = "OUT")]
        output: Option<String>,
        /// Change the input file
        #[arg(long)]
        in_place: bool,
    },
    /// Apply autofixes from findings
    #[command(after_help = "Example:\n  svgdoc fix fig/orders.svg --unsafe --dry-run")]
    Fix {
        /// SVG file; - reads stdin
        #[arg(value_name = "FILE")]
        file: String,
        /// Fix only these rules
        #[arg(long, value_delimiter = ',', value_name = "ID,...")]
        rules: Vec<String>,
        /// Also apply fixes that move or recolour content
        #[arg(long = "unsafe")]
        allow_unsafe: bool,
        /// Report the diff and the resulting findings; write nothing
        #[arg(long)]
        dry_run: bool,
        /// Write the result to a new file; - writes the SVG to stdout
        #[arg(short = 'o', value_name = "OUT")]
        output: Option<String>,
        /// Change the input file
        #[arg(long)]
        in_place: bool,
    },
    /// Serve the commands as Model Context Protocol tools over stdio
    #[command(after_help = "Example:\n  svgdoc mcp --root ./docs")]
    Mcp {
        /// Directory the server may read and write; paths outside it are refused
        #[arg(long, value_name = "DIR")]
        root: PathBuf,
    },
    /// Print a JSON Schema
    #[command(after_help = "Example:\n  svgdoc schema findings")]
    Schema {
        #[arg(value_enum, value_name = "spec|config|findings|scene|ops")]
        name: SchemaName,
    },
    /// List or explain rules
    #[command(after_help = "Example:\n  svgdoc rules --explain text-overflow")]
    Rules {
        /// Show rationale, thresholds and a failing and a passing example
        #[arg(long, value_name = "ID")]
        explain: Option<String>,
    },
}

impl Command {
    pub fn name(&self) -> &'static str {
        match self {
            Command::Render { .. } => "render",
            Command::Validate { .. } => "validate",
            Command::Inspect { .. } => "inspect",
            Command::Preview { .. } => "preview",
            Command::Edit { .. } => "edit",
            Command::Fix { .. } => "fix",
            Command::Mcp { .. } => "mcp",
            Command::Schema { .. } => "schema",
            Command::Rules { .. } => "rules",
        }
    }
}

pub const COMMANDS: &[&str] = &["render", "validate", "inspect", "preview", "edit", "fix", "mcp", "schema", "rules"];
/// Specified commands that are not built yet.
pub const PLANNED: &[&str] = &[];
