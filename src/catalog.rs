use crate::model::Guest;

pub fn guests(idle: bool) -> Vec<Guest> {
    let units = [
        (100, "tailscale", "overlay", true, false),
        (102, "technitium", "dns", false, false),
        (103, "vaultwarden", "passwords", false, false),
        (105, "jitsi", "video", false, true),
        (106, "immich", "photos", false, true),
        (108, "npm", "proxy", false, false),
        (109, "papra", "documents", false, true),
        (110, "mail-gateway", "mail", false, true),
        (111, "open-archiver", "archive", false, true),
    ];

    units
        .into_iter()
        .map(|(id, name, role, floor, quiet)| Guest {
            id,
            name,
            role,
            floor,
            status: if floor {
                "running"
            } else if idle {
                "stopped"
            } else if quiet {
                "parked"
            } else {
                "running"
            },
            why: if floor {
                "Always on. This is the path to your files when you are not at home, and the door to open other apps."
            } else if idle {
                "Closed. Still installed. Open it when you need it."
            } else if quiet {
                "Closed until something asks: a URL, a job, a backup window."
            } else {
                "Usually open when someone is using the box (files, DNS, vault, proxy)."
            },
        })
        .collect()
}
