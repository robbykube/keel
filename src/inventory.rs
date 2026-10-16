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
            name: "sda".into(),
            path: "/dev/sda".into(),
            size: 256_060_514_304,
            kind: "disk".into(),
            model: "MOCK-SSD-256".into(),
            transport: "sata".into(),
            mount: "/".into(),
            fstype: "ext4".into(),
            label: "KEEL-ROOT".into(),
            system: true,
            ..Disk::default()
        },
        Disk {
            name: "nvme0n1".into(),
            path: "/dev/nvme0n1".into(),
            size: 512_110_190_592,
            kind: "disk".into(),
            model: "MOCK-NVME".into(),
            transport: "nvme".into(),
            empty: true,
            ..Disk::default()
        },
        Disk {
            name: "sdb".into(),
            path: "/dev/sdb".into(),
            size: 500_107_862_016,
            kind: "disk".into(),
            model: "MOCK-HDD-500".into(),
            transport: "usb".into(),
            rotational: true,
            empty: true,
            ..Disk::default()
        },
        Disk {
            name: "sdc".into(),
            path: "/dev/sdc".into(),
            size: 500_107_862_016,
            kind: "disk".into(),
            model: "MOCK-HDD-500".into(),
            transport: "usb".into(),
            rotational: true,
            fstype: "ntfs".into(),
            ..Disk::default()
        },
    ]
}

fn error_disk(msg: &str) -> Disk {
    Disk {
        name: "error".into(),
        kind: "error".into(),
        model: msg.into(),
        ..Disk::default()
    }
}

pub(crate) fn lsblk_disks() -> Vec<Disk> {
    let out = match std::process::Command::new("lsblk")
        .args([
            "-J",
            "-b",
            "-o",
            "NAME,SIZE,TYPE,MODEL,ROTA,TRAN,MOUNTPOINT,FSTYPE,LABEL",
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

const SYSTEM_MOUNTS: &[&str] = &["/", "/boot", "/boot/efi", "/efi", "[SWAP]"];

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
        let mut mounts = Vec::new();
        let mut labels = Vec::new();
        let mut fstypes = Vec::new();
        collect(d, &mut mounts, &mut labels, &mut fstypes);
        let has_children = d
            .get("children")
            .and_then(|x| x.as_array())
            .is_some_and(|c| !c.is_empty());
        let system = mounts
            .iter()
            .any(|m| SYSTEM_MOUNTS.contains(&m.as_str()) || m.starts_with("/run/live"));
        let label = labels
            .iter()
            .find(|l| l.starts_with("keel-") || l.as_str() == "KEEL-ROOT")
            .or_else(|| labels.first())
            .cloned()
            .unwrap_or_default();
        disks.push(Disk {
            path: format!("/dev/{name}"),
            name,
            size: d.get("size").and_then(|x| x.as_u64()).unwrap_or(0),
            kind,
            model: json_str(d, "model"),
            transport: json_str(d, "tran"),
            rotational: d.get("rota").and_then(|x| x.as_bool()).unwrap_or(false),
            mount: mounts.first().cloned().unwrap_or_default(),
            fstype: fstypes.first().cloned().unwrap_or_default(),
            label,
            system,
            empty: !has_children && json_str(d, "fstype").is_empty(),
            ..Disk::default()
        });
    }
    Ok(disks)
}

fn collect(
    node: &serde_json::Value,
    mounts: &mut Vec<String>,
    labels: &mut Vec<String>,
    fstypes: &mut Vec<String>,
) {
    for (key, list) in [("mountpoint", &mut *mounts), ("label", &mut *labels), ("fstype", &mut *fstypes)] {
        let value = json_str(node, key);
        if !value.is_empty() {
            list.push(value);
        }
    }
    if let Some(children) = node.get("children").and_then(|x| x.as_array()) {
        for child in children {
            collect(child, mounts, labels, fstypes);
        }
    }
}

fn json_str(v: &serde_json::Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::parse_lsblk;

    #[test]
    fn reads_partitions_roles_and_the_system_disk() {
        let raw = br#"{"blockdevices":[
          {"name":"sda","size":256060514304,"type":"disk","model":"SAMSUNG","rota":false,"tran":"sata","mountpoint":null,"fstype":null,"label":null,
           "children":[
             {"name":"sda1","size":1048576,"type":"part","mountpoint":null,"fstype":null,"label":null},
             {"name":"sda3","size":255000000000,"type":"part","mountpoint":"/","fstype":"ext4","label":"KEEL-ROOT"}]},
          {"name":"sdb","size":500107862016,"type":"disk","model":"WDC","rota":true,"tran":"usb","mountpoint":null,"fstype":null,"label":null},
          {"name":"sdc","size":500107862016,"type":"disk","model":"ST500","rota":true,"tran":"usb","mountpoint":null,"fstype":null,"label":null,
           "children":[{"name":"sdc1","size":500000000000,"type":"part","mountpoint":"/srv/keel/files","fstype":"ext4","label":"keel-files"}]},
          {"name":"loop0","size":1,"type":"loop","mountpoint":null,"fstype":null,"label":null}
        ]}"#;
        let disks = parse_lsblk(raw).unwrap();
        assert_eq!(disks.len(), 3);
        assert!(disks[0].system && !disks[0].empty);
        assert_eq!(disks[0].mount, "/");
        assert!(!disks[1].system && disks[1].empty);
        assert_eq!(disks[2].label, "keel-files");
        assert_eq!(disks[2].mount, "/srv/keel/files");
        assert!(!disks[2].system && !disks[2].empty);
    }
}
