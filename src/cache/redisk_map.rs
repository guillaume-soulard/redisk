use crate::server::{RediskKeyValue, RediskStorageAddress, RediskValue, TTL};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::ops::Add;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub struct RediskMap {
    map: HashMap<String, RediskKeyValue>,
    file: File,
}

#[derive(Serialize, Deserialize)]
pub struct Record {
    pub key: String,
    pub value: Vec<u8>,
    pub deleted: bool,
    pub expires_at: Option<u64>,
}

fn read_record(file: &mut File, offset: u64) -> Result<Record> {
    file.seek(SeekFrom::Start(offset))?;
    match bincode::deserialize_from(file) {
        Ok(record) => Ok(record),
        Err(e) => Err(e.into()),
    }
}

fn read_from_file(file: &mut File, value: RediskKeyValue) -> Option<RediskKeyValue> {
    match value.storage_address {
        Some(address) => match read_record(file, address.offset) {
            Ok(record) => {
                let value = RediskKeyValue {
                    value: record.value,
                    mounted: false,
                    deleted: record.deleted,
                    ttl: record.expires_at,
                    storage_address: Some(address),
                };
                Some(value)
            }
            Err(_) => None,
        },
        None => None,
    }
}

fn write_on_file(
    file: &mut File,
    map: &mut HashMap<String, RediskKeyValue>,
    key: String,
    value: &RediskKeyValue,
) {
    let expires_at = value.ttl.map(|t| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + t
    });
    let record = Record {
        key: key.clone(),
        value: value.value.clone(),
        deleted: value.deleted,
        expires_at,
    };
    let offset = file.seek(SeekFrom::End(0)).unwrap();
    match map.get_mut(&key) {
        Some(v) => {
            v.mounted = false;
        }
        None => {
            let value = RediskKeyValue {
                value: vec![],
                mounted: false,
                deleted: false,
                ttl: None,
                storage_address: Some(RediskStorageAddress { offset }),
            };
            map.insert(key, value);
        }
    };
    bincode::serialize_into(&(*file), &record).unwrap();
    file.flush().unwrap();
}

impl RediskMap {
    pub fn new(db: u32) -> Self {
        let path = format!("db{}.rdat", db);
        let mut map = HashMap::new();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)
            .unwrap();

        let mut offset = 0;
        let file_size = file.metadata().unwrap().len();
        while offset < file_size {
            match read_record(&mut file, offset) {
                Ok(record) => {
                    let record_size = bincode::serialized_size(&record).unwrap();
                    let value = RediskKeyValue {
                        value: vec![],
                        mounted: false,
                        deleted: false,
                        ttl: None,
                        storage_address: Some(RediskStorageAddress { offset }),
                    };
                    map.insert(record.key, value);
                    offset += record_size;
                }
                Err(_) => break,
            }
        }

        Self { map, file }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &RediskKeyValue)> {
        let now = get_now();
        self.map
            .iter()
            .filter(move |(_, v)| v.ttl.is_none() || v.ttl.unwrap() > now)
    }

    pub fn mount(&mut self, key: String) -> Option<RediskKeyValue> {
        match self.map.get_mut(&key) {
            Some(v) => match read_from_file(&mut self.file, v.clone()) {
                Some(r) => {
                    v.value = r.value;
                    v.mounted = true;
                    Some(v.clone())
                }
                None => None,
            },
            None => {
                let val = RediskKeyValue {
                    mounted: true,
                    ttl: None,
                    deleted: false,
                    value: vec![],
                    storage_address: None,
                };
                self.map.insert(key, val.clone());
                Some(val)
            }
        }
    }

    pub fn unmount(&mut self, key: &str) -> Option<RediskKeyValue> {
        let value: Option<RediskKeyValue>;
        {
            value = match self.map.get(key) {
                Some(v) => Some(v.clone()),
                None => None,
            };
        }
        match value {
            Some(mut v) => {
                v.mounted = false;
                v.value = vec![];
                write_on_file(&mut self.file, &mut self.map, key.to_string(), &v);
                self.map.insert(key.to_string(), v.clone());
                Some(v)
            }
            None => None,
        }
    }

    pub fn set(&mut self, key: String, new_value: RediskValue, ttl: TTL) -> Option<RediskKeyValue> {
        let expiration = get_next_timestamp_by_duration(ttl);
        let value: Option<RediskKeyValue>;
        {
            value = match self.map.get(&key) {
                Some(v) => Some(v.clone()),
                None => None,
            };
        }
        match value {
            Some(mut v) => {
                v.ttl = expiration;
                v.value = new_value;
                if !v.mounted {
                    write_on_file(&mut self.file, &mut self.map, key.to_string(), &v);
                }
                Some(v.clone())
            }
            None => {
                let new_value = RediskKeyValue {
                    mounted: false,
                    ttl: expiration,
                    deleted: false,
                    value: new_value,
                    storage_address: None,
                };
                self.map.insert(key, new_value.clone());
                Some(new_value)
            }
        }
    }

    pub fn get(&mut self, key: String) -> Option<RediskKeyValue> {
        if let Some(existing) = self.map.get(&key) {
            return if existing.mounted {
                if let Some(ttl) = existing.ttl {
                    if ttl < get_now() {
                        self.map.remove(&key);
                        return None;
                    }
                }
                Some(existing.clone())
            } else {
                read_from_file(&mut self.file, existing.clone())
            }
        }
        None
    }

    pub fn delete(&mut self, key: String) -> Option<RediskKeyValue> {
        let value: Option<RediskKeyValue>;
        {
            value = match self.map.get(&key) {
                Some(v) => Some(v.clone()),
                None => None,
            };
        }
        if let Some(mut existing) = value {
            existing.deleted = true;
            if !existing.mounted {
                write_on_file(&mut self.file, &mut self.map, key.to_string(), &existing);
            }
            return self.map.remove(&key);
        }
        None
    }

    pub fn expire(&mut self, key: String, ttl: TTL) -> Option<RediskKeyValue> {
        let value: Option<RediskKeyValue>;
        {
            value = match self.map.get(&key) {
                Some(v) => Some(v.clone()),
                None => None,
            };
        }
        match value {
            Some(mut v) => {
                let expiration = get_next_timestamp_by_duration(ttl);
                v.ttl = expiration;
                if !v.mounted {
                    write_on_file(&mut self.file, &mut self.map, key.to_string(), &v);
                }
                Some(v)
            }
            None => None,
        }
    }

    pub fn persist(&mut self, key: String) -> Option<RediskKeyValue> {
        let value: Option<RediskKeyValue>;
        {
            value = match self.map.get(&key) {
                Some(v) => Some(v.clone()),
                None => None,
            };
        }
        match value {
            Some(mut v) if v.ttl.is_some() => {
                v.ttl = None;
                if v.mounted {
                    write_on_file(&mut self.file, &mut self.map, key.to_string(), &v);
                }
                Some(v)
            }
            _ => None,
        }
    }
}

fn get_now() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn get_next_timestamp_by_duration(ttl: TTL) -> TTL {
    match ttl {
        Some(t) => Some(
            SystemTime::now()
                .add(Duration::from_secs(t))
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        ),
        None => None,
    }
}
