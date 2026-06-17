use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::pipeline::RunOptions;
use crate::schema::{CanvasAspect, ImageProviderKind, StyleName};

#[derive(Debug, Parser)]
#[command(name = "methodfig")]
#[command(about = "Compile paper method descriptions into editable PPTX figures.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Run(RunArgs),
    Doctor,
    Schema(SchemaArgs),
    Resume(ResumeArgs),
}

#[derive(Debug, Args)]
pub struct RunArgs {
    #[arg(long)]
    pub method: PathBuf,
    #[arg(long)]
    pub out: PathBuf,
    #[arg(long, value_enum, default_value_t = StyleArg::WpsClean)]
    pub style: StyleArg,
    #[arg(long, value_enum, default_value_t = AspectArg::PaperWide)]
    pub aspect: AspectArg,
    #[arg(long, default_value_t = 85)]
    pub target_width_mm: u32,
    #[arg(long, default_value_t = 12)]
    pub max_iterations: u32,
    #[arg(long, default_value_t = 3.0)]
    pub max_cost_usd: f64,
    #[arg(long, default_value_t = 20)]
    pub max_minutes: u32,
    #[arg(long, value_enum, default_value_t = ImageProviderArg::Openrouter)]
    pub image_provider: ImageProviderArg,
    #[arg(long)]
    pub mock_models: bool,
    #[arg(long)]
    pub keep_intermediate: bool,
}

impl RunArgs {
    pub fn into_options(self) -> RunOptions {
        RunOptions {
            method_path: self.method,
            out_dir: self.out,
            style: self.style.into(),
            aspect: self.aspect.into(),
            target_width_mm: self.target_width_mm,
            max_iterations: self.max_iterations,
            max_cost_usd: self.max_cost_usd,
            max_minutes: self.max_minutes,
            image_provider: self.image_provider.into(),
            mock_models: self.mock_models,
            keep_intermediate: self.keep_intermediate,
            renderer_timeout: Duration::from_secs(60),
        }
    }
}

#[derive(Debug, Args)]
pub struct ResumeArgs {
    #[arg(long)]
    pub run: PathBuf,
}

#[derive(Debug, Args)]
pub struct SchemaArgs {
    #[arg(long)]
    pub print: bool,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum StyleArg {
    WpsClean,
    CvprClean,
    NeuripsMinimal,
}

impl From<StyleArg> for StyleName {
    fn from(value: StyleArg) -> Self {
        match value {
            StyleArg::WpsClean => StyleName::WpsClean,
            StyleArg::CvprClean => StyleName::CvprClean,
            StyleArg::NeuripsMinimal => StyleName::NeuripsMinimal,
        }
    }
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum AspectArg {
    PaperWide,
    SingleColumn,
    DoubleColumn,
    SixteenNine,
}

impl From<AspectArg> for CanvasAspect {
    fn from(value: AspectArg) -> Self {
        match value {
            AspectArg::PaperWide => CanvasAspect::PaperWide,
            AspectArg::SingleColumn => CanvasAspect::SingleColumn,
            AspectArg::DoubleColumn => CanvasAspect::DoubleColumn,
            AspectArg::SixteenNine => CanvasAspect::SixteenNine,
        }
    }
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum ImageProviderArg {
    Openrouter,
    OpenaiImages,
    Replicate,
    None,
}

impl From<ImageProviderArg> for ImageProviderKind {
    fn from(value: ImageProviderArg) -> Self {
        match value {
            ImageProviderArg::Openrouter => ImageProviderKind::OpenRouter,
            ImageProviderArg::OpenaiImages => ImageProviderKind::OpenAiImages,
            ImageProviderArg::Replicate => ImageProviderKind::Replicate,
            ImageProviderArg::None => ImageProviderKind::None,
        }
    }
}
