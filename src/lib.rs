use lazy_static::lazy_static;
pub use pprof_proc::time;
use std::sync::Mutex;
use std::time::Instant;

lazy_static! {
    pub static ref PROFILER: Mutex<Profiler> = Mutex::new(Profiler::new());
}

pub struct Anchor {
    name: String,
    elapsed_exclusive: u64,
    elapsed_inclusive: u64,
    calls: usize,
    bytes: usize,
}

impl Anchor {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            elapsed_exclusive: 0,
            elapsed_inclusive: 0,
            calls: 0,
            bytes: 0,
        }
    }
}

pub struct Profiler {
    anchors: Vec<Anchor>,
    start: Instant,
    parent_id: usize,
}

impl Profiler {
    pub fn new() -> Self {
        let mut anchors = Vec::new();
        anchors.push(Anchor::new(""));
        Self {
            anchors,
            start: Instant::now(),
            parent_id: 0,
        }
    }

    pub fn get_anchor_id(&mut self, name: &str) -> usize {
        let i = if let Some(i) = self.anchors.iter().position(|n| n.name.as_str() == name) {
            i
        } else {
            self.anchors.push(Anchor::new(name));
            self.anchors.len() - 1
        };
        i
    }

    pub fn print(&mut self) {
        let total_duration = self.start.elapsed().as_nanos() as f64 / 1_000_000_000.0;
        let freq = get_duration_freq();
        println!("--- PProf Results ---");
        println!("Total time: {:.4}ms", total_duration * 1000.0);
        for anchor in &self.anchors {
            if anchor.elapsed_inclusive != 0 {
                let elapsed = anchor.elapsed_inclusive as f64 / freq;
                let self_elapsed = anchor.elapsed_exclusive as f64 / freq;
                let elapsed_percentage = elapsed as f64 / total_duration * 100.0;
                let self_elapsed_percentage = self_elapsed as f64 / total_duration * 100.0;

                let throughput_str = if anchor.bytes != 0 {
                    let mb = (1024 * 1024) as f64;
                    let gb = (1024 * 1024 * 1024) as f64;
                    format!(" throughput={:.4} MB at {:.4} GB/s", anchor.bytes as f64 / mb, anchor.bytes as f64 / gb / elapsed)
                } else {
                    String::new()
                };

                println!(
                    "{}[{}] - total={:.4}ms ({:.4}%) self={:.4}ms ({:.4}%){}",
                    anchor.name,
                    anchor.calls,
                    elapsed * 1000.0,
                    elapsed_percentage,
                    self_elapsed * 1000.0,
                    self_elapsed_percentage,
                    throughput_str,
                );
            }
        }
    }

    pub fn add_bytes(&mut self, anchor_id: usize, bytes: usize) {
        self.anchors[anchor_id].bytes += bytes;
    }
}

#[cfg(feature = "rdtsc")]
macro_rules! get_cpu_timer {
    () => {{
        unsafe {
            core::arch::x86_64::_rdtsc()
        }
    }}
}

#[cfg(not(feature = "rdtsc"))]
pub struct Block {
    start: Instant,
    anchor_id: usize,
    parent_id: usize,
    old_elapsed_inclusive: u64,
}

#[cfg(feature = "rdtsc")]
pub struct Block {
    start: u64,
    anchor_id: usize,
    parent_id: usize,
    old_elapsed_inclusive: u64,
}

impl Block {
    #[cfg(not(feature = "rdtsc"))]
    pub fn new(anchor_id: usize, parent_id: usize, old_elapsed_inclusive: u64) -> Self {
        Self {
            start: Instant::now(),
            anchor_id,
            parent_id,
            old_elapsed_inclusive,
        }
    }

    #[cfg(feature = "rdtsc")]
    pub fn new(anchor_id: usize, parent_id: usize, old_elapsed_inclusive: u64) -> Self {
        Self {
            start: get_cpu_timer!(),
            anchor_id,
            parent_id,
            old_elapsed_inclusive,
        }
    }

    #[cfg(not(feature = "rdtsc"))]
    pub fn elapsed(&self) -> u64 {
        self.start.elapsed().as_nanos() as u64
    }

    #[cfg(feature = "rdtsc")]
    pub fn elapsed(&self) -> u64 {
        get_cpu_timer!() - self.start
    }

    pub fn from_id(id: usize) -> Self {
        let mut p = PROFILER.lock().unwrap();
        let parent_id = p.parent_id;
        let old_elapsed_inclusive = p.anchors[id].elapsed_inclusive;
        p.parent_id = id;
        Self::new(id, parent_id, old_elapsed_inclusive)
    }
}

impl Drop for Block {
    fn drop(&mut self) {
        let elapsed = self.elapsed();
        let mut p = PROFILER.lock().unwrap();
        p.parent_id = self.parent_id;
        p.anchors[self.parent_id].elapsed_exclusive -= elapsed;
        p.anchors[self.anchor_id].elapsed_exclusive += elapsed;
        p.anchors[self.anchor_id].elapsed_inclusive = self.old_elapsed_inclusive + elapsed;
        p.anchors[self.anchor_id].calls += 1;
    }
}

#[cfg(not(feature = "rdtsc"))]
fn get_duration_freq() -> f64 {
    1000_000_000.0
}

#[cfg(feature = "rdtsc")]
fn get_duration_freq() -> f64 {
    let start = get_cpu_timer!();
    std::thread::sleep(std::time::Duration::from_millis(100));
    let end = get_cpu_timer!();
    (end - start) as f64 * 10.0
}


#[macro_export]
macro_rules! fn_name {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        type_name_of(f).strip_suffix("::f").unwrap()
    }};
}

#[macro_export]
macro_rules! block {
    () => {{
        let name = pprof::fn_name!();
        let id = pprof::PROFILER.lock().unwrap().get_anchor_id(&name);
        pprof::Block::from_id(id)
    }};
    ($name:expr) => {{
        let name = format!("{}[{}]", pprof::fn_name!(), $name);
        let id = pprof::PROFILER.lock().unwrap().get_anchor_id(&name);
        pprof::Block::from_id(id)
    }};
    ($name:expr, $bytes:expr) => {{
        let name = format!("{}[{}]", pprof::fn_name!(), $name);
        let id = pprof::PROFILER.lock().unwrap().get_anchor_id(&name);
        pprof::PROFILER.lock().unwrap().add_bytes(id, $bytes);
        pprof::Block::from_id(id)
    }}
}

pub fn init() {
    PROFILER.lock().unwrap().start = Instant::now();
}

pub fn print() {
    PROFILER.lock().unwrap().print();
}
