# DisableAPM

[简体中文](README.zh.md) | English

A lightweight Windows utility that disables Advanced Power Management (APM) on mechanical hard drives to prevent latency
and noise caused by frequent spin-up/spin-down cycles.

## Background

Does your mechanical hard drive make clicking sounds followed by a high-pitched spin-up noise when accessing files after
a period of inactivity? Do you experience 1-2 second freezes when opening folders or loading game resources stored on
your HDD?

This happens because modern hard drives have APM (Advanced Power Management) enabled by default. When the drive is idle
for a certain period, it spins down to save power. The next time you need to access data, the drive must spin up again,
causing:

- Loud clicking and whirring noises
- 1-2 second freezes/stuttering
- Poor user experience

Common scenarios:

- Opening a folder on the HDD after the computer has been idle
- Loading game assets that haven't been accessed in a few minutes
- Accessing files on a secondary storage drive

**DisableAPM** solves this by disabling APM on your mechanical hard drives, keeping them spinning and ready for instant
access.

> **Note:** SSDs are automatically detected and skipped since they don't have moving parts and don't benefit from APM
> changes.

## Features

- Automatically detects and skips SSDs
- Optionally skips USB external drives
- Support for targeting a specific disk
- Minimal resource usage
- No background service required - run once per boot

## Installation

### Download Pre-built Binary

Download the latest release from the [Releases](../../releases) page.

### Build from Source

```bash
cargo build --release
```

The compiled binary will be located at `target/release/DisableAPM.exe`.

## Usage

### Run with Administrator Privileges

This program requires administrator privileges to send ATA commands to your hard drives.

Right-click `DisableAPM.exe` and select **"Run as administrator"**, or run from an elevated command prompt:

```cmd
DisableAPM.exe
```

### Command Line Options

```
DisableAPM.exe [OPTIONS]

OPTIONS:
    -i, --index <INDEX>    Target a specific disk index (0-based)
    -u, --usb              Include USB external drives
    -h, --help             Print help information
    -V, --version          Print version information
```

### Examples

Disable APM on all mechanical hard drives (skips SSDs and USB drives by default):

```cmd
DisableAPM.exe
```

Disable APM on a specific disk (e.g., disk 2):

```cmd
DisableAPM.exe --index 2
```

Disable APM on all drives including USB external hard drives:

```cmd
DisableAPM.exe --usb
```

### How to Find Your Disk Index

You can find your disk index using Windows Disk Management:

1. Press `Win + X` and select "Disk Management"
2. Look at the left side of each disk (Disk 0, Disk 1, Disk 2, etc.)
3. Use the number shown (0-based index)

Alternatively, use PowerShell:

```powershell
Get-Disk | Select-Object Number, FriendlyName, BusType
```

## How It Works

DisableAPM sends the ATA `SET FEATURES` command (0xEF) with subcommand 0x85 (Disable APM) directly to the hard drive
controller. This instructs the drive to remain spinning and ready for immediate access, eliminating the spin-up delay.

The ATA command used:

- Command Register: `0xEF` (SET FEATURES)
- Features Register: `0x85` (Disable APM)
- Sector Count Register: `0x00` (Disabled completely)

## Compatibility

- **OS:** Windows 10/11 (x64)
- **Drive Types:** SATA HDDs, NVMe drives are skipped automatically
- **USB Drives:** Skipped by default (use `--usb` to include)
- **SSDs:** Automatically detected and skipped

## Limitations

- APM settings are not persistent across reboots - you need to run this tool after each boot
- Some USB enclosures may not support ATA pass-through commands
- RAID volumes may not respond to APM commands depending on the controller

## Auto-run on Startup

To automatically disable APM on every boot:

1. Press `Win + R`, type `shell:startup` and press Enter
2. Create a shortcut to `DisableAPM.exe`
3. Right-click the shortcut → Properties → Shortcut tab → Advanced
4. Check "Run as administrator" and click OK

Alternatively, use Task Scheduler:

1. Open Task Scheduler (`taskschd.msc`)
2. Create a new task triggered by "At startup"
3. Set the action to run `DisableAPM.exe` with highest privileges

## Safety

- Only mechanical hard drives are affected
- SSDs are automatically detected and skipped
- USB drives are skipped by default
- The tool only sends the APM disable command and does not modify any data

## License

MIT License

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.
