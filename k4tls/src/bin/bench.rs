use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
struct Stats {
    samples: Vec<Duration>,
}

impl Stats {
    fn new(n: usize) -> Self {
        Stats {
            samples: Vec::with_capacity(n),
        }
    }
    fn push(&mut self, d: Duration) {
        self.samples.push(d);
    }
    fn report(&self) -> String {
        let mut s = self.samples.clone();
        s.sort();
        if s.is_empty() {
            return "no samples".into();
        }
        let min = s[0].as_secs_f64() * 1000.0;
        let med = s[s.len() / 2].as_secs_f64() * 1000.0;
        let p95 = s[(s.len() as f64 * 0.95).ceil() as usize - 1].as_secs_f64() * 1000.0;
        let mean = s.iter().map(|d| d.as_secs_f64() * 1000.0).sum::<f64>() / s.len() as f64;
        format!(
            "n={} min={min:.1}ms mean={mean:.1}ms median={med:.1}ms p95={p95:.1}ms",
            s.len()
        )
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut url: Option<String> = None;
    let mut n: usize = 10;
    let mut block_ms: i32 = 4000;
    let mut total_ms: i32 = 15000;
    let mut trust_pem: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--url" => {
                i += 1;
                url = args.get(i).cloned();
            }
            "--n" => {
                i += 1;
                n = args.get(i).and_then(|v| v.parse().ok()).unwrap_or(10);
            }
            "--block-ms" => {
                i += 1;
                block_ms = args.get(i).and_then(|v| v.parse().ok()).unwrap_or(4000);
            }
            "--total-ms" => {
                i += 1;
                total_ms = args.get(i).and_then(|v| v.parse().ok()).unwrap_or(15000);
            }
            "--trust-pem" => {
                i += 1;
                trust_pem = args.get(i).cloned();
            }
            other => {
                eprintln!("unknown arg: {other}");
                std::process::exit(2);
            }
        }
        i += 1;
    }
    let Some(url) = url else {
        eprintln!("k4tls-bench --url https://host/path [--n 10] [--block-ms 4000] [--total-ms 15000] [--trust-pem file]");
        std::process::exit(2);
    };
    let pem = trust_pem.map(|p| {
        std::fs::read(&p).unwrap_or_else(|e| {
            eprintln!("read {p}: {e}");
            std::process::exit(2);
        })
    });

    println!(
        "{}",
        unsafe { std::ffi::CStr::from_ptr(k4tls::k4tls_version()) }.to_string_lossy()
    );
    let mut stats = Stats::new(n);
    let mut ok = 0usize;
    for i in 0..n {
        let started = Instant::now();
        let req = k4tls::K4TlsRequest {
            url: cstr(&url),
            method: cstr("GET"),
            headers: std::ptr::null(),
            nheaders: 0,
            body: std::ptr::null(),
            body_len: 0,
            block_timeout_ms: block_ms,
            total_timeout_ms: total_ms,
            max_response_bytes: 1 << 20,
            trust_pem: pem.as_ref().map_or(std::ptr::null(), |p| p.as_ptr()),
            trust_pem_len: pem.as_ref().map_or(0, |p| p.len()),
        };
        let mut resp: k4tls::K4TlsResponse = unsafe { std::mem::zeroed() };
        let rc = unsafe { k4tls::k4tls_perform(&req, &mut resp) };
        let elapsed = started.elapsed();
        let err = unsafe { std::ffi::CStr::from_ptr(resp.err.as_ptr()) }
            .to_string_lossy()
            .to_string();
        let status = resp.status;
        unsafe { k4tls::k4tls_free_response(&mut resp) };
        println!(
            "#{} rc={} status={} {} {:.1}ms",
            i,
            rc,
            status,
            if err.is_empty() { "-" } else { &err },
            elapsed.as_secs_f64() * 1000.0
        );
        if (200..400).contains(&status) && rc == 0 {
            ok += 1;
            stats.push(elapsed);
        }
    }
    println!("success: {ok}/{n}");
    println!("{}", stats.report());
}

fn cstr(s: &str) -> *const i8 {
    // bench 进程生命周期覆盖请求，泄漏无碍
    std::ffi::CString::new(s).unwrap().into_raw()
}
