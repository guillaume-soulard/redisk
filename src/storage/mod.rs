use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;
use rand::seq::IteratorRandom;

use std::time::{SystemTime, UNIX_EPOCH};
use crate::server::{RediskValue, TTL};

#[derive(Serialize, Deserialize, Debug)]
pub struct Record {
    pub key: String,
    pub value: Vec<u8>,
    pub deleted: bool,
    pub version: u64,
    pub expires_at: Option<u64>, // Unix timestamp in seconds
    pub db: u32,
}

pub struct StorageEngine {
    _path: PathBuf,
    file: File,
    indices: HashMap<u32, HashMap<String, (u64, u64, Option<u64>)>>, // db to (key to (file offset, version, expires_at))
}

impl StorageEngine {
    pub fn new(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)?;

        let mut indices: HashMap<u32, HashMap<String, (u64, u64, Option<u64>)>> = HashMap::new();
        let mut offset = 0;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs();

        let file_size = file.metadata()?.len();
        while offset < file_size {
            match read_record(&mut file, offset) {
                Ok(record) => {
                    let record_size = bincode::serialized_size(&record)?;
                    let db_index = indices.entry(record.db).or_default();
                    if !record.deleted {
                        if let Some(expires_at) = record.expires_at {
                            if expires_at <= now {
                                db_index.remove(&record.key);
                            } else {
                                db_index.insert(record.key, (offset, record.version, record.expires_at));
                            }
                        } else {
                            db_index.insert(record.key, (offset, record.version, record.expires_at));
                        }
                    } else {
                        db_index.remove(&record.key);
                    }
                    offset += record_size;
                }
                Err(_) => break,
            }
        }

        Ok(Self {
            _path: path,
            file,
            indices,
        })
    }

    pub fn set(&mut self, db: u32, key: String, value: RediskValue, ttl: TTL) -> Option<RediskValue> {
        let expires_at = ttl.map(|t| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + t
        });

        let mut record = Record {
            key: key.clone(),
            value,
            deleted: false,
            version: 0,
            expires_at,
            db,
        };
        let offset = self.file.seek(SeekFrom::End(0))?;
        let db_index = self.indices.entry(db).or_default();
        let existing: &(u64, u64, Option<u64>) = db_index.get(&key)
            .map_or_else(|| &(0u64, 0u64, None), |e|e);
        record.version = existing.1 + 1;
        bincode::serialize_into(&self.file, &record)?;
        self.file.flush()?;
        db_index.insert(key, (offset, record.version, expires_at));
        Ok(())
    }

    pub fn get(&mut self, db: u32, key: &str) -> Result<Option<Vec<u8>>> {
        let db_index = self.indices.entry(db).or_default();
        if let Some(&entry) = db_index.get(key) {
            let (offset, _, expires_at) = entry;
            if let Some(exp) = expires_at {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)?
                    .as_secs();
                if exp <= now {
                    db_index.remove(key);
                    return Ok(None);
                }
            }

            return match read_record(&mut self.file, offset) {
                Ok(record) => {
                    if record.deleted {
                        return Ok(None);
                    }
                    Ok(Some(record.value))
                },
                Err(_) => Ok(None),
            }
        }
        Ok(None)
    }

    pub fn get_ttl(&self, db: u32, key: &str) -> Result<Option<Option<u64>>> {
        if let Some(db_index) = self.indices.get(&db) {
            if let Some(&(_, _, expires_at)) = db_index.get(key) {
                return if let Some(exp) = expires_at {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)?
                        .as_secs();
                    if exp <= now {
                        return Ok(Some(None));
                    }
                    Ok(Some(Some(exp - now)))
                } else {
                    Ok(Some(None))
                }
            }
        }
        Ok(None)
    }

    pub fn exists(&self, db: u32, key: &str) -> bool {
        if let Some(db_index) = self.indices.get(&db) {
            if let Some(&(_, _, expires_at)) = db_index.get(key) {
                if let Some(exp) = expires_at {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    if exp <= now {
                        return false;
                    }
                }
                return true;
            }
        }
        false
    }

    pub fn expire(&mut self, db: u32, key: &str, ttl: TTL) -> Option<RediskValue> {
        let db_index = self.indices.entry(db).or_default();
        if let Some(&(offset, version, _)) = db_index.get(key) {
            let mut record = read_record(&mut self.file, offset)?;
            let expires_at = Some(SystemTime::now()
                .duration_since(UNIX_EPOCH)?
                .as_secs() + seconds);
            
            record.expires_at = expires_at;
            record.version = version + 1;
            record.db = db;
            
            let new_offset = self.file.seek(SeekFrom::End(0))?;
            bincode::serialize_into(&self.file, &record)?;
            self.file.flush()?;
            
            db_index.insert(key.to_string(), (new_offset, record.version, expires_at));
            return Ok(true);
        }
        Ok(false)
    }

    pub fn persist(&mut self, db: u32, key: &str) -> Option<RediskValue> {
        let db_index = self.indices.entry(db).or_default();
        if let Some(&(offset, version, expires_at)) = db_index.get(key) {
            if expires_at.is_none() {
                return Ok(false);
            }

            let mut record = read_record(&mut self.file, offset)?;
            record.expires_at = None;
            record.version = version + 1;
            record.db = db;

            let new_offset = self.file.seek(SeekFrom::End(0))?;
            bincode::serialize_into(&self.file, &record)?;
            self.file.flush()?;

            db_index.insert(key.to_string(), (new_offset, record.version, None));
            return Ok(true);
        }
        Ok(false)
    }

    pub fn keys(&self, db: u32, pattern: &str) -> Vec<String> {
        let db_index = match self.indices.get(&db) {
            Some(idx) => idx,
            None => return vec![],
        };
        let regex_pattern = pattern
            .replace("*", ".*")
            .replace("?", ".");
        let regex = match regex::Regex::new(&format!("^{}$", regex_pattern)) {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        db_index
            .iter()
            .filter(|(key, entry)| {
                let (_, _, expires_at) = entry;
                if let Some(exp) = *expires_at {
                    if exp <= now {
                        return false;
                    }
                }
                regex.is_match(key)
            })
            .map(|(key, _)| key.clone())
            .collect()
    }

    pub fn scan(&self, db: u32, cursor: usize, pattern: Option<String>, count: usize) -> (usize, Vec<String>) {
        let db_index = match self.indices.get(&db) {
            Some(idx) => idx,
            None => return (0, vec![]),
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let keys: Vec<String> = db_index
            .iter()
            .filter(|(_, entry)| {
                let (_, _, expires_at) = entry;
                if let Some(exp) = *expires_at {
                    if exp <= now {
                        return false;
                    }
                }
                true
            })
            .map(|(key, _)| key.clone())
            .collect();

        let regex = pattern.and_then(|p| {
            let rp = p.replace("*", ".*").replace("?", ".");
            regex::Regex::new(&format!("^{}$", rp)).ok()
        });

        let mut result = Vec::new();
        let mut next_cursor = cursor;

        while next_cursor < keys.len() && result.len() < count {
            let key = &keys[next_cursor];
            if let Some(ref r) = regex {
                if r.is_match(key) {
                    result.push(key.clone());
                }
            } else {
                result.push(key.clone());
            }
            next_cursor += 1;
        }

        if next_cursor >= keys.len() {
            next_cursor = 0;
        }

        (next_cursor, result)
    }

    pub fn get_type(&self, db: u32, key: &str) -> String {
        if self.exists(db, key) {
            "string".to_string()
        } else {
            "none".to_string()
        }
    }

    pub fn rename(&mut self, db: u32, old_key: &str, new_key: &str) -> Result<()> {
        let db_index = self.indices.entry(db).or_default();
        if let Some(&(offset, version, expires_at)) = db_index.get(old_key) {
            let mut record = read_record(&mut self.file, offset)?;
            
            // Delete old key
            let delete_record = Record {
                key: old_key.to_string(),
                value: vec![],
                deleted: true,
                version: version + 1,
                expires_at: None,
                db,
            };
            self.file.seek(SeekFrom::End(0))?;
            bincode::serialize_into(&self.file, &delete_record)?;
            
            // Set new key
            record.key = new_key.to_string();
            record.version = db_index.get(new_key).map_or(0, |e| e.1) + 1;
            record.db = db;
            let new_offset = self.file.seek(SeekFrom::End(0))?;
            bincode::serialize_into(&self.file, &record)?;
            self.file.flush()?;

            db_index.remove(old_key);
            db_index.insert(new_key.to_string(), (new_offset, record.version, expires_at));
            
            Ok(())
        } else {
            Err(anyhow!("no such key"))
        }
    }

    pub fn randomkey(&self, db: u32) -> Option<String> {
        let db_index = self.indices.get(&db)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut rng = rand::thread_rng();
        db_index
            .iter()
            .filter(|(_, entry)| {
                let (_, _, expires_at) = entry;
                if let Some(exp) = *expires_at {
                    if exp <= now {
                        return false;
                    }
                }
                true
            })
            .choose(&mut rng)
            .map(|(key, _)| key.clone())
    }

    pub fn delete(&mut self, db: u32, key: &str) -> Option<RediskValue> {
        let db_index = self.indices.entry(db).or_default();
        if let Some(v) = db_index.remove(key) {
            let record = Record {
                key: key.to_string(),
                value: vec![],
                deleted: true,
                version: v.1 + 1,
                expires_at: None,
                db,
            };
            self.file.seek(SeekFrom::End(0))?;
            bincode::serialize_into(&self.file, &record)?;
            self.file.flush()?;
        }
        Ok(())
    }

    pub fn move_key(&mut self, src_db: u32, dest_db: u32, key: &str) -> Result<bool> {
        if src_db == dest_db {
            return Err(anyhow!("source and destination databases are the same"));
        }

        // Check if key exists in src_db
        let src_index = match self.indices.get(&src_db) {
            Some(idx) => idx,
            None => return Ok(false),
        };

        let entry = match src_index.get(key) {
            Some(&e) => e,
            None => return Ok(false),
        };

        // Check if key already exists in dest_db
        if self.exists(dest_db, key) {
            return Ok(false);
        }

        let (offset, version, expires_at) = entry;
        let mut record = read_record(&mut self.file, offset)?;

        // Mark as deleted in src_db
        let delete_record = Record {
            key: key.to_string(),
            value: vec![],
            deleted: true,
            version: version + 1,
            expires_at: None,
            db: src_db,
        };
        self.file.seek(SeekFrom::End(0))?;
        bincode::serialize_into(&self.file, &delete_record)?;

        // Insert in dest_db
        record.db = dest_db;
        record.version = self.indices.get(&dest_db).and_then(|idx| idx.get(key)).map_or(0, |e| e.1) + 1;
        let new_offset = self.file.seek(SeekFrom::End(0))?;
        bincode::serialize_into(&self.file, &record)?;
        self.file.flush()?;

        // Update indices
        self.indices.get_mut(&src_db).unwrap().remove(key);
        self.indices.entry(dest_db).or_default().insert(key.to_string(), (new_offset, record.version, expires_at));

        Ok(true)
    }

    pub fn compact(&mut self) -> Result<()> {
        let temp_path = "temp.rdat";
        let mut temp_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&temp_path)?;
        let mut indices: HashMap<u32, HashMap<String, (u64, u64, Option<u64>)>> = HashMap::new();

        let dbs: Vec<u32> = self.indices.keys().cloned().collect();
        for db in dbs {
            let keys: Vec<String> = self.indices.get(&db).unwrap().keys().cloned().collect();
            for key in keys {
                let (offset, version, expires_at) = self.indices.get(&db).unwrap().get(&key).unwrap();
                let record = read_record(&mut self.file, *offset)?;
                let temp_offset = temp_file.seek(SeekFrom::End(0))?;
                bincode::serialize_into(&temp_file, &record)?;
                temp_file.flush()?;
                indices.entry(db).or_default().insert(key.clone(), (temp_offset, *version, *expires_at));
            }
        }
        
        temp_file.flush()?;
        std::fs::remove_file(self._path.to_str().unwrap())?;
        std::fs::rename(temp_path, self._path.to_str().unwrap())?;
        
        self.file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self._path)?;
        self.indices = indices;
        
        Ok(())
    }
}

fn read_record(file: &mut File, offset: u64) -> Result<Record> {
    file.seek(SeekFrom::Start(offset))?;
    match bincode::deserialize_from(file) {
        Ok(record) => Ok(record),
        Err(e) => Err(e.into()),
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

        engine.set(0, "key1".to_string(), b"val1".to_vec(), None).unwrap();
        assert_eq!(engine.get(0, "key1").unwrap(), Some(b"val1".to_vec()));

        engine
            .set(0, "key1".to_string(), b"val1_updated".to_vec(), None)
            .unwrap();
        assert_eq!(engine.get(0, "key1").unwrap(), Some(b"val1_updated".to_vec()));

        engine.delete(0, "key1").unwrap();
        assert_eq!(engine.get(0, "key1").unwrap(), None);
    }

    #[test]
    fn test_rebuild_index() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_path_buf();

        {
            let mut engine = StorageEngine::new(&path).unwrap();
            engine.set(0, "key1".to_string(), b"val1".to_vec(), None).unwrap();
            engine.set(0, "key2".to_string(), b"val2".to_vec(), None).unwrap();
        }

        {
            let mut engine = StorageEngine::new(&path).unwrap();
            assert_eq!(engine.get(0, "key1").unwrap(), Some(b"val1".to_vec()));
            assert_eq!(engine.get(0, "key2").unwrap(), Some(b"val2".to_vec()));
        }
    }

    #[test]
    fn test_ttl() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        let mut engine = StorageEngine::new(path).unwrap();

        engine.set(0, "key1".to_string(), b"val1".to_vec(), Some(5)).unwrap();
        assert_eq!(engine.get(0, "key1").unwrap(), Some(b"val1".to_vec()));

        let ttl = engine.get_ttl(0, "key1").unwrap();
        assert!(ttl.is_some());
        assert!(ttl.unwrap() <= Some(5));

        std::thread::sleep(std::time::Duration::from_secs(6));
        assert_eq!(engine.get(0, "key1").unwrap(), None);
        assert!(engine.get_ttl(0, "key1").unwrap().is_none());
    }

    #[test]
    fn test_get_ttl_no_expire() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        let mut engine = StorageEngine::new(path).unwrap();

        engine.set(0, "key1".to_string(), b"val1".to_vec(), None).unwrap();
        let ttl = engine.get_ttl(0, "key1").unwrap();
        assert_eq!(ttl, None);
    }

    #[test]
    fn test_get_ttl_non_existent() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        let engine = StorageEngine::new(path).unwrap();

        let ttl = engine.get_ttl(0, "key1").unwrap();
        assert_eq!(ttl, None);
    }

    #[test]
    fn test_exists() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        let mut engine = StorageEngine::new(path).unwrap();

        engine.set(0, "key1".to_string(), b"val1".to_vec(), None).unwrap();
        assert!(engine.exists(0, "key1"));
        assert!(!engine.exists(0, "key2"));
    }

    #[test]
    fn test_expire() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        let mut engine = StorageEngine::new(path).unwrap();

        engine.set(0, "key1".to_string(), b"val1".to_vec(), None).unwrap();
        assert!(engine.expire(0, "key1", 10).unwrap());
        let ttl = engine.get_ttl(0, "key1").unwrap();
        assert!(ttl.is_some());
    }

    #[test]
    fn test_persist() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        let mut engine = StorageEngine::new(path).unwrap();

        engine.set(0, "key1".to_string(), b"val1".to_vec(), Some(10)).unwrap();
        assert!(engine.persist(0, "key1").unwrap());
        let ttl = engine.get_ttl(0, "key1").unwrap();
        assert_eq!(ttl, None);
    }

    #[test]
    fn test_keys() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        let mut engine = StorageEngine::new(path).unwrap();

        engine.set(0, "abc".to_string(), b"val".to_vec(), None).unwrap();
        engine.set(0, "abd".to_string(), b"val".to_vec(), None).unwrap();
        engine.set(0, "xyz".to_string(), b"val".to_vec(), None).unwrap();

        let mut keys = engine.keys(0, "ab*");
        keys.sort();
        assert_eq!(keys, vec!["abc", "abd"]);

        let mut keys = engine.keys(0, "?b?");
        keys.sort();
        assert_eq!(keys, vec!["abc", "abd"]);
    }

    #[test]
    fn test_scan() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        let mut engine = StorageEngine::new(path).unwrap();

        engine.set(0, "k1".to_string(), b"v".to_vec(), None).unwrap();
        engine.set(0, "k2".to_string(), b"v".to_vec(), None).unwrap();
        engine.set(0, "k3".to_string(), b"v".to_vec(), None).unwrap();

        let (cursor, keys) = engine.scan(0, 0, None, 2);
        assert_eq!(keys.len(), 2);
        assert_eq!(cursor, 2);

        let (cursor, keys) = engine.scan(0, cursor, None, 2);
        assert_eq!(keys.len(), 1);
        assert_eq!(cursor, 0);
    }

    #[test]
    fn test_type() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        let mut engine = StorageEngine::new(path).unwrap();

        engine.set(0, "k1".to_string(), b"v".to_vec(), None).unwrap();
        assert_eq!(engine.get_type(0, "k1"), "string");
        assert_eq!(engine.get_type(0, "k2"), "none");
    }

    #[test]
    fn test_rename() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        let mut engine = StorageEngine::new(path).unwrap();

        engine.set(0, "k1".to_string(), b"v1".to_vec(), None).unwrap();
        engine.rename(0, "k1", "k2").unwrap();
        
        assert_eq!(engine.get(0, "k1").unwrap(), None);
        assert_eq!(engine.get(0, "k2").unwrap(), Some(b"v1".to_vec()));
    }

    #[test]
    fn test_randomkey() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        let mut engine = StorageEngine::new(path).unwrap();

        assert_eq!(engine.randomkey(0), None);

        engine.set(0, "k1".to_string(), b"v".to_vec(), None).unwrap();
        assert_eq!(engine.randomkey(0), Some("k1".to_string()));
    }

    #[test]
    fn test_move() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        let mut engine = StorageEngine::new(path).unwrap();

        engine.set(0, "k1".to_string(), b"v1".to_vec(), None).unwrap();
        assert!(engine.move_key(0, 1, "k1").unwrap());
        
        assert_eq!(engine.get(0, "k1").unwrap(), None);
        assert_eq!(engine.get(1, "k1").unwrap(), Some(b"v1".to_vec()));
    }
}
