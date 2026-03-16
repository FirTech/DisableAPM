use clap::Parser;

#[derive(Parser)]
#[clap(version)]
pub struct Cli {
    /// Target a specific disk index
    #[arg(short, long)]
    pub(crate) index: Option<u32>,

    /// Target USB disk
    #[arg(short, long)]
    pub(crate) usb: bool,
}
