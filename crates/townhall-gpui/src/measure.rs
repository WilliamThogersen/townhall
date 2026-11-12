use std::mem::MaybeUninit;
use std::thread;
use std::time::{Duration, Instant};

const SETTLE: Duration = Duration::from_secs(5);

fn cpu_micros() -> u64 {
    let mut usage = MaybeUninit::<libc::rusage>::uninit();
    let rc = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    assert_eq!(rc, 0, "getrusage failed");
    let usage = unsafe { usage.assume_init() };
    let micros = |t: libc::timeval| t.tv_sec as u64 * 1_000_000 + t.tv_usec as u64;
    micros(usage.ru_utime) + micros(usage.ru_stime)
}

#[allow(deprecated)]
fn rss_bytes() -> u64 {
    let mut info = MaybeUninit::<libc::mach_task_basic_info>::uninit();
    let mut count = libc::MACH_TASK_BASIC_INFO_COUNT;
    let rc = unsafe {
        libc::task_info(
            libc::mach_task_self(),
            libc::MACH_TASK_BASIC_INFO,
            info.as_mut_ptr() as libc::task_info_t,
            &mut count,
        )
    };
    assert_eq!(rc, 0, "task_info failed");
    unsafe { info.assume_init() }.resident_size
}

pub fn spawn(seconds: u64) {
    thread::spawn(move || {
        thread::sleep(SETTLE);
        let (cpu_start, wall_start) = (cpu_micros(), Instant::now());
        thread::sleep(Duration::from_secs(seconds));
        let (cpu_end, wall) = (cpu_micros(), wall_start.elapsed());
        let cpu_percent = (cpu_end - cpu_start) as f64 / wall.as_micros() as f64 * 100.0;
        println!(
            "idle {:.0}s after {:.0}s settle: cpu {:.2}% of one core, rss {:.1} MB",
            wall.as_secs_f64(),
            SETTLE.as_secs_f64(),
            cpu_percent,
            rss_bytes() as f64 / 1_048_576.0
        );
        std::process::exit(0);
    });
}
