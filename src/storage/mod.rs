use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug)]
pub struct Record {
    pub key: String,
    pub value: Vec<u8>,
    pub deleted: bool,
    pub version: u64,
}

pub struct StorageEngine {
    _path: PathBuf,
    file: File,
    index: HashMap<String, (u64, u64)>, // key to file offset
}

impl StorageEngine {
    pub fn new(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)?;

        let mut index: HashMap<String, (u64, u64)> = HashMap::new();
        let mut offset = 0;

        let file_size = file.metadata()?.len();
        while offset < file_size {
            file.seek(SeekFrom::Start(offset))?;
            match bincode::deserialize_from::<&File, Record>(&file) {
                Ok(record) => {
                    let record_size = bincode::serialized_size(&record)?;
                    if !record.deleted {
                        index.insert(record.key, (offset, record.version));
                    } else {
                        index.remove(&record.key);
                    }
                    offset += record_size;
                }
                Err(_) => break,
            }
        }

        Ok(Self {
            _path: path,
            file,
            index,
        })
    }

    pub fn set(&mut self, key: String, value: Vec<u8>) -> Result<()> {
        let mut record = Record {
            key: key.clone(),
            value,
            deleted: false,
            version: 0,
        };
        let offset = self.file.seek(SeekFrom::End(0))?;
        let existing: &(u64, u64) = self.index.get(&key)
            .map_or_else(|| &(0u64, 0u64), |e|e);
        record.version = existing.1 + 1;
        bincode::serialize_into(&self.file, &record)?;
        self.file.flush()?;
        self.index.insert(key, (offset, record.version));
        Ok(())
    }

    pub fn get(&mut self, key: &str) -> Result<Option<Vec<u8>>> {
        if let Some(&offset) = self.index.get(key) {
            self.file.seek(SeekFrom::Start(offset.0))?;
            let record: Record = bincode::deserialize_from(&self.file)?;
            if record.deleted {
                return Ok(None);
            }
            return Ok(Some(record.value));
        }
        Ok(None)
    }

    pub fn delete(&mut self, key: &str) -> Result<()> {
        if let Some(v) = self.index.remove(key) {
            let record = Record {
                key: key.to_string(),
                value: vec![],
                deleted: true,
                version: v.1 + 1,
            };
            self.file.seek(SeekFrom::End(0))?;
            bincode::serialize_into(&self.file, &record)?;
            self.file.flush()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_storage_basic() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        let mut engine = StorageEngine::new(path).unwrap();

        engine.set("key1".to_string(), b"val1".to_vec()).unwrap();
        assert_eq!(engine.get("key1").unwrap(), Some(b"val1".to_vec()));

        engine
            .set("key1".to_string(), b"val1_updated".to_vec())
            .unwrap();
        assert_eq!(engine.get("key1").unwrap(), Some(b"val1_updated".to_vec()));

        engine.delete("key1").unwrap();
        assert_eq!(engine.get("key1").unwrap(), None);
    }

    #[test]
    fn test_rebuild_index() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_path_buf();

        {
            let mut engine = StorageEngine::new(&path).unwrap();
            engine.set("key1".to_string(), b"val1".to_vec()).unwrap();
            engine.set("key2".to_string(), b"val2".to_vec()).unwrap();
        }

        {
            let mut engine = StorageEngine::new(&path).unwrap();
            assert_eq!(engine.get("key1").unwrap(), Some(b"val1".to_vec()));
            assert_eq!(engine.get("key2").unwrap(), Some(b"val2".to_vec()));
        }
    }
}
