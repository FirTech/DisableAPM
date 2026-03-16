use anyhow::Result;
use std::mem;
use windows::core::{Interface, BSTR, HSTRING, PCWSTR};
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW, DIGCF_DEVICEINTERFACE,
    DIGCF_PRESENT, SP_DEVICE_INTERFACE_DATA,
};
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_NO_MORE_ITEMS, INVALID_HANDLE_VALUE, VARIANT_TRUE,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, ReadFile, SetFilePointer, FILE_ATTRIBUTE_NORMAL, FILE_BEGIN,
    FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, STORAGE_BUS_TYPE,
};
use windows::Win32::Storage::IscsiDisc::{ATA_PASS_THROUGH_EX, IOCTL_ATA_PASS_THROUGH};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::System::Ioctl::{
    PropertyStandardQuery, StorageDeviceProperty, StorageDeviceSeekPenaltyProperty,
    DEVICE_SEEK_PENALTY_DESCRIPTOR, GUID_DEVINTERFACE_DISK, IOCTL_STORAGE_QUERY_PROPERTY,
    STORAGE_DEVICE_DESCRIPTOR, STORAGE_PROPERTY_QUERY,
};
use windows::Win32::System::TaskScheduler::{
    IExecAction, ISessionStateChangeTrigger, ITaskDefinition, ITaskService, TaskScheduler,
    TASK_ACTION_EXEC, TASK_CREATE_OR_UPDATE, TASK_LOGON_INTERACTIVE_TOKEN,
    TASK_LOGON_SERVICE_ACCOUNT, TASK_RUNLEVEL_HIGHEST, TASK_SESSION_UNLOCK, TASK_TRIGGER_SESSION_STATE_CHANGE,
};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::System::IO::DeviceIoControl;

#[repr(C)]
struct AtaPassThroughExWithBuffers {
    apt: ATA_PASS_THROUGH_EX,
    filler: u32,
    buf: [u8; 512],
}

pub fn disable_apm(disk_index: u32) -> Result<()> {
    let device_name = HSTRING::from(format!("\\\\.\\PhysicalDrive{}", disk_index));

    // 第一次打开设备：进行预读（唤醒硬盘）
    let handle = unsafe {
        CreateFileW(
            PCWSTR(device_name.as_ptr()),
            FILE_GENERIC_READ.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None, // htemplatefile: Option<HANDLE>
        )?
    };

    if handle == INVALID_HANDLE_VALUE {
        return Err(windows::core::Error::from_thread().into());
    }

    let mut buf = [0u8; 512];
    let mut read_size = 0u32;
    unsafe {
        SetFilePointer(handle, 0, None, FILE_BEGIN);
        ReadFile(handle, Some(&mut buf), Some(&mut read_size), None)?;
        let _ = CloseHandle(handle);
    }

    // 第二次打开设备：准备发送 IOCTL 命令
    let handle = unsafe {
        CreateFileW(
            PCWSTR(device_name.as_ptr()),
            FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None, // lpsecurityattributes: Option<*const SECURITY_ATTRIBUTES>
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None, // htemplatefile: Option<HANDLE>
        )?
    };

    if handle == INVALID_HANDLE_VALUE {
        return Err(windows::core::Error::from_thread().into());
    }

    // 初始化结构体
    let mut ab: AtaPassThroughExWithBuffers = unsafe { mem::zeroed() };
    let header_size = mem::size_of::<ATA_PASS_THROUGH_EX>() as u16;
    let total_size = mem::size_of::<AtaPassThroughExWithBuffers>() as u32;

    ab.apt.Length = header_size;
    ab.apt.TimeOutValue = 3;
    // 设置缓冲区偏移量
    ab.apt.DataBufferOffset =
        (mem::size_of::<ATA_PASS_THROUGH_EX>() + mem::size_of::<u32>()) as usize;

    // 设置 ATA 寄存器
    // CurrentTaskFile[0]: Features Register -> 0x85 (Disable APM)
    ab.apt.CurrentTaskFile[0] = 0x85;
    // CurrentTaskFile[1]: Sector Count Register -> 0
    ab.apt.CurrentTaskFile[1] = 0x00;
    // CurrentTaskFile[6]: Command Register -> 0xEF (Set Features)
    ab.apt.CurrentTaskFile[6] = 0xEF;

    let mut bytes_returned = 0u32;

    unsafe {
        let result = DeviceIoControl(
            handle,
            IOCTL_ATA_PASS_THROUGH,
            Some(&ab as *const _ as _),
            total_size,
            Some(&mut ab as *mut _ as *mut _),
            total_size,
            Some(&mut bytes_returned),
            None,
        );
        let _ = CloseHandle(handle);
        Ok(result?)
    }
}

/// 获取系统中所有物理磁盘的数量
///
/// # 返回值
/// - `Ok(u32)`: 成功获取物理磁盘数量
/// - `Err(anyhow::Error)`: 获取失败，返回错误信息
pub fn get_disk_count() -> Result<u32> {
    let mut count = 0;

    unsafe {
        // 获取系统中所有磁盘接口设备的句柄列表 (Device Information Set)
        let device_info_set = SetupDiGetClassDevsW(
            Some(&GUID_DEVINTERFACE_DISK),
            None,
            None,
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        )?;

        // 循环枚举接口
        let mut device_interface_data: SP_DEVICE_INTERFACE_DATA = std::mem::zeroed();
        device_interface_data.cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32;

        let mut index = 0;
        loop {
            // 尝试获取第 index 个设备接口
            let result = SetupDiEnumDeviceInterfaces(
                device_info_set,
                None,
                &GUID_DEVINTERFACE_DISK,
                index,
                &mut device_interface_data,
            );

            match result {
                Ok(_) => {
                    count += 1;
                    index += 1;
                }
                Err(_) => {
                    let err = GetLastError();
                    if err == ERROR_NO_MORE_ITEMS {
                        // 枚举结束
                        break;
                    }
                    // 其他错误（如权限问题）通常也意味着没有更多可读设备
                    break;
                }
            }
        }

        // 释放资源
        let _ = SetupDiDestroyDeviceInfoList(device_info_set);
    }
    Ok(count)
}

/// 检查是否为 SSD (通过检测是否存在寻道时间)
///
/// # 参数
/// - `disk_index`: 要检查的磁盘索引（例如 0 表示 C:）
///
/// # 返回值
/// - `Ok(bool)`: 成功检查，返回是否为 SSD（true 为 SSD，false 为 HDD）
/// - `Err(anyhow::Error)`: 检查失败，返回错误信息
pub fn disk_is_ssd(disk_index: u32) -> Result<bool> {
    let device_name = HSTRING::from(format!("\\\\.\\PhysicalDrive{}", disk_index));

    // 以读写方式打开设备
    let handle = unsafe {
        CreateFileW(
            PCWSTR(device_name.as_ptr()),
            FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None, // lpsecurityattributes: Option<*const SECURITY_ATTRIBUTES>
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None, // htemplatefile: Option<HANDLE>
        )?
    };

    if handle == INVALID_HANDLE_VALUE {
        return Err(windows::core::Error::from_thread().into());
    }

    let query = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageDeviceSeekPenaltyProperty,
        QueryType: PropertyStandardQuery,
        ..Default::default()
    };

    let mut descriptor = DEVICE_SEEK_PENALTY_DESCRIPTOR::default();
    let mut bytes_returned = 0u32;

    unsafe {
        let result = DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            Some(&query as *const _ as _),
            size_of_val(&query) as _,
            Some(&mut descriptor as *mut _ as *mut _),
            std::mem::size_of::<DEVICE_SEEK_PENALTY_DESCRIPTOR>() as u32,
            Some(&mut bytes_returned),
            None,
        );
        let _ = CloseHandle(handle);
        result?;
    };

    if bytes_returned != std::mem::size_of::<DEVICE_SEEK_PENALTY_DESCRIPTOR>() as u32 {
        return Err(windows::core::Error::from_thread().into());
    }

    // 如果 IncursSeekPenalty 为 false (0)，说明没有寻道时间，通常被认为是 SSD
    Ok(!descriptor.IncursSeekPenalty)
}

/// 获取指定盘符设备的 BusType（返回值为 u8），失败返回 None
///
/// [参考文档](https://learn.microsoft.com/zh-cn/windows/win32/api/winioctl/ne-winioctl-storage_bus_type)
///
/// # 参数
/// - `disk_index`: 要检查的磁盘索引（例如 0 表示 C:）
///
/// # 返回值
/// - `Ok(STORAGE_BUS_TYPE)`: 成功获取 BusType，返回值为 STORAGE_BUS_TYPE 类型
/// - `Err(anyhow::Error)`: 获取失败，返回错误信息
pub fn get_disk_bus_type(disk_index: u32) -> Result<STORAGE_BUS_TYPE> {
    let device_name = HSTRING::from(format!("\\\\.\\PhysicalDrive{}", disk_index));

    // 以读写方式打开设备
    let handle = unsafe {
        CreateFileW(
            PCWSTR(device_name.as_ptr()),
            FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None, // lpsecurityattributes: Option<*const SECURITY_ATTRIBUTES>
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None, // htemplatefile: Option<HANDLE>
        )?
    };

    if handle == INVALID_HANDLE_VALUE {
        return Err(anyhow::anyhow!("CreateFileW failed: INVALID_HANDLE_VALUE"));
    }

    let query = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageDeviceProperty,
        QueryType: PropertyStandardQuery,
        ..Default::default()
    };

    let mut buffer = vec![0u8; 512];
    let mut returned = 0u32;

    unsafe {
        let result = DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            Some(&query as *const _ as _),
            size_of_val(&query) as _,
            Some(buffer.as_mut_ptr() as _),
            buffer.len() as _,
            Some(&mut returned),
            None,
        );
        let _ = CloseHandle(handle);
        result?;
    }

    let desc = unsafe { &*(buffer.as_ptr() as *const STORAGE_DEVICE_DESCRIPTOR) };
    Ok(desc.BusType)
}

/// 安装任务计划程序任务
///
/// # 参数
/// - `name`: 任务名称
/// - `description`: 任务描述
/// - `exe_path`: 要执行的程序路径
/// - `args`: 要传递给程序的参数列表
///
/// # 返回值
/// - `Ok(())`: 成功安装任务
/// - `Err(anyhow::Error)`: 安装失败，返回错误信息
pub fn install_task(name: &str, description: &str, exe_path: &str, args: &[String]) -> Result<()> {
    unsafe {
        // 初始化 COM 组件
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;

        // 创建 TaskService 实例
        let service: ITaskService = CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)?;
        service.Connect(
            &VARIANT::default(),
            &VARIANT::default(),
            &VARIANT::default(),
            &VARIANT::default(),
        )?;

        // 创建一个新的任务定义
        let task_definition: ITaskDefinition = service.NewTask(0)?;

        // 设置基本信息
        let reg_info = task_definition.RegistrationInfo()?;
        reg_info.SetDescription(&BSTR::from(description))?;

        // 设置安全上下文 (以最高权限运行)
        let principal = task_definition.Principal()?;
        principal.SetRunLevel(TASK_RUNLEVEL_HIGHEST)?;
        // 指定为 SYSTEM 账户
        principal.SetUserId(&BSTR::from("SYSTEM"))?;
        // 设置为服务账户登录模式
        principal.SetLogonType(TASK_LOGON_SERVICE_ACCOUNT)?;

        // 设置设置选项 (电源条件)
        let settings = task_definition.Settings()?;
        // 只有在交流电时启动
        settings.SetDisallowStartIfOnBatteries(VARIANT_TRUE)?;
        // 切换到电池时停止
        settings.SetStopIfGoingOnBatteries(VARIANT_TRUE)?;
        // 任务永不超时
        settings.SetExecutionTimeLimit(&BSTR::from("PT0S"))?;
        settings.SetStartWhenAvailable(VARIANT_TRUE)?;

        // 设置触发器 (工作站解锁时)
        let triggers = task_definition.Triggers()?;
        let trigger = triggers.Create(TASK_TRIGGER_SESSION_STATE_CHANGE)?;
        let session_trigger: ISessionStateChangeTrigger = trigger.cast()?;
        session_trigger.SetStateChange(TASK_SESSION_UNLOCK)?; // 设置为工作站解锁时

        // 设置动作 (执行程序)
        let actions = task_definition.Actions()?;
        let action = actions.Create(TASK_ACTION_EXEC)?;
        let exec_action: IExecAction = action.cast()?;

        exec_action.SetPath(&BSTR::from(exe_path))?;
        exec_action.SetArguments(&BSTR::from(args.join(" ")))?;

        // 注册任务
        let folder = service.GetFolder(&BSTR::from("\\"))?;
        folder.RegisterTaskDefinition(
            &BSTR::from(name),
            &task_definition,
            TASK_CREATE_OR_UPDATE.0,
            &VARIANT::default(),
            &VARIANT::default(),
            TASK_LOGON_INTERACTIVE_TOKEN,
            &VARIANT::default(),
        )?;
    }

    Ok(())
}

/// 卸载任务计划程序任务
///
/// # 参数
/// - `name`: 任务名称
///
/// # 返回值
/// - `Ok(())`: 成功卸载任务
/// - `Err(anyhow::Error)`: 卸载失败，返回错误信息
pub fn uninstall_task(name: &str) -> Result<()> {
    unsafe {
        // 初始化 COM 组件
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;

        // 创建 TaskService 实例
        let service: ITaskService = CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)?;
        service.Connect(
            &VARIANT::default(),
            &VARIANT::default(),
            &VARIANT::default(),
            &VARIANT::default(),
        )?;

        // 删除任务
        let folder = service.GetFolder(&BSTR::from("\\"))?;
        folder.DeleteTask(&BSTR::from(name), 0)?;
    }

    Ok(())
}

/// 检查任务计划程序任务是否已安装
///
/// # 参数
/// - `name`: 任务名称
///
/// # 返回值
/// - `Ok(true)`: 任务已安装
/// - `Ok(false)`: 任务未安装
/// - `Err(anyhow::Error)`: 检查失败，返回错误信息
pub fn is_task_installed(name: &str) -> Result<bool> {
    unsafe {
        // 初始化 COM 组件
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;

        // 创建 TaskService 实例
        let service: ITaskService = CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)?;
        service.Connect(
            &VARIANT::default(),
            &VARIANT::default(),
            &VARIANT::default(),
            &VARIANT::default(),
        )?;

        // 检查任务是否存在
        let folder = service.GetFolder(&BSTR::from("\\"))?;
        let task = folder.GetTask(&BSTR::from(name));
        Ok(task.is_ok())
    }
}
