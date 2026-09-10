use crate::model::Opportunity;

pub fn opportunities(idle: bool, cpu_pct: f32) -> Vec<Opportunity> {
    let mut out = Vec::new();

    if idle {
        out.push(Opportunity {
            id: "floor",
            title: "Already on the Tailscale floor",
            why: "Catalog guests should be stopped. Host stays on so Tailscale (or the overlay we ship) still reaches your files. Next watts come from CPU C-states and spinning HDDs down, not from killing that path.",
            phase: "idle",
            safe_while_working: true,
            auto: true,
            evidence: "mode=idle".into(),
        });
    } else {
        out.push(Opportunity {
            id: "demand-catalog",
            title: "Do not run the whole catalog just because you are Active",
            why: "Active means we can start what you need. Jitsi with zero conferences, Immich with no jobs, PBS outside a backup window can stay stopped. Park unused work while you are still using the box.",
            phase: "active",
            safe_while_working: true,
            auto: true,
            evidence: "mode=active; per-app demand not wired yet".into(),
        });
        if cpu_pct < 20.0 {
            out.push(Opportunity {
                id: "no-turbo",
                title: "Drop turbo / pin a quiet governor",
                why: "CPU is quiet. Turbo spends watts for latency you are not using. Recheck when load spikes (backup, Immich ML).",
                phase: "active",
                safe_while_working: true,
                auto: true,
                evidence: format!("cpu={cpu_pct:.0}%"),
            });
        }
        out.push(Opportunity {
            id: "hdd-spindown",
            title: "Spin down HDDs with no I/O",
            why: "Keep the disk you are using awake. Idle spindles still draw watts. Only park disks with no reads/writes for a window.",
            phase: "active",
            safe_while_working: true,
            auto: true,
            evidence: "needs /proc/diskstats (mock host has no NAS I/O)".into(),
        });
        out.push(Opportunity {
            id: "jitsi-empty",
            title: "Stop Jitsi while no conferences",
            why: "Video is expensive at idle. Warm it when a meeting starts or someone opens the URL.",
            phase: "active",
            safe_while_working: true,
            auto: true,
            evidence: "guest 105; conference count not scraped yet".into(),
        });
        out.push(Opportunity {
            id: "immich-workers",
            title: "Scale Immich ML to zero between jobs",
            why: "The photo UI can stay up; the ML worker does not. You browse; the heavy path sleeps until an upload.",
            phase: "active",
            safe_while_working: true,
            auto: true,
            evidence: "guest 106; job queue not scraped yet".into(),
        });
    }

    out.push(Opportunity {
        id: "no-host-s3",
        title: "Never S3 the host to save a few watts",
        why: "That kills Tailscale, the file path from away, and this advisor. The floor is cheaper than a full wake path.",
        phase: "always",
        safe_while_working: true,
        auto: true,
        evidence: "policy".into(),
    });

    out
}
