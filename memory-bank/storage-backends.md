# Storage: keep the bits, replace the rest

keel can **own the experience** (GUI, catalog, idle, Repair) while **not owning the disks**. That is how we respect rok / an existing TrueNAS: do not reimage them; mount what they already export.

## Two owners cannot both run the same ZFS pool

If TrueNAS (or Proxmox) still has the pool imported, keel must be a **client** (NFS, SMB, or iSCSI).  
If keel **imports** the pool, TrueNAS must export it first — that *is* replacing TrueNAS for those disks, even if the data never copies.

## Mode: foreign pool (keep Proxmox/TrueNAS storage)

- rok or TrueNAS keeps serving the dataset (NFS/SMB/iSCSI).
- keel mounts it as the data root. OpenCloud/Immich/SMB on keel point at that mount.
- Snapshots/SMART/spindown stay on the **storage** box. keel’s idle story applies to **keel’s** units, not to spinning down someone else’s vdev.
- Repair tells the truth: “pool is remote; Fix cannot rewrite ZFS on TrueNAS.”

Good for: reticence about touching rok, gradual cutover, family files that already live on OpenCloud’s disk.

**Checking those files from this Windows copy:** Sign in to Proxmox on Pools, then List files / OpenCloud (files-app password). That is read-only listing, not a mount. Snapshots/SMART stay on rok.

## Mode: native pool (keel is the NAS)

- Disks (or HBA) belong to keel. ZFS/mdadm locally.
- Full idle + scrub + snapshots in keel.

Good for: a second machine that is actually the NAS.

## What we do not do

- Skin TrueNAS’s API as “keel” (two control planes, their API, our look).
- Import a pool while TrueNAS still holds it.
- Pretend remote NFS is a local vdev in the UI.

## Later: you can still delete TrueNAS or Proxmox

Foreign pool is a **phase**, not a lock. When you are ready:

1. Export the pool / stop using the share (TrueNAS export, or copy once if it was only SMB).
2. Import or attach those disks on keel (native pool).
3. Then the old OS can go.

Until that day, do not reimage rok. After that day, keel *is* the NAS and the hypervisor/TrueNAS box is optional leftover hardware.
