use console::style;
use std::cmp::PartialEq;

#[derive(PartialEq)]
pub enum ConsoleType {
    /// 普通信息
    Info,
    /// 成功信息
    Success,
    /// 警告信息
    Warning,
    /// 错误信息
    Error,
    /// 调试信息
    Debug,
}

/// 写入控制台
///
/// # 参数
/// - `consoleType`: 控制台类型
/// - `message`: 控制台消息
///
/// # 返回值
/// - `Ok(())`: 写入成功
pub fn write_console(console_type: ConsoleType, message: &str) {
    let title = match &console_type {
        ConsoleType::Info => style("Info").cyan(),
        ConsoleType::Success => style("Success").green(),
        ConsoleType::Warning => style("Warning").yellow(),
        ConsoleType::Error => style("Error").red().on_black().bold(),
        ConsoleType::Debug => style("Debug").blue(),
    };

    if console_type == ConsoleType::Error {
        eprintln!("  {}      {}", &title, message);
    } else {
        println!("  {}      {}", &title, message);
    }
}
