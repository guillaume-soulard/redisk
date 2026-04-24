use crate::server::{RediskKeyValue, RediskStorageAddress, RediskValue, TTL};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom};
use std::ops::Add;
use std::time::{Duration, SystemTime};

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

impl RediskMap {
    pub fn new(db: u32) -> Self {
        let path = format!("db{}.rdat", db);
        let mut map = HashMap::new();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path).unwrap();

            let mut offset = 0;
            let file_size = file.metadata().unwrap().len();
            while offset < file_size {
                match read_record(&mut file, offset) {
                    Ok(record) => {
                        let record_size = bincode::serialized_size(&record).unwrap();
                        let mut value = RediskKeyValue {
                            value: vec![],
                            mounted: false,
                            deleted: false,
                            ttl: None,
                            storage_address: Some(RediskStorageAddress{
                                offset,
                            }),
                        };
                        map.insert(record.key, value);
                        offset += record_size;
                    }
                    Err(_) => break,
                }
            }

        Self {
            map,
            file
        }
    }

    pub fn iter(&self) -> impl Iterator<Item=(&String, &RediskKeyValue)> {
        let now = get_now();
        self.map.iter()
            .filter(move |(_, v)| v.ttl.is_none() || v.ttl.unwrap() > now)
    }

    pub fn mount(&mut self, key: String) -> Option<RediskKeyValue> {
        match self.map.get_mut(&key) {
            Some(v) => {
                match v.clone().storage_address {
                    Some(address) => {
                        match read_record(&mut self.file, address.offset) {
                            Ok(_) | Err(_) => todo!(),
                        }
                    },
                    None => {}
                }
                v.mounted = true;
                Some(v.clone())
            }
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

    pub fn is_mounted(&self, key: &str) -> bool {
        self.map.get(key).map_or(false, |v| v.mounted)
    }

    pub fn unmount(&mut self, key: &str) -> Option<RediskKeyValue> {
        let value = self.map.get_mut(key);
        match value {
            Some(v) => {
                (*v).mounted = false;
                Some(v.clone())
            },
            None => None,
        }
    }

    pub fn set(&mut self, key: String, value: RediskValue, ttl: TTL) -> Option<RediskKeyValue> {
        let expiration = get_next_timestamp_by_duration(ttl);
        match self.map.get_mut(&key) {
            Some(v) => {
                v.ttl = expiration;
                v.value = value;
                Some(v.clone())
            },
            None => {
                let new_value = RediskKeyValue {
                    mounted: false,
                    ttl: expiration,
                    deleted: false,
                    value,
                    storage_address: None,
                };
                self.map.insert(key, new_value.clone());
                Some(new_value)
            }
        }
    }

    pub fn get(&mut self, key: String) -> Option<RediskKeyValue> {
        if let Some(existing) = self.map.get(&key) {
            if let Some(ttl) = existing.ttl {
                if ttl < get_now() {
                    self.map.remove(&key);
                    return None;
                }
            }
            return Some(existing.clone());
        }
        None
    }

    pub fn delete(&mut self, key: String) -> Option<RediskKeyValue> {
        self.map.remove(&key)
    }

    pub fn expire(&mut self, key: String, ttl: TTL) -> Option<RediskKeyValue> {
        match self.map.get_mut(&key) {
            Some(v) => {
                let expiration = get_next_timestamp_by_duration(ttl);
                v.ttl = expiration;
                Some(v.clone())
            },
            None => None
        }
    }

    pub fn persist(&mut self, key: String) -> Option<RediskKeyValue> {
        match self.map.get_mut(&key) {
            Some(v) if v.ttl.is_some() => {
                v.ttl = None;
                Some(v.clone())
            },
            _ => None
        }
    }
}

fn get_now() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs()
}

fn get_next_timestamp_by_duration(ttl: TTL) -> TTL {
    match ttl {
        Some(t) => {
            Some(SystemTime::now()
                .add(Duration::from_secs(t))
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs())
        },
        None => None
    }
}