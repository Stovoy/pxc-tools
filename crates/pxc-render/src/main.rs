use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "pxc-render", version, about = "Headless Pixel Composer renderer")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Render a project end-to-end to an image.
    Render {
        /// Input .pxc file
        input: PathBuf,
        /// Output image path (png)
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Frame index to render (for animated graphs)
        #[arg(long, default_value_t = 0)]
        frame: u32,
        /// Override preview node id
        #[arg(long)]
        preview_node: Option<String>,
        /// Only validate the project (skip rendering)
        #[arg(long)]
        validate_only: bool,
    },
    /// Validate a project without rendering.
    Validate {
        /// Input .pxc file
        input: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Render {
            input,
            out,
            frame,
            preview_node,
            validate_only,
        } => {
            let config = pxc_render::RenderConfig {
                input,
                output: out,
                frame,
                preview_node,
                validate_only,
            };
            let result = pxc_render::render_project(config)?;
            if let Some(path) = result.output_path {
                println!("Rendered to: {}", path.display());
            }
        }
        Command::Validate { input } => {
            let report = pxc_render::validate_project(&input)?;
            if report.ok {
                println!("Validation OK");
            } else {
                println!("Validation failed with {} errors", report.errors.len());
            }
        }
    }

    Ok(())
}
