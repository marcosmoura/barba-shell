use std::thread;

pub fn spawn_named_thread<F>(name: &str, task: F)
where F: FnOnce() + Send + 'static {
    let thread_name = format!("barba-{name}");

    if let Err(err) = thread::Builder::new().name(thread_name.clone()).spawn(task) {
        eprintln!("Failed to spawn {thread_name}: {err}");
    }
}
