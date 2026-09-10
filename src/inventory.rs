use crate::model::Disk;

#[derive(Clone, Copy)]
pub enum Inventory {
    Linux,
    Mock,
}

impl Inventory {
    pub fn detect() -> Self {
        if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            Self::Mock
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Linux => "linux-lsblk",
            Self::Mock => "mock",
        }
    }

    pub async fn disks(self) -> Vec<Disk> {
        match self {
            Self::Linux => tokio::task::spawn_blocking(lsblk_disks)
                .await
                .unwrap_or_else(|_| vec![error_disk("lsblk task failed")]),
            Self::Mock => mock_disks(),
        }
    }
}

fn mock_disks() -> Vec<Disk> {
    vec![
        Disk {
            name: "nvme0n1".into(),
            path: "/dev/nvme0n1".into(),
            size: 512_110_190_592,
            kind: "disk".into(),
            model: "MOCK-NVME".into(),
            transport: "nvme".into(),
            rotational: false,
            mount: "".into(),
            fstype: "".into(),
        },
        Disk {
            name: "sda".into(),
            path: "/dev/sda".into(),
            size: 8_001_563_222_016,
            kind: "disk".into(),
            model: "MOCK-HDD-8T".into(),
            transport: "sata".into(),
            rotational: true,
            mount: "".into(),
            fstype: "".into(),
        },
        Disk {
            name: "sdb".into(),
            path: "/dev/sdb".into(),
            size: 8_001_563_222_016,
            kind: "disk".into(),
            model: "MOCK-HDD-8T".into(),
            transport: "sata".into(),
            rotational: true,
            mount: "".into(),
            fstype: "".into(),
        },
    ]
}

fn error_disk(msg: &str) -> Disk {
    Disk {
        name: "error".into(),
        path: "".into(),
        size: 0,
        kind: "error".into(),
        model: msg.into(),
        transport: "".into(),
        rotational: false,
        mount: "".into(),
        fstype: "".into(),
    }
}

fn lsblk_disks() -> Vec<Disk> {
    let out = match std::process::Command::new("lsblk")
        .args([
            "-J",
            "-b",
            "-d",
            "-o",
            "NAME,SIZE,TYPE,MODEL,ROTA,TRAN,MOUNTPOINT,FSTYPE",
        ])
        .output()
    {
        Ok(o) if o.status.success() => o.stdout,
        Ok(o) => {
            return vec![error_disk(&String::from_utf8_lossy(&o.stderr))];
        }
        Err(e) => return vec![error_disk(&e.to_string())],
    };

    parse_lsblk(&out).unwrap_or_else(|e| vec![error_disk(&e)])
}

fn parse_lsblk(raw: &[u8]) -> Result<Vec<Disk>, String> {
    let v: serde_json::Value =
        serde_json::from_slice(raw).map_err(|e| format!("lsblk json: {e}"))?;
    let blockdevices = v
        .get("blockdevices")
        .and_then(|x| x.as_array())
        .ok_or_else(|| "no blockdevices".to_string())?;

    let mut disks = Vec::new();
    for d in blockdevices {
        let kind = json_str(d, "type");
        if kind != "disk" {
            continue;
        }
        let name = json_str(d, "name");
        disks.push(Disk {
            path: format!("/dev/{name}"),
            name,
            size: d.get("size").and_then(|x| x.as_u64()).unwrap_or(0),
            kind,
            model: json_str(d, "model"),
            transport: json_str(d, "tran"),
            rotational: d.get("rota").and_then(|x| x.as_bool()).unwrap_or(false),
            mount: json_str(d, "mountpoint"),
            fstype: json_str(d, "fstype"),
        });
    }
    Ok(disks)
}

fn json_str(v: &serde_json::Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}
