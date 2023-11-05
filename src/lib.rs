use lazy_static::lazy_static;
pub use pprof_proc::time;
use std::sync::Mutex;
use std::time::{Duration, Instant};

lazy_static! {
    pub static ref PROFILER: Mutex<Profiler> = Mutex::new(Profiler::new());
}

#[must_use]
pub struct Block {
    anchor_id: usize,
    start: Instant,
}

impl Block {
    pub fn new(anchor_id: usize) -> Self {
        Self {
            anchor_id,
            start: Instant::now(),
        }
    }
}

#[derive(Default)]
pub struct Anchor {
    name: String,
    calls: usize,
    elapsed: Duration,
    children_elapsed: Duration,
    self_children_elapsed: Duration,
}

impl Anchor {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            calls: 0,
            elapsed: Duration::default(),
            children_elapsed: Duration::default(),
            self_children_elapsed: Duration::default(),
        }
    }

    pub fn update(&mut self, duration: Duration) {
        self.elapsed += duration;
        self.calls += 1;
    }

    pub fn update_from_child(&mut self, duration: Duration) {
        self.children_elapsed += duration;
    }

    pub fn update_from_self_child(&mut self, duration: Duration) {
        self.self_children_elapsed += duration;
    }
}

impl Drop for Block {
    fn drop(&mut self) {
        PROFILER.lock().unwrap().add_block(self);
    }
}

pub struct Profiler {
    anchor_id_stack: Vec<usize>,
    anchors: Vec<Anchor>,
    start: Instant,
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
        let duration = block.start.elapsed();
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
        println!("--- PProf Results ---");
        println!("Total time: {:?}", total_duration);
        for anchor in &self.anchors {
            let elapsed = anchor.elapsed - anchor.self_children_elapsed;
            let self_elapsed = anchor.elapsed - anchor.children_elapsed;
            let elapsed_percentage =
                elapsed.as_nanos() as f64 / total_duration.as_nanos() as f64 * 100.0;
            let self_elapsed_percentage =
                self_elapsed.as_nanos() as f64 / total_duration.as_nanos() as f64 * 100.0;
            println!(
                "{}[{}] - total={:?} ({:.4}%) self={:?} ({:.4}%)",
                anchor.name,
                anchor.calls,
                elapsed,
                elapsed_percentage,
                self_elapsed,
                self_elapsed_percentage
            );
        }
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
    ($arg:expr) => {{
        let name = format!("{}[{}]", pprof::fn_name!(), $arg);
        let id = pprof::PROFILER.lock().unwrap().get_anchor_id(&name);
        pprof::Block::new(id)
    }};
}

pub fn init() {
    PROFILER.lock().unwrap().start = Instant::now();
}

pub fn print() {
    PROFILER.lock().unwrap().print();
}
