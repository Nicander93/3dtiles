//! Cancel via SIGINT/SIGTERM or stdin line ("cancel" / {"type":"cancel"}). Plan §7.5.

use std::io::{self, BufRead};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

#[derive(Clone)]
pub struct CancelFlag {
    inner: Arc<AtomicBool>,
}

impl CancelFlag {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn request(&self) {
        self.inner.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner.load(Ordering::SeqCst)
    }

    /// Install signal handlers + stdin watcher. Safe to call once per process.
    pub fn install_watchers(&self) {
        let flag = self.clone();
        let _ = ctrlc::set_handler(move || {
            flag.request();
            eprintln!("[processor] cancel requested via signal");
        });

        let flag = self.clone();
        thread::spawn(move || {
            let stdin = io::stdin();
            let mut lock = stdin.lock();
            let mut line = String::new();
            loop {
                line.clear();
                match lock.read_line(&mut line) {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        let t = line.trim();
                        if t.eq_ignore_ascii_case("cancel")
                            || t.contains("\"type\":\"cancel\"")
                            || t.contains("\"type\": \"cancel\"")
                        {
                            flag.request();
                            eprintln!("[processor] cancel requested via stdin");
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    }
}

impl Default for CancelFlag {
    fn default() -> Self {
        Self::new()
    }
}
