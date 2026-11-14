//! Command line surface. Every --help stays short and carries one example.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser, Debug)]
#[command(
    name = "svgdog",
    version,
    about = "Compile and check SVG diagrams for technical documentation.",
    disable_help_subcommand = true,
    after_help = "Example:\n  svgdog validate docs/fig/orders.svg --format json"
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
    /// Config file to use [default: nearest svgdog.toml above the input file]
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

/// How much a rendered drawing responds to its reader.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Live {
    None,
    Css,
    Html,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum SchemaName {
    Spec,
    Config,
    Findings,
    Scene,
    Ops,
    Theme,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Compile a spec to SVG and validate the result
    #[command(after_help = "Example:\n  svgdog render fig/orders.yaml -o fig/orders.svg")]
    Render {
        /// Diagram spec, YAML or JSON; - reads stdin
        #[arg(value_name = "SPEC")]
        spec: String,
        /// Where to write the SVG; - writes it to stdout in place of the report
        #[arg(short = 'o', value_name = "OUT")]
        output: String,
        /// Theme: docs, blueprint, paper, bold or cards (add -dark or -auto), or a theme folder [default: the spec's, else svgdog.toml's, else docs-light]
        #[arg(long, value_name = "NAME|DIR")]
        theme: Option<String>,
        /// css: an SVG that highlights on hover; html: a page with step controls [default: none]
        #[arg(long, value_enum, default_value = "none", value_name = "none|css|html", hide_possible_values = true, hide_default_value = true)]
        interactive: Live,
        /// Write one picture per step (OUT.1.svg, OUT.2.svg, ...), each with its step in focus
        #[arg(long)]
        frames: bool,
        /// Store the spec inside the SVG, so the drawing can be read back and edited
        #[arg(long)]
        embed_spec: bool,
        /// Write nothing; exit 1 if OUT is missing or is not what this spec renders now
        #[arg(long, conflicts_with = "force")]
        check: bool,
        /// An SVG rendered earlier: boxes in both stay where they were (may be OUT itself)
        #[arg(long, value_name = "SVG")]
        previous: Option<String>,
        /// Skip the checks on the result
        #[arg(long)]
        no_validate: bool,
        /// Replace an existing output file; with --previous, lay out afresh if the layout cannot be kept
        #[arg(long)]
        force: bool,
        /// Convert text to outlines: exact everywhere, but not searchable
        #[arg(long)]
        text_as_paths: bool,
    },
    /// Run rules on one or more SVG files
    #[command(after_help = "Example:\n  svgdog validate 'docs/**/*.svg' --fail-on warning")]
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
    #[command(after_help = "Example:\n  svgdog inspect fig/orders.svg --select '#node-api' --fields id,tag,bbox,text")]
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
    #[command(after_help = "Example:\n  svgdog preview fig/orders.svg -o /tmp/orders.png --annotate")]
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
        /// Show the drawing with this step of its walk-through chosen, the rest faded
        #[arg(long, value_name = "N")]
        step: Option<usize>,
        /// Replace an existing output file
        #[arg(long)]
        force: bool,
    },
    /// Apply targeted changes by id
    #[command(after_help = "Example:\n  svgdog edit fig/orders.svg --set '#node-api > rect.fill=#2563eb' --dry-run")]
    Edit {
        /// SVG file; - reads stdin
        #[arg(value_name = "FILE")]
        file: String,
        /// Set one attribute on every element the selector matches; repeatable
        #[arg(long, value_name = "SELECTOR.attr=value")]
        set: Vec<String>,
        /// JSON operations: set, remove, move, rename-id, add-node (see: svgdog schema ops)
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
    #[command(after_help = "Example:\n  svgdog fix fig/orders.svg --unsafe --dry-run")]
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
    #[command(after_help = "Example:\n  svgdog mcp --root-path ./docs")]
    Mcp {
        /// Directory the server may read and write; paths outside it are refused
        #[arg(long, value_name = "DIR")]
        root_path: PathBuf,
    },
    /// Print a JSON Schema
    #[command(after_help = "Example:\n  svgdog schema findings")]
    Schema {
        #[arg(value_enum, value_name = "spec|config|findings|scene|ops|theme")]
        name: SchemaName,
    },
    /// List or explain rules
    #[command(after_help = "Example:\n  svgdog rules --explain text-overflow")]
    Rules {
        /// Show rationale, thresholds and a failing and a passing example
        #[arg(long, value_name = "ID")]
        explain: Option<String>,
    },
    /// Render the diagrams written inside Markdown pages
    #[command(after_help = "Example:\n  svgdog docs docs/ --check")]
    Docs {
        /// Markdown files, or folders to search for them
        #[arg(required = true, value_name = "PATH")]
        paths: Vec<String>,
        /// Write nothing; exit 1 if any picture or image link is out of date
        #[arg(long)]
        check: bool,
        /// Folder for the pictures, next to each page
        #[arg(long, default_value = "diagrams", value_name = "DIR")]
        assets: String,
        /// Theme for every diagram: a built-in name or a theme folder [default: each spec's]
        #[arg(long, value_name = "NAME|DIR")]
        theme: Option<String>,
        /// Lay a diagram out afresh when its boxes cannot stay where the picture on disk has them
        #[arg(long, conflicts_with = "check")]
        force: bool,
        /// css: the pictures highlight on hover where the page inlines them; html: also write a page per diagram and link it
        #[arg(long, value_enum, default_value = "none", value_name = "none|css|html", hide_possible_values = true, hide_default_value = true)]
        interactive: Live,
    },
    /// Say in words what a diagram shows
    #[command(after_help = "Example:\n  svgdog describe fig/orders.yaml --format text")]
    Describe {
        /// A diagram spec, or an SVG rendered with --embed-spec; - reads stdin
        #[arg(value_name = "FILE")]
        file: String,
        /// Print one or two sentences, as for alt text
        #[arg(long)]
        short: bool,
    },
    /// List the bundled example specs, or print one
    #[command(after_help = "Example:\n  svgdog examples flow --format text > fig/first.yaml")]
    Examples {
        /// Example to print [default: list them]
        #[arg(value_name = "NAME")]
        name: Option<String>,
    },
    /// List the built-in themes, start a theme of your own, or check one
    #[command(after_help = "Example:\n  svgdog theme init themes/acme --from paper")]
    Theme {
        #[command(subcommand)]
        action: ThemeAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum ThemeAction {
    /// List the built-in themes
    List,
    /// Write a theme.toml to start from into a folder
    #[command(after_help = "Example:\n  svgdog theme init themes/acme --from paper")]
    Init {
        /// Folder for the theme; it is created when missing
        #[arg(value_name = "DIR")]
        dir: String,
        /// Built-in theme to copy: docs, blueprint, paper, bold or cards
        #[arg(long, default_value = "docs", value_name = "NAME")]
        from: String,
        /// Replace a theme.toml that is already there
        #[arg(long)]
        force: bool,
    },
    /// Check a theme folder: the file, the contrast of its colours, and the bundled examples drawn with it
    #[command(after_help = "Example:\n  svgdog theme check themes/acme")]
    Check {
        /// Folder that holds the theme.toml
        #[arg(value_name = "DIR")]
        dir: String,
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
            Command::Examples { .. } => "examples",
            Command::Describe { .. } => "describe",
            Command::Docs { .. } => "docs",
            Command::Theme { .. } => "theme",
        }
    }
}

pub const COMMANDS: &[&str] = &["render", "validate", "inspect", "preview", "edit", "fix", "mcp", "schema", "rules", "examples", "describe", "docs", "theme"];
/// Specified commands that are not built yet.
pub const PLANNED: &[&str] = &[];
