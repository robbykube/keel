use crate::inventory::Inventory;
use crate::model::{Issue, IssuesReport, RepairAction, RepairResult};

pub fn report(idle: bool, inventory: Inventory, pool_count: usize, share_count: usize) -> IssuesReport {
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
            title: "No pool",
            why: "There is no bucket for files yet. Open Pools and add one. Prefer “on another computer” so Proxmox is left alone.",
            evidence: "pools=0".into(),
            repair: None,
        });
    }

    if share_count == 0 && !idle {
        items.push(Issue {
            id: "no-share",
            severity: "yellow",
            title: "No share",
            why: "You turned things on but there is no named door into the files. Open Shares and add one.",
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
            id: "restart-samba",
            label: "Restart SMB",
            safe: true,
            available: false,
        },
        RepairAction {
            id: "restart-tailscale",
            label: "Restart Tailscale",
            safe: true,
            available: false,
        },
        RepairAction {
            id: "open-ports",
            label: "Open 9478 and 445",
            safe: true,
            available: false,
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
            message: "Rescan queued. On Linux this is lsblk/udevadm settle. Here it is a no-op besides acknowledging the playbook.".into(),
        },
        "restart-samba" | "restart-tailscale" | "open-ports" => RepairResult {
            ok: false,
            id: id.into(),
            message: "Not available until keel is the Linux appliance (systemctl / nft).".into(),
        },
        other => RepairResult {
            ok: false,
            id: other.into(),
            message: "Unknown repair.".into(),
        },
    }
}
