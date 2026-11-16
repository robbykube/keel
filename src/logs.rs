//! Logs from Names and the front door.
//!
//! A signed-in browser on another computer can read them. The Keel screen
//! can too. Nothing here is offered before sign-in.

pub fn recent() -> String {
    if !cfg!(target_os = "linux") {
        return "Logs are on the Keel computer.".into();
    }
    match crate::storage::host_all("exec /bin/bash -s", &[], Some(include_str!("logs.sh"))) {
        Ok(text) if text.trim().is_empty() => "Nothing has been written yet.".into(),
        Ok(text) => text,
        Err(message) => message,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_log_script_reads_the_front_door_and_names() {
        let script = include_str!("logs.sh");
        assert!(script.contains("podman logs --tail 100 keel-door"));
        assert!(script.contains("/var/lib/keel/door.log"));
        assert!(script.contains("podman logs --tail 80 keel-names"));
        assert!(!script.contains("9478"));
        assert!(!script.contains("KEEL_ADMIN_TOKEN"));
    }
}
