use clap::Parser;

#[derive(Parser)]
#[clap(version)]
pub struct Cli {
    /// Install the service
    #[arg(long)]
    pub(crate) install: bool,
    /// Uninstall the service
    #[arg(long)]
    pub(crate) uninstall: bool,
    /// Target a specific disk index
    #[arg(short, long)]
    pub(crate) index: Option<u32>,
    /// Target USB disk
    #[arg(short, long)]
    pub(crate) usb: bool,
}
