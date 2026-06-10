//! File
//!
//! reference: <https://wiki.osdev.org/FAT#Directories_on_FAT12.2F16.2F32>

use super::*;

#[derive(Debug, Clone)]
pub struct File {
    /// The current offset in the file
    offset: usize,
    /// The current cluster of this file
    current_cluster: Cluster,
    /// DirEntry of this file
    entry: DirEntry,
    /// The file system handle that contains this file
    handle: Fat16Handle,
}

impl File {
    pub fn new(handle: Fat16Handle, entry: DirEntry) -> Self {
        Self {
            offset: 0,
            current_cluster: entry.cluster,
            entry,
            handle,
        }
    }

    pub fn length(&self) -> usize {
        self.entry.size as usize
    }
}

impl Read for File {
    fn read(&mut self, buf: &mut [u8]) -> FsResult<usize> {
        if buf.is_empty() || self.offset >= self.length() {
            return Ok(0);
        }

        let sectors_per_cluster =
            self.handle.bpb.sectors_per_cluster() as usize;
        let cluster_size = sectors_per_cluster * BLOCK_SIZE;

        if cluster_size == 0 {
            return Err(FsError::InvalidOperation);
        }

        // 不能超过文件剩余长度
        let read_len =
            buf.len().min(self.length() - self.offset);

        let mut read_count = 0;

        while read_count < read_len {
            match self.current_cluster {
                Cluster::BAD | Cluster::INVALID | Cluster::EMPTY => {
                    return Err(FsError::BadCluster);
                }
                Cluster::END_OF_FILE => break,
                _ => {}
            }

            // 当前偏移在簇、扇区中的位置
            let cluster_offset = self.offset % cluster_size;
            let sector_index = cluster_offset / BLOCK_SIZE;
            let sector_offset = cluster_offset % BLOCK_SIZE;

            let first_sector =
                self.handle.cluster_to_sector(&self.current_cluster);
            let sector = first_sector + sector_index;

            let mut block = Block512::default();
            self.handle.inner.read_block(sector, &mut block)?;

            let copy_len = (BLOCK_SIZE - sector_offset)
                .min(read_len - read_count);

            buf[read_count..read_count + copy_len]
                .copy_from_slice(
                    &block.as_ref()
                        [sector_offset..sector_offset + copy_len],
                );

            read_count += copy_len;
            self.offset += copy_len;

            // 恰好读完当前簇，沿 FAT 链进入下一簇
            if self.offset % cluster_size == 0
                && self.offset < self.length()
            {
                let next = self
                    .handle
                    .next_cluster(&self.current_cluster)?;

                match next {
                    Cluster::BAD | Cluster::INVALID | Cluster::EMPTY => {
                        return Err(FsError::BadCluster);
                    }
                    Cluster::END_OF_FILE => break,
                    _ => self.current_cluster = next,
                }
            }
        }

        Ok(read_count)
    }
}

// NOTE: `Seek` trait is not required for this lab
impl Seek for File {
    fn seek(&mut self, pos: SeekFrom) -> FsResult<usize> {
        unimplemented!()
    }
}

// NOTE: `Write` trait is not required for this lab
impl Write for File {
    fn write(&mut self, _buf: &[u8]) -> FsResult<usize> {
        unimplemented!()
    }

    fn flush(&mut self) -> FsResult {
        unimplemented!()
    }
}
