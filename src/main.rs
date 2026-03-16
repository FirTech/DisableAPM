use crate::utils::{disable_apm, disk_is_ssd, get_disk_bus_type, get_disk_count};
use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use windows::Win32::Storage::FileSystem::BusTypeUsb;

mod cli;
mod console;
mod utils;

fn main() -> Result<()> {
    // 解析命令行参数
    let cli = cli::Cli::parse();

    // 获取硬盘数量
    let disk_count = get_disk_count().with_context(|| "Failed to get disk count")?;
    console::write_console(
        console::ConsoleType::Info,
        &format!("Total {} disks found", disk_count),
    );

    if let Some(index) = cli.index {
        // 检查硬盘索引是否超出范围
        if index >= disk_count {
            console::write_console(
                console::ConsoleType::Error,
                &format!(
                    "Disk index {} is out of range, max index is {}",
                    index,
                    disk_count - 1
                ),
            );
            return Err(anyhow::anyhow!(
                "Disk index {} is out of range, max index is {}",
                index,
                disk_count - 1
            ));
        }

        // 检查硬盘是否为 SSD
        if disk_is_ssd(index).unwrap_or(false) {
            console::write_console(
                console::ConsoleType::Error,
                &format!("Disk {} is SSD, cannot disable APM", index),
            );
            return Err(anyhow::anyhow!("Disk {} is SSD, cannot disable APM", index));
        }

        match disable_apm(index) {
            Ok(_) => {
                console::write_console(
                    console::ConsoleType::Success,
                    &format!("Disabled APM for disk {}", index),
                );
            }
            Err(e) => {
                console::write_console(
                    console::ConsoleType::Error,
                    &format!("Failed to disable APM for disk {}: {}", index, e),
                );
            }
        };
        return Ok(());
    }

    // 遍历所有硬盘
    for n in 0..disk_count {
        // 检查硬盘是否为 USB 硬盘
        if !cli.usb {
            if get_disk_bus_type(n)
                .with_context(|| format!("Failed to get bus type for disk {}", n))?
                == BusTypeUsb
            {
                console::write_console(
                    console::ConsoleType::Info,
                    &format!("Disk {} is USB, skip disabling APM", n),
                );
                continue;
            }
        }

        // 检查硬盘是否为 SSD
        if disk_is_ssd(n).unwrap_or(false) {
            console::write_console(
                console::ConsoleType::Info,
                &format!("Disk {} is SSD, skip disabling APM", n),
            );
            continue;
        }

        match disable_apm(n) {
            Ok(_) => {
                console::write_console(
                    console::ConsoleType::Success,
                    &format!("Disabled APM for disk {}", n),
                );
            }
            Err(e) => {
                console::write_console(
                    console::ConsoleType::Error,
                    &format!("Failed to disable APM for disk {}: {}", n, e),
                );
            }
        };
    }
    Ok(())
}
