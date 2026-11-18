//! Keeps this computer answering when memory runs short.
//!
//! Compressed memory takes the overflow, the screen has a share it cannot go
//! past, and Keel and the file sharing are the last things the system stops.
//! A full screen then restarts on its own instead of the whole computer
//! locking up. Keel applies this at every start, so an update brings it to a
//! computer that is already installed.

pub fn guard() -> Result<(), String> {
    if !cfg!(target_os = "linux") {
        return Ok(());
    }
    crate::storage::host("exec /bin/bash -s", &[], Some(script()))
}

fn script() -> &'static str {
    include_str!("memory-guard.sh")
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_screen_gives_way_before_keel_and_the_files() {
        let script = super::script();
        assert!(script.contains("MemoryMax=50%"));
        assert!(script.contains("keel-kiosk.service.d/memory.conf"));
        assert!(script.contains("OOMScoreAdjust=-600"));
        assert!(script.contains("zramctl --find"));
        assert!(script.contains("WantedBy=swap.target"));
    }
}
