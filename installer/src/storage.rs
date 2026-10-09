//! `storage:` -> partitions on disk and mounts under /mnt. plan() is what
//! validation checks and what apply() carries out, so the two can't disagree.

use std::{collections::BTreeSet, fs};

use anyhow::{bail, Result};
use shell_iface::{logger::Logger, Shell};

use crate::{
    config::{size_bytes, FormatChoice, NewPartition, PartitionType, Storage},
    filesystem_tasks::Filesystem,
    system::{human_size, lsblk, BlockDevice, System},
};

/// A partition a mount refers to, before partitioning runs.
#[derive(Clone, PartialEq, Debug)]
pub enum Part {
    Existing { path: String, blank: bool },
    /// Made by partitioning, found afterwards by its GPT name.
    New { disk: String, name: String },
}

impl Part {
    fn blank(&self) -> bool {
        match self {
            Part::Existing { blank, .. } => *blank,
            Part::New { .. } => true,
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Part::Existing { path, .. } => path.clone(),
            Part::New { disk, name } => format!("{name} (new on {disk})"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct DiskJob {
    pub disk: String,
    pub wipe: bool,
    pub add: Vec<NewPartition>,
}

#[derive(Clone, Debug)]
pub struct Plan {
    pub disks: Vec<DiskJob>,
    /// Mount point relative to /mnt ("" is root) -> partition
    pub mounts: Vec<(String, Part)>,
    pub format_boot: bool,
    pub format_home: bool,
}

fn disk_of<'s>(sys: &'s System, part: &BlockDevice) -> Option<&'s BlockDevice> {
    sys.disks().into_iter().find(|d| Some(d.name()) == part.pkname.as_deref())
}

/// Works out disks, partitions and mounts, or every reason it can't.
pub fn plan(storage: &Storage, sys: &System) -> std::result::Result<Plan, Vec<String>> {
    let mut errors = vec![];
    let mut disks: Vec<DiskJob> = vec![];

    for entry in &storage.partitioning {
        let disk = if entry.disk == "auto" {
            match sys.internal_disks()[..] {
                [disk] => disk.path.clone(),
                [] => {
                    errors.push("disk: auto found no internal disk".into());
                    continue;
                }
                ref many => {
                    let names: Vec<&str> = many.iter().map(|d| d.path.as_str()).collect();
                    errors.push(format!("disk: auto found several internal disks ({}); name one", names.join(", ")));
                    continue;
                }
            }
        } else {
            let real = fs::canonicalize(&entry.disk).map(|p| p.display().to_string()).unwrap_or(entry.disk.clone());
            if !sys.disks().iter().any(|d| d.path == real) {
                errors.push(format!("{} is not a disk on this machine", entry.disk));
                continue;
            }
            real
        };
        if disks.iter().any(|d| d.disk == disk) {
            errors.push(format!("{disk} is listed twice under partitioning"));
            continue;
        }

        let last = entry.add.len().saturating_sub(1);
        for (i, part) in entry.add.iter().enumerate() {
            if part.size == "rest" && i != last {
                errors.push(format!("{disk}: size: rest is only allowed on the last new partition ({})", part.name));
            }
        }

        // Does it fit?
        let disk_size = sys.disks().iter().find(|d| d.path == disk).and_then(|d| d.size).unwrap_or(0);
        let used: u64 = if entry.wipe {
            0
        } else {
            sys.partitions().iter().filter(|p| disk_of(sys, p).map(|d| &d.path) == Some(&disk)).filter_map(|p| p.size).sum()
        };
        // GPT headers and 1 MiB alignment per partition
        let overhead = (2 + entry.add.len() as u64) << 20;
        let free = disk_size.saturating_sub(used + overhead);
        let needed: u64 = entry.add.iter().map(|p| size_bytes(&p.size).unwrap_or(1 << 20)).sum();
        if needed > free {
            errors.push(format!("{disk}: the new partitions need {} but only {} is free", human_size(needed), human_size(free)));
        }
        disks.push(DiskJob { disk, wipe: entry.wipe, add: entry.add.clone() });
    }

    let mut names = BTreeSet::new();
    for job in &disks {
        for part in &job.add {
            if !names.insert(part.name.clone()) {
                errors.push(format!("two new partitions are named {}", part.name));
            }
        }
    }

    let wiped: Vec<&str> = disks.iter().filter(|d| d.wipe).map(|d| d.disk.as_str()).collect();
    let survives = |p: &&BlockDevice| disk_of(sys, p).is_none_or(|d| !wiped.contains(&d.path.as_str()));
    let existing: Vec<&BlockDevice> = sys.partitions().into_iter().filter(survives).collect();
    let new: Vec<(&DiskJob, &NewPartition)> = disks.iter().flat_map(|d| d.add.iter().map(move |p| (d, p))).collect();

    let resolve = |reference: &str| -> std::result::Result<Part, String> {
        let as_existing = |d: &BlockDevice| Part::Existing { path: d.path.clone(), blank: d.fstype.is_none() };
        let as_new = |(job, p): &(&DiskJob, &NewPartition)| Part::New { disk: job.disk.clone(), name: p.name.clone() };
        let mut found: Vec<Part> = if reference.starts_with("/dev/") {
            let real = fs::canonicalize(reference).map(|p| p.display().to_string()).unwrap_or(reference.to_string());
            if let Some(gone) = sys.partitions().into_iter().find(|p| p.path == real && !survives(p)) {
                let disk = disk_of(sys, gone).map(|d| d.path.clone()).unwrap_or_default();
                return Err(format!("{reference} is on {disk}, which gets wiped; refer to new partitions as PARTLABEL=<name>"));
            }
            existing.iter().filter(|p| p.path == real).map(|p| as_existing(p)).collect()
        } else if let Some((key, value)) = reference.split_once('=') {
            let matches = |field: &Option<String>| field.as_deref().is_some_and(|f| f.eq_ignore_ascii_case(value));
            let mut found: Vec<Part> = existing
                .iter()
                .filter(|p| match key {
                    "PARTLABEL" => p.partlabel.as_deref() == Some(value),
                    "PARTUUID" => matches(&p.partuuid),
                    "UUID" => matches(&p.uuid),
                    "LABEL" => p.label.as_deref() == Some(value),
                    _ => false,
                })
                .map(|p| as_existing(p))
                .collect();
            if key == "PARTLABEL" {
                found.extend(new.iter().filter(|(_, p)| p.name == value).map(as_new));
            }
            found
        } else {
            vec![]
        };
        match found.len() {
            1 => Ok(found.remove(0)),
            0 => Err(format!("no partition matches {reference}")),
            n => Err(format!("{reference} matches {n} partitions")),
        }
    };

    let first_new = |kind: PartitionType| new.iter().find(|(_, p)| p.kind == kind).map(|(job, p)| Part::New { disk: job.disk.clone(), name: p.name.clone() });

    let mut mounts: Vec<(String, Part)> = vec![];
    fn add(mounts: &mut Vec<(String, Part)>, errors: &mut Vec<String>, mount: &str, part: std::result::Result<Part, String>) {
        match part {
            Ok(part) => mounts.push((mount.to_string(), part)),
            Err(e) => errors.push(e),
        }
    }

    let root = match &storage.root {
        Some(root) => resolve(&root.partition),
        None => first_new(PartitionType::Linux).ok_or_else(|| "storage needs a root partition: partitioning with a linux partition, or root.partition".to_string()),
    };
    add(&mut mounts, &mut errors, "", root);

    let efi = if storage.efi.partition == "auto" {
        first_new(PartitionType::Efi)
            .or_else(|| existing.iter().find(|p| p.parttypename.as_deref() == Some("EFI System")).map(|p| Part::Existing { path: p.path.clone(), blank: p.fstype.is_none() }))
            .ok_or_else(|| "no EFI partition: make one with partitioning or name it in efi.partition".to_string())
    } else {
        resolve(&storage.efi.partition)
    };
    add(&mut mounts, &mut errors, "boot", efi);

    if let Some(home) = storage.home.as_ref().filter(|h| h.partition != "none") {
        add(&mut mounts, &mut errors, "home", resolve(&home.partition));
    }

    for other in &storage.other {
        let mount = other.mountpoint.trim_matches('/').to_string();
        if ["", "boot", "home"].contains(&mount.as_str()) || mounts.iter().any(|(m, _)| *m == mount) {
            errors.push(format!("mountpoint {} is used twice", other.mountpoint));
            continue;
        }
        match resolve(&other.partition) {
            Ok(part) if part.blank() => errors.push(format!("{} has no filesystem, and other mounts are never formatted", other.partition)),
            part => add(&mut mounts, &mut errors, &mount, part),
        }
    }

    for (i, (mount, part)) in mounts.iter().enumerate() {
        if let Some((other, _)) = mounts[..i].iter().find(|(_, p)| p == part) {
            errors.push(format!("{} is used for both /{other} and /{mount}", part.describe()));
        }
    }

    let format = |mount: &str, choice: FormatChoice, errors: &mut Vec<String>| -> bool {
        let Some((_, part)) = mounts.iter().find(|(m, _)| m == mount) else { return false };
        match choice {
            FormatChoice::Format => true,
            FormatChoice::Auto => part.blank(),
            FormatChoice::Keep if matches!(part, Part::New { .. }) => {
                errors.push(format!("/{mount}: {} is new, so it has to be formatted", part.describe()));
                false
            }
            FormatChoice::Keep => false,
        }
    };
    let format_boot = format("boot", storage.efi.format, &mut errors);
    let format_home = format("home", storage.home.as_ref().map_or(FormatChoice::Auto, |h| h.format), &mut errors);

    if errors.is_empty() {
        Ok(Plan { disks, mounts, format_boot, format_home })
    } else {
        Err(errors)
    }
}

/// sfdisk lines for new partitions, e.g. `size=1GiB, type=uefi, name="arch-efi"`
pub fn sfdisk_script(job: &DiskJob) -> String {
    let lines: Vec<String> = job
        .add
        .iter()
        .map(|p| {
            let kind = match p.kind {
                PartitionType::Efi => "uefi",
                PartitionType::Linux => "linux",
            };
            let size = if p.size == "rest" { String::new() } else { format!("size={}, ", p.size) };
            format!("{size}type={kind}, name=\"{}\"", p.name)
        })
        .collect();
    let label = if job.wipe { "label: gpt\n" } else { "" };
    format!("{label}{}\n", lines.join("\n"))
}

/// Partitions the disks and resolves every mount to a device.
pub fn apply<'a>(plan: &Plan, logger: &'a Logger) -> Result<Filesystem<'a>> {
    let mut shell = Shell::new("Storage", logger);
    for job in &plan.disks {
        let flags = if job.wipe { "--wipe always --wipe-partitions always" } else { "--append --wipe-partitions always" };
        shell.run_with_input("sfdisk", &format!("{flags} {}", job.disk), &sfdisk_script(job))?;
    }
    if !plan.disks.is_empty() {
        shell.run_and_wait_with_args("udevadm", "settle")?;
    }

    let devices = lsblk(&mut shell)?;
    let mut filesystem = Filesystem::new(logger);
    for (mount, part) in &plan.mounts {
        let path = match part {
            Part::Existing { path, .. } => path.clone(),
            Part::New { disk, name } => {
                let disk_name = disk.trim_start_matches("/dev/");
                match devices.iter().find(|d| d.pkname.as_deref() == Some(disk_name) && d.partlabel.as_deref() == Some(name)) {
                    Some(d) => d.path.clone(),
                    None => bail!("the new partition {name} doesn't show up on {disk}"),
                }
            }
        };
        filesystem.set(mount, Some(&path))?;
    }
    filesystem.format_boot = plan.format_boot;
    filesystem.format_home = plan.format_home;
    Ok(filesystem)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{default_layout, DiskPlan, Efi, Home, OtherMount, Root};

    fn dev(path: &str, kind: &str, size_gib: u64, fstype: Option<&str>, partlabel: Option<&str>, pkname: Option<&str>) -> BlockDevice {
        BlockDevice {
            path: path.into(),
            kind: kind.into(),
            size: Some(size_gib << 30),
            fstype: fstype.map(String::from),
            partlabel: partlabel.map(String::from),
            pkname: pkname.map(String::from),
            parttypename: (fstype == Some("vfat")).then(|| "EFI System".into()),
            ..Default::default()
        }
    }

    /// nvme0n1 (500G): Windows EFI + C:, 300G free. sda: USB stick.
    fn machine() -> System {
        System {
            devices: vec![
                dev("/dev/nvme0n1", "disk", 500, None, None, None),
                dev("/dev/nvme0n1p1", "part", 1, Some("vfat"), Some("EFI system partition"), Some("nvme0n1")),
                dev("/dev/nvme0n1p2", "part", 199, Some("ntfs"), Some("Basic data partition"), Some("nvme0n1")),
                BlockDevice { tran: Some("usb".into()), ..dev("/dev/sda", "disk", 16, None, None, None) },
                dev("/dev/sda1", "part", 1, Some("iso9660"), None, Some("sda")),
            ],
            ..Default::default()
        }
    }

    #[test]
    fn dual_boot_on_free_space() {
        let storage = Storage {
            partitioning: vec![DiskPlan { disk: "auto".into(), wipe: false, add: default_layout() }],
            other: vec![OtherMount { partition: "/dev/nvme0n1p2".into(), mountpoint: "/mnt/windows".into() }],
            ..Default::default()
        };
        let plan = plan(&storage, &machine()).unwrap();
        assert_eq!(plan.disks[0].disk, "/dev/nvme0n1");
        let mounts: Vec<(&str, Part)> = plan.mounts.iter().map(|(m, p)| (m.as_str(), p.clone())).collect();
        assert_eq!(mounts[0], ("", Part::New { disk: "/dev/nvme0n1".into(), name: "arch-root".into() }));
        assert_eq!(mounts[1], ("boot", Part::New { disk: "/dev/nvme0n1".into(), name: "arch-efi".into() }));
        assert_eq!(mounts[2].0, "mnt/windows");
        assert!(plan.format_boot);
        assert_eq!(sfdisk_script(&plan.disks[0]), "size=1GiB, type=uefi, name=\"arch-efi\"\ntype=linux, name=\"arch-root\"\n");
    }

    #[test]
    fn wipe_drops_old_partitions_and_size_is_checked() {
        let storage = Storage {
            partitioning: vec![DiskPlan {
                disk: "/dev/nvme0n1".into(),
                wipe: true,
                add: vec![
                    NewPartition { name: "efi".into(), kind: PartitionType::Efi, size: "rest".into() },
                    NewPartition { name: "root".into(), kind: PartitionType::Linux, size: "600GiB".into() },
                ],
            }],
            efi: Efi { partition: "/dev/nvme0n1p1".into(), format: FormatChoice::Keep },
            ..Default::default()
        };
        let errors = plan(&storage, &machine()).unwrap_err();
        assert!(errors.iter().any(|e| e.contains("only allowed on the last")), "{errors:?}");
        assert!(errors.iter().any(|e| e.contains("need 600G but only 500G is free")), "{errors:?}");
        assert!(errors.iter().any(|e| e.contains("gets wiped")), "{errors:?}");
    }

    #[test]
    fn existing_partitions() {
        let storage = Storage {
            root: Some(Root { partition: "PARTLABEL=Basic data partition".into() }),
            home: Some(Home { partition: "/dev/nvme0n1p2".into(), format: FormatChoice::Auto }),
            ..Default::default()
        };
        let errors = plan(&storage, &machine()).unwrap_err();
        assert_eq!(errors, ["/dev/nvme0n1p2 is used for both / and /home"]);

        let storage = Storage { root: Some(Root { partition: "/dev/nvme0n1p2".into() }), ..Default::default() };
        let plan = plan(&storage, &machine()).unwrap();
        assert_eq!(plan.mounts[1].1, Part::Existing { path: "/dev/nvme0n1p1".into(), blank: false });
        assert!(!plan.format_boot);
    }
}
