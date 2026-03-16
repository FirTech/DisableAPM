use crate::console::ConsoleType;
use crate::utils::{disable_apm, disk_is_ssd, get_disk_bus_type, get_disk_count};
use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use std::{env, fs};
use windows::Win32::Storage::FileSystem::BusTypeUsb;

mod cli;
mod console;
mod utils;

fn main() -> Result<()> {
    // 解析命令行参数
    let cli = cli::Cli::parse();

    // 检查是否安装服务
    if cli.install {
        return install_service(&cli);
    }

    // 检查是否卸载服务
    if cli.uninstall {
        return uninstall_service();
    }

    // 获取硬盘数量
    let disk_count = get_disk_count().with_context(|| "Failed to get disk count")?;
    console::write_console(
        ConsoleType::Info,
        &format!("Total {} disks found", disk_count),
    );

    if let Some(index) = cli.index {
        // 检查硬盘索引是否超出范围
        if index >= disk_count {
            console::write_console(
                ConsoleType::Error,
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
                ConsoleType::Error,
                &format!("Disk {} is SSD, cannot disable APM", index),
            );
            return Err(anyhow::anyhow!("Disk {} is SSD, cannot disable APM", index));
        }

        return disable_apm_for_disk(index);
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
                    ConsoleType::Info,
                    &format!("Disk {} is USB, skip disabling APM", n),
                );
                continue;
            }
        }

        // 检查硬盘是否为 SSD
        if disk_is_ssd(n).unwrap_or(false) {
            console::write_console(
                ConsoleType::Info,
                &format!("Disk {} is SSD, skip disabling APM", n),
            );
            continue;
        }
        let _ = disable_apm_for_disk(n);
    }
    Ok(())
}

fn disable_apm_for_disk(index: u32) -> Result<()> {
    match disable_apm(index) {
        Ok(_) => {
            console::write_console(
                ConsoleType::Success,
                &format!("Disabled APM for disk {}", index),
            );
            Ok(())
        }
        Err(e) => {
            console::write_console(
                ConsoleType::Error,
                &format!("Failed to disable APM for disk {}: {}", index, e),
            );
            Err(e)
        }
    }
}

fn install_service(cli: &cli::Cli) -> Result<()> {
    console::write_console(ConsoleType::Info, "Installing service......");

    // 确定目标路径
    let program_files =
        env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".to_string());
    let target_dir = PathBuf::from(program_files).join("DisableAPM");
    if !target_dir.exists() {
        fs::create_dir_all(&target_dir)
            .context("Failed to create install directory (check admin privileges)")?;
    }

    let target_exe = target_dir.join("DisableAPM.exe");
    let current_exe = env::current_exe().context("Failed to get current exe path")?;
    fs::copy(&current_exe, &target_exe).context("Failed to copy program to system directory")?;

    console::write_console(
        ConsoleType::Info,
        &format!("Program installed to: {}", target_exe.display()),
    );

    let mut args: Vec<String> = vec![];
    if let Some(index) = cli.index {
        args.push(format!("--index {}", index).to_string());
    }
    if cli.usb {
        args.push("--usb".to_string());
    }

    match utils::install_task(
        "DisableAPM",
        "Disable APM for mechanical disks",
        &target_exe.to_string_lossy().to_string(),
        &args,
    ) {
        Ok(_) => {
            console::write_console(ConsoleType::Success, "Service installed successfully");
            Ok(())
        }
        Err(e) => {
            console::write_console(
                ConsoleType::Error,
                &format!("Failed to install service: {}", e),
            );
            Err(e)
        }
    }
}

fn uninstall_service() -> Result<()> {
    // 检查任务是否已安装
    if !utils::is_task_installed("DisableAPM").unwrap_or(false) {
        console::write_console(ConsoleType::Info, "Service not installed");
        return Ok(());
    }
    console::write_console(ConsoleType::Info, "Uninstalling service......");

    let program_files =
        env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".to_string());
    let target_dir = PathBuf::from(program_files).join("DisableAPM");
    if target_dir.exists() {
        console::write_console(
            ConsoleType::Info,
            &format!("Program removed from: {}", target_dir.display()),
        );
        fs::remove_dir_all(&target_dir)
            .context("Failed to remove install directory (check admin privileges)")?;
    }

    match utils::uninstall_task("DisableAPM") {
        Ok(_) => {
            console::write_console(ConsoleType::Success, "Service uninstalled successfully");
            Ok(())
        }
        Err(e) => {
            console::write_console(
                ConsoleType::Error,
                &format!("Failed to uninstall service: {}", e),
            );
            Err(e)
        }
    }
}
