use crate::inventory::Inventory;
use crate::model::{Issue, IssuesReport, RepairAction, RepairResult};

pub fn report(
    idle: bool,
    inventory: Inventory,
    pool_count: usize,
    share_count: usize,
) -> IssuesReport {
    let mut items = Vec::new();

    if matches!(inventory, Inventory::Mock) {
        items.push(Issue {
            id: "mock-inventory",
            severity: "yellow",
            title: "This GUI is the control-plane host, not the NAS yet",
            why: "Windows has no lsblk. Disks are mock. Repair is: install keel on the Debian box, not a button here.",
            evidence: "inventory=mock".into(),
            repair: None,
        });
    }

    if pool_count == 0 {
        items.push(Issue {
            id: "no-pool",
            severity: "yellow",
            title: "No disk for files yet",
            why: "Files are kept on the system disk for now. Open Disks and give a disk the job of keeping files.",
            evidence: "pools=0".into(),
            repair: None,
        });
    }

    if share_count == 0 && !idle {
        items.push(Issue {
            id: "no-share",
            severity: "yellow",
            title: "No shared folder",
            why: "Keel is on, but there is no folder for other computers to open. Open Files and add one.",
            evidence: "shares=0".into(),
            repair: None,
        });
    }

    let playbook = vec![
        RepairAction {
            id: "rescan-disks",
            label: "Rescan disks",
            safe: true,
            available: true,
        },
        RepairAction {
            id: "restart-keel",
            label: "Restart Keel",
            safe: true,
            available: matches!(inventory, Inventory::Linux),
        },
        RepairAction {
            id: "restart-samba",
            label: "Restart file sharing",
            safe: true,
            available: matches!(inventory, Inventory::Linux),
        },
        RepairAction {
            id: "restart-tailscale",
            label: "Restart Away",
            safe: true,
            available: matches!(inventory, Inventory::Linux),
        },
        RepairAction {
            id: "open-ports",
            label: "Open the page and files to the house",
            safe: true,
            available: false,
        },
        RepairAction {
            id: "house-only",
            label: "Only the house and Away",
            safe: true,
            available: matches!(inventory, Inventory::Linux),
        },
    ];

    IssuesReport {
        red: items.iter().any(|i| i.severity == "red"),
        items,
        playbook,
    }
}

pub fn apply(id: &str) -> RepairResult {
    match id {
        "rescan-disks" => RepairResult {
            ok: true,
            id: id.into(),
            message: "Keel looked for disks again.".into(),
        },
        "restart-samba" | "restart-tailscale" | "open-ports" => RepairResult {
            ok: false,
            id: id.into(),
            message: "This works on the Keel computer itself.".into(),
        },
        other => RepairResult {
            ok: false,
            id: other.into(),
            message: "Unknown repair.".into(),
        },
    }
}
