use lazy_static::lazy_static;
pub use pprof_proc::time;
use std::sync::Mutex;
use std::time::Instant;

lazy_static! {
    pub static ref PROFILER: Mutex<Profiler> = Mutex::new(Profiler::new());
}

#[cfg(not(feature = "rdtsc"))]
pub struct Block {
    anchor_id: usize,
    start: Instant,
}

#[cfg(not(feature = "rdtsc"))]
impl Block {
    pub fn new(anchor_id: usize) -> Self {
        Self {
            anchor_id,
            start: Instant::now(),
        }
    }

    pub fn elapsed(&self) -> u64 {
        self.start.elapsed().as_nanos() as u64
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

#[cfg(feature = "rdtsc")]
pub struct Block {
    anchor_id: usize,
    start: u64,
}

#[cfg(feature = "rdtsc")]
impl Block {
    pub fn new(anchor_id: usize) -> Self {
        let start = get_cpu_timer!();
        Self {
            anchor_id,
            start,
        }
    }

    pub fn elapsed(&self) -> u64 {
        let end = get_cpu_timer!();
        end - self.start
    }
}

impl Drop for Block {
    fn drop(&mut self) {
        PROFILER.lock().unwrap().add_block(self);
    }
}

#[derive(Default)]
pub struct Anchor {
    name: String,
    calls: usize,
    elapsed: u64,
    children_elapsed: u64,
    self_children_elapsed: u64,
    bytes: usize,
}

impl Anchor {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            calls: 0,
            elapsed: 0,
            children_elapsed: 0,
            self_children_elapsed: 0,
            bytes: 0,
        }
    }

    pub fn update(&mut self, duration: u64) {
        self.elapsed += duration;
        self.calls += 1;
    }

    pub fn update_from_child(&mut self, duration: u64) {
        self.children_elapsed += duration;
    }

    pub fn update_from_self_child(&mut self, duration: u64) {
        self.self_children_elapsed += duration;
    }
}

pub struct Profiler {
    anchor_id_stack: Vec<usize>,
    anchors: Vec<Anchor>,
    start: Instant,
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

impl Profiler {
    pub fn new() -> Self {
        Self {
            anchor_id_stack: Vec::new(),
            anchors: Vec::new(),
            start: Instant::now(),
        }
    }

    pub fn get_anchor_id(&mut self, name: &str) -> usize {
        let i = if let Some(i) = self.anchors.iter().position(|n| n.name.as_str() == name) {
            i
        } else {
            self.anchors.push(Anchor::new(name));
            self.anchors.len() - 1
        };
        self.anchor_id_stack.push(i);
        i
    }

    pub fn add_block(&mut self, block: &Block) {
        self.anchor_id_stack.pop();
        let duration = block.elapsed();
        self.anchors[block.anchor_id].update(duration);
        let mut updated_children = Vec::new();
        for a in &self.anchor_id_stack {
            if !updated_children.contains(a) {
                updated_children.push(*a);
                if *a == block.anchor_id {
                    self.anchors[*a].update_from_self_child(duration);
                } else {
                    self.anchors[*a].update_from_child(duration);
                }
            }
        }
    }

    pub fn print(&mut self) {
        let total_duration = self.start.elapsed();
        let freq = get_duration_freq();
        println!("--- PProf Results ---");
        println!("Total time: {:?}", total_duration);
        for anchor in &self.anchors {
            let elapsed = (anchor.elapsed - anchor.self_children_elapsed) as f64 / freq;
            let self_elapsed = (anchor.elapsed - anchor.children_elapsed) as f64 / freq;
            let elapsed_percentage =
                elapsed as f64 / (total_duration.as_nanos() as f64 / 1_000_000.0) * 100.0;
            let self_elapsed_percentage =
                self_elapsed as f64 / (total_duration.as_nanos() as f64 / 1_000_000.0) * 100.0;

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

    pub fn add_bytes(&mut self, anchor_id: usize, bytes: usize) {
        self.anchors[anchor_id].bytes += bytes;
    }
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
        pprof::Block::new(id)
    }};
    ($name:expr) => {{
        let name = format!("{}[{}]", pprof::fn_name!(), $name);
        let id = pprof::PROFILER.lock().unwrap().get_anchor_id(&name);
        pprof::Block::new(id)
    }};
    ($name:expr, $bytes:expr) => {{
        let name = format!("{}[{}]", pprof::fn_name!(), $name);
        let id = pprof::PROFILER.lock().unwrap().get_anchor_id(&name);
        pprof::PROFILER.lock().unwrap().add_bytes(id, $bytes);
        pprof::Block::new(id)
    }}
}

pub fn init() {
    PROFILER.lock().unwrap().start = Instant::now();
}

pub fn print() {
    PROFILER.lock().unwrap().print();
}
