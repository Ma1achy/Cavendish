//! A small out-of-process sampling profiler for Windows that needs no administrator rights:
//! every `interval` ms it suspends each thread of the child that used CPU since the last sample
//! (QueryThreadCycleTime), walks its stack with DbgHelp's StackWalk64, and resumes it. Writes
//! folded stacks ("root;...;leaf count") to `out`.
//!
//! Usage: sampler <out.folded> <interval_ms> -- <program> [args...]

use std::collections::HashMap;
use std::io::Write;
use std::mem::zeroed;
use std::os::windows::io::AsRawHandle;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::Diagnostics::Debug::*;
use windows_sys::Win32::System::Diagnostics::ToolHelp::*;
use windows_sys::Win32::System::Threading::*;
use windows_sys::Win32::System::WindowsProgramming::QueryThreadCycleTime;

#[repr(C, align(16))]
struct Ctx(CONTEXT);

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = &args[1];
    let interval: u64 = args[2].parse().unwrap();
    let sep = args.iter().position(|a| a == "--").unwrap();
    let mut child = std::process::Command::new(&args[sep + 1])
        .args(&args[sep + 2..])
        .spawn()
        .expect("spawn");
    let pid = child.id();
    let hproc = child.as_raw_handle() as HANDLE;
    unsafe {
        SetPriorityClass(hproc, BELOW_NORMAL_PRIORITY_CLASS);
    }
    std::thread::sleep(Duration::from_millis(150));
    unsafe {
        SymSetOptions(SYMOPT_DEFERRED_LOADS | SYMOPT_UNDNAME);
        let dir = std::path::Path::new(&args[sep + 1]).parent().map(|p| p.display().to_string()).unwrap_or_default();
        let wide: Vec<u16> = dir.encode_utf16().chain(std::iter::once(0)).collect();
        if SymInitializeW(hproc, wide.as_ptr(), 1) == 0 {
            eprintln!("SymInitialize failed {}", GetLastError());
        }
    }
    let mut threads: HashMap<u32, (HANDLE, u64)> = HashMap::new();
    let mut stacks: HashMap<Vec<u64>, u64> = HashMap::new();
    let mut names: HashMap<u64, String> = HashMap::new();
    let started = Instant::now();
    let mut last_refresh = Instant::now() - Duration::from_secs(10);
    let mut last_symrefresh = Instant::now();
    let mut samples = 0u64;
    let mut idle_skipped = 0u64;
    loop {
        if let Ok(Some(_)) = child.try_wait() {
            break;
        }
        if last_refresh.elapsed() >= Duration::from_millis(20) {
            last_refresh = Instant::now();
            unsafe {
                let snap = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
                let mut te: THREADENTRY32 = zeroed();
                te.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
                if Thread32First(snap, &mut te) != 0 {
                    loop {
                        if te.th32OwnerProcessID == pid && !threads.contains_key(&te.th32ThreadID) {
                            let h = OpenThread(
                                THREAD_SUSPEND_RESUME | THREAD_GET_CONTEXT | THREAD_QUERY_INFORMATION,
                                0,
                                te.th32ThreadID,
                            );
                            if !h.is_null() {
                                threads.insert(te.th32ThreadID, (h, 0));
                            }
                        }
                        if Thread32Next(snap, &mut te) == 0 {
                            break;
                        }
                    }
                }
                CloseHandle(snap);
            }
        }
        if last_symrefresh.elapsed() > Duration::from_secs(2) {
            last_symrefresh = Instant::now();
            unsafe {
                SymRefreshModuleList(hproc);
            }
        }
        let mut dead = vec![];
        for (&tid, (h, cycles)) in threads.iter_mut() {
            unsafe {
                let mut now = 0u64;
                if QueryThreadCycleTime(*h, &mut now) == 0 {
                    dead.push(tid);
                    continue;
                }
                // Our own suspend/resume costs the thread some cycles: count it as running only
                // past 200k cycles (about 50 us at 4 GHz) since the last sample's resume.
                if now.saturating_sub(*cycles) < 200_000 {
                    idle_skipped += 1;
                    continue;
                }
                if SuspendThread(*h) == u32::MAX {
                    dead.push(tid);
                    continue;
                }
                let mut ctx: Ctx = zeroed();
                ctx.0.ContextFlags = 0x0010_000B;
                let mut frames = Vec::with_capacity(48);
                if GetThreadContext(*h, &mut ctx.0) != 0 {
                    let mut sf: STACKFRAME64 = zeroed();
                    sf.AddrPC.Offset = ctx.0.Rip;
                    sf.AddrPC.Mode = AddrModeFlat;
                    sf.AddrFrame.Offset = ctx.0.Rbp;
                    sf.AddrFrame.Mode = AddrModeFlat;
                    sf.AddrStack.Offset = ctx.0.Rsp;
                    sf.AddrStack.Mode = AddrModeFlat;
                    for _ in 0..64 {
                        let ok = StackWalk64(
                            0x8664,
                            hproc,
                            *h,
                            &mut sf,
                            &mut ctx.0 as *mut _ as *mut core::ffi::c_void,
                            None,
                            Some(SymFunctionTableAccess64),
                            Some(SymGetModuleBase64),
                            None,
                        );
                        if ok == 0 || sf.AddrPC.Offset == 0 {
                            break;
                        }
                        frames.push(sf.AddrPC.Offset);
                    }
                }
                ResumeThread(*h);
                let mut after = 0u64;
                QueryThreadCycleTime(*h, &mut after);
                *cycles = after;
                if !frames.is_empty() {
                    *stacks.entry(frames).or_default() += 1;
                    samples += 1;
                }
            }
        }
        for tid in dead {
            if let Some((h, _)) = threads.remove(&tid) {
                unsafe { CloseHandle(h) };
            }
        }
        // Resolve new addresses while the process is alive.
        for stack in stacks.keys() {
            for &a in stack {
                names.entry(a).or_insert_with(|| resolve(hproc, a));
            }
        }
        std::thread::sleep(Duration::from_millis(interval));
    }
    let mut f = std::fs::File::create(out).unwrap();
    for (stack, n) in &stacks {
        let line: Vec<&str> = stack
            .iter()
            .rev()
            .map(|a| names.get(a).map(String::as_str).unwrap_or("?"))
            .collect();
        writeln!(f, "{} {}", line.join(";"), n).unwrap();
    }
    eprintln!(
        "sampler: {samples} samples ({idle_skipped} idle thread-ticks skipped) over {:.1} s",
        started.elapsed().as_secs_f64()
    );
}

fn resolve(hproc: HANDLE, addr: u64) -> String {
    unsafe {
        const MAX: usize = 512;
        let mut buf = vec![0u8; std::mem::size_of::<SYMBOL_INFOW>() + MAX * 2];
        let info = buf.as_mut_ptr() as *mut SYMBOL_INFOW;
        (*info).SizeOfStruct = std::mem::size_of::<SYMBOL_INFOW>() as u32;
        (*info).MaxNameLen = MAX as u32;
        let mut disp = 0u64;
        if SymFromAddrW(hproc, addr, &mut disp, info) != 0 {
            let len = (*info).NameLen as usize;
            let name = std::slice::from_raw_parts((*info).Name.as_ptr(), len);
            String::from_utf16_lossy(name).replace(';', ":").replace(' ', "")
        } else {
            static ONCE: std::sync::Once = std::sync::Once::new();
            ONCE.call_once(|| eprintln!("SymFromAddr failed: {}", GetLastError()));
            let base = SymGetModuleBase64(hproc, addr);
            format!("0x{base:x}+{:x}", addr - base)
        }
    }
}
