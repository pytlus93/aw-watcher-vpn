//! Host-owned action children use a kill-on-close job, assigned before execution.
#[cfg(windows)]
mod windows {
    use super::super::*;
    use std::{
        ffi::c_void,
        os::windows::{io::AsRawHandle, process::CommandExt},
    };
    type Handle = *mut c_void;
    #[repr(C)]
    #[derive(Default)]
    struct Basic {
        process_time: i64,
        job_time: i64,
        flags: u32,
        min_work: usize,
        max_work: usize,
        active: u32,
        affinity: usize,
        priority: u32,
        scheduling: u32,
    }
    #[repr(C)]
    #[derive(Default)]
    struct Extended {
        basic: Basic,
        io: [u64; 6],
        process_memory: usize,
        job_memory: usize,
        peak_process: usize,
        peak_job: usize,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn CreateJobObjectW(attributes: *const c_void, name: *const u16) -> Handle;
        fn SetInformationJobObject(job: Handle, class: u32, info: *const c_void, size: u32) -> i32;
        fn AssignProcessToJobObject(job: Handle, process: Handle) -> i32;
        fn CloseHandle(handle: Handle) -> i32;
    }
    #[link(name = "ntdll")]
    extern "system" {
        fn NtResumeProcess(process: Handle) -> i32;
    }
    struct Job(Handle);
    impl Drop for Job {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    fn spawn(script: &str) -> Result<(Job, std::process::Child), String> {
        let job = Job(unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) });
        if job.0.is_null() {
            return Err("Cannot create owned helper job".into());
        }
        let mut limits = Extended::default();
        limits.basic.flags = 0x2000;
        if unsafe {
            SetInformationJobObject(
                job.0,
                9,
                &limits as *const _ as *const c_void,
                std::mem::size_of::<Extended>() as u32,
            )
        } == 0
        {
            return Err("Cannot configure helper ownership".into());
        }
        let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let exe = std::path::PathBuf::from(
            std::env::var_os("SystemRoot").ok_or("SystemRoot unavailable")?,
        )
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let mut child = Command::new(exe)
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-STA",
                "-EncodedCommand",
                &base64::engine::general_purpose::STANDARD.encode(bytes),
            ])
            .creation_flags(0x08000004)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        if unsafe { AssignProcessToJobObject(job.0, child.as_raw_handle()) } == 0
            || unsafe { NtResumeProcess(child.as_raw_handle()) } < 0
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Cannot attach or resume owned helper".into());
        }
        Ok((job, child))
    }
    pub fn run(script: &str) -> Result<(), String> {
        let (_job, mut child) = spawn(script)?;
        let status = child.wait().map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err("Action UI helper failed".into())
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn job_close_terminates_noninteractive_child() {
            let (job, mut child) = spawn("Start-Sleep -Seconds 30").unwrap();
            drop(job);
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                if child.try_wait().unwrap().is_some() {
                    break;
                }
                if Instant::now() > deadline {
                    let _ = child.kill();
                    panic!("Owned helper survived job close")
                }
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
}
#[cfg(windows)]
pub use windows::run;
#[cfg(not(windows))]
pub fn run(_script: &str) -> Result<(), String> {
    Err("Action UI requires Windows".into())
}
