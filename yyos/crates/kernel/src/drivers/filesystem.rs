use alloc::{boxed::Box, format};
use storage::{fat16::Fat16, mbr::*, *};

use super::ata::*;

pub static ROOTFS: spin::Once<Mount> = spin::Once::new();

pub fn get_rootfs() -> Option<&'static Mount> {
    ROOTFS.get()
}

pub fn init() -> FsResult {
    info!("Opening disk device...");

    let drive = AtaDrive::open(0, 0).ok_or(FsError::DeviceError(DeviceError::UnknownDevice))?;
    let table = MbrTable::parse(drive)?;
    let part = table
        .partitions()?
        .into_iter()
        .next()
        .ok_or(FsError::InvalidOperation)?;

    info!("Mounting filesystem...");

    let fs = Fat16::new(part)?;
    ROOTFS.call_once(|| Mount::new(Box::new(fs), "/".into()));

    trace!("Root filesystem: {:#?}", ROOTFS.get());
    info!("Initialized Filesystem.");
    Ok(())
}

pub fn ls(root_path: &str) -> bool {
    let Some(rootfs) = get_rootfs() else {
        warn!("Root filesystem is not mounted");
        return false;
    };

    let iter = match rootfs.read_dir(root_path) {
        Ok(iter) => iter,
        Err(err) => {
            warn!("{:?}", err);
            return false;
        }
    };

    // FIXME: format and print the file metadata
    //      - use `for meta in iter` to iterate over the entries
    //      - use `crate::humanized_size_short` for file size
    //      - add '/' to the end of directory names
    //      - format the date as you like
    //      - do not forget to print the table header
    println!("{:<32} {:>10}  {}", "NAME", "SIZE", "MODIFIED");

    for meta in iter {
        let name = if meta.is_dir() {
            format!("{}/", meta.name)
        } else {
            meta.name
        };

        let (size, unit) = crate::humanized_size_short(meta.len as u64);

        match meta.modified {
            Some(time) => {
                println!("{:<32} {:>7.1} {:<2}  {}", name, size, unit, time);
            }
            None => {
                println!("{:<32} {:>7.1} {:<2}  -", name, size, unit);
            }
        }
    }

    true
}
