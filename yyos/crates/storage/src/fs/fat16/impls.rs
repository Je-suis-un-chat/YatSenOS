use super::*;

impl Fat16Impl {
    pub fn new(inner: impl BlockDevice<Block512>) -> FsResult<Self> {
        let mut block = Block::default();
        let block_size = Block512::size();

        inner.read_block(0, &mut block)?;
        let bpb = Fat16Bpb::new(block.as_ref())?;

        trace!("Loading Fat16 Volume: {:#?}", bpb);

        let fat_start = bpb.reserved_sector_count() as usize;
        let root_dir_size =
            (bpb.root_entries_count() as usize * DirEntry::LEN + block_size - 1) / block_size;
        let first_root_dir_sector =
            fat_start + bpb.fat_count() as usize * bpb.sectors_per_fat() as usize;
        let first_data_sector = first_root_dir_sector + root_dir_size;

        Ok(Self {
            bpb,
            inner: Box::new(inner),
            fat_start,
            first_data_sector,
            first_root_dir_sector,
        })
    }

    pub fn cluster_to_sector(&self, cluster: &Cluster) -> usize {
        match *cluster {
            Cluster::ROOT_DIR => self.first_root_dir_sector,
            Cluster(c) => {
                (c as usize - 2) * self.bpb.sectors_per_cluster() as usize + self.first_data_sector
            }
        }
    }

    pub fn next_cluster(&self, cluster: &Cluster) -> FsResult<Cluster> {
        let offset = cluster.0 as usize * 2;
        let sector = self.fat_start + offset / BLOCK_SIZE;
        let index = offset % BLOCK_SIZE;

        let mut block = Block512::default();
        self.inner.read_block(sector, &mut block)?;

        let value = u16::from_le_bytes([block.as_ref()[index], block.as_ref()[index + 1]]);

        match value {
            0x0000 => Ok(Cluster::EMPTY),
            0xfff7 => Ok(Cluster::BAD),
            0xfff8..=0xffff => Ok(Cluster::END_OF_FILE),
            0x0002..=0xffef => Ok(Cluster(value as u32)),
            _ => Ok(Cluster::INVALID),
        }
    }

    pub fn read_directory(&self, dir: &Directory) -> FsResult<Vec<DirEntry>> {
        let mut entries = Vec::new();
        let mut cluster = dir.cluster;

        loop {
            let start = self.cluster_to_sector(&cluster);
            let sectors = if cluster == Cluster::ROOT_DIR {
                (self.bpb.root_entries_count() as usize * DirEntry::LEN + BLOCK_SIZE - 1)
                    / BLOCK_SIZE
            } else {
                self.bpb.sectors_per_cluster() as usize
            };

            for sector in start..start + sectors {
                let mut block = Block512::default();
                self.inner.read_block(sector, &mut block)?;

                for data in block.as_ref().chunks_exact(DirEntry::LEN) {
                    if data[0] == 0x00 {
                        return Ok(entries);
                    }
                    if data[0] == 0xe5 || data[11] == Attributes::LFN.bits() {
                        continue;
                    }

                    let entry = DirEntry::parse(data)?;
                    if !entry.attributes.contains(Attributes::VOLUME_ID) {
                        entries.push(entry);
                    }
                }
            }

            if cluster == Cluster::ROOT_DIR {
                break;
            }

            cluster = self.next_cluster(&cluster)?;
            match cluster {
                Cluster::END_OF_FILE => break,
                Cluster::BAD | Cluster::INVALID | Cluster::EMPTY => {
                    return Err(FsError::BadCluster);
                }
                _ => {}
            }
        }

        Ok(entries)
    }

    pub fn find_entry(&self, dir: &Directory, name: &str) -> FsResult<DirEntry> {
        let name = ShortFileName::parse(name)?;

        self.read_directory(dir)?
            .into_iter()
            .find(|entry| entry.filename.matches(&name))
            .ok_or(FsError::FileNotFound)
    }

    pub fn find_path(&self, path: &str) -> FsResult<Option<DirEntry>> {
        let mut dir = Directory::root();
        let mut result = None;
        let mut components = path.split('/').filter(|part| !part.is_empty()).peekable();

        while let Some(name) = components.next() {
            let entry = self.find_entry(&dir, name)?;

            if components.peek().is_some() {
                if !entry.is_directory() {
                    return Err(FsError::NotADirectory);
                }
                dir = Directory::from_entry(entry.clone());
            }

            result = Some(entry);
        }

        Ok(result)
    }
}

impl FileSystem for Fat16 {
    fn read_dir(&self, path: &str) -> FsResult<Box<dyn Iterator<Item = Metadata> + Send>> {
        let dir = match self.handle.find_path(path)? {
            Some(entry) if entry.is_directory() => Directory::from_entry(entry),
            Some(_) => return Err(FsError::NotADirectory),
            None => Directory::root(),
        };

        let entries = self
            .handle
            .read_directory(&dir)?
            .into_iter()
            .map(|entry| entry.as_meta())
            .collect::<Vec<_>>();

        Ok(Box::new(entries.into_iter()))
    }

    fn open_file(&self, path: &str) -> FsResult<FileHandle> {
        let entry = self.handle.find_path(path)?.ok_or(FsError::NotAFile)?;

        if entry.is_directory() {
            return Err(FsError::NotAFile);
        }

        let metadata = entry.as_meta();
        let file = File::new(self.handle.clone(), entry);
        Ok(FileHandle::new(metadata, Box::new(file)))
    }

    fn metadata(&self, path: &str) -> FsResult<Metadata> {
        match self.handle.find_path(path)? {
            Some(entry) => Ok(entry.as_meta()),
            None => Ok(Metadata::new(
                String::from("/"),
                FileType::Directory,
                0,
                None,
                None,
                None,
            )),
        }
    }

    fn exists(&self, path: &str) -> FsResult<bool> {
        match self.handle.find_path(path) {
            Ok(_) => Ok(true),
            Err(FsError::FileNotFound) => Ok(false),
            Err(err) => Err(err),
        }
    }
}
