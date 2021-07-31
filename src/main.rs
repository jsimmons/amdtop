use std::{
    collections::HashMap,
    fmt::Display,
    fs::File,
    io::{self, BufRead},
    path::Path,
};

fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

#[inline]
fn checked_log(x: u64, base: u64) -> Option<u64> {
    if x <= 0 || base <= 1 {
        None
    } else {
        let mut n = 0;
        let mut r = x;
        while r >= base {
            r /= base;
            n += 1;
        }
        Some(n)
    }
}

#[inline]
fn log(x: u64, base: u64) -> u64 {
    match checked_log(x, base) {
        Some(n) => n,
        None => 0,
    }
}

fn main() -> Result<(), io::Error> {
    let gpu_index = 0;
    let gem_info_path = format!("/sys/kernel/debug/dri/{}/amdgpu_gem_info", gpu_index);

    #[derive(Default, Copy, Clone)]
    struct MemInfo {
        pid: i32,
        gtt_bytes: u64,
        vram_bytes: u64,
        unknown_bytes: u64,
    }

    let mut mem_infos = HashMap::<i32, MemInfo>::new();
    let mut cur_pid = -1;

    let mut process_line = |line: &str| -> Option<()> {
        let mut segments = line.split_whitespace();
        match segments.next()? {
            "pid" => {
                let pid = segments.next()?;
                if let Ok(pid) = pid.parse() {
                    cur_pid = pid;
                }
            }
            _ => {
                let bytes = str::parse::<u64>(segments.next()?).ok()?;
                let _skip = segments.next()?;
                let memory_type = segments.next()?;
                let mem_info = mem_infos.entry(cur_pid).or_default();
                match memory_type {
                    "VRAM" => mem_info.vram_bytes += bytes,
                    "GTT" => mem_info.gtt_bytes += bytes,
                    _ => mem_info.unknown_bytes += bytes,
                }
            }
        }

        Some(())
    };

    for line in read_lines(gem_info_path)? {
        process_line(&line?);
    }

    let mut mem_infos_sorted = mem_infos
        .iter()
        .map(|(pid, mem_info)| MemInfo {
            pid: *pid,
            ..*mem_info
        })
        .collect::<Vec<_>>();

    mem_infos_sorted
        .sort_by_key(|mem_info| std::cmp::Reverse(mem_info.vram_bytes + mem_info.gtt_bytes));

    struct FormatBytes {
        bytes: u64,
    }
    impl FormatBytes {
        fn new(bytes: u64) -> Self {
            Self { bytes }
        }
    }
    impl Display for FormatBytes {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            const DIVISOR: u64 = 1024;
            const SUFFIXES: &[&'static str] = &["", "KiB", "MiB", "GiB"];

            if self.bytes == 0 {
                return self.bytes.fmt(f);
            }

            let divisions = std::cmp::min(log(self.bytes, DIVISOR), SUFFIXES.len() as u64);
            let result = self.bytes as f64 / DIVISOR.pow(divisions as u32) as f64;
            format!("{:.2} {}", result, SUFFIXES[divisions as usize]).fmt(f)
        }
    }

    println!(
        "{0: <10} | {1: <20} | {2: <40} | {3: >15} | {4: >15} | {5: >15}",
        "PID", "PROCESS", "PATH", "TOTAL", "VRAM", "GTT"
    );

    println!("{:-^1$}", "", 130);

    for mem_info in mem_infos_sorted {
        if mem_info.pid == 0 {
            continue;
        }

        let path = std::fs::read_link(format!("/proc/{}/exe", mem_info.pid))
            .map(|path| path.to_string_lossy().to_owned().to_string());
        let name = std::fs::read_to_string(format!("/proc/{}/comm", mem_info.pid));

        println!(
            "{0: <10} | {1: <20} | {2: <40} | {3: >15} | {4: >15} | {5: >15}",
            mem_info.pid,
            name.as_ref()
                .map(String::as_str)
                .map(str::trim)
                .unwrap_or("unknown"),
            path.as_ref()
                .map(String::as_str)
                .map(str::trim)
                .unwrap_or("unknown"),
            FormatBytes::new(mem_info.vram_bytes + mem_info.gtt_bytes),
            FormatBytes::new(mem_info.vram_bytes),
            FormatBytes::new(mem_info.gtt_bytes),
        );
    }

    Ok(())
}
