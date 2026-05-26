//! # Memory Pool
//!
//! Persistent memory pool with memmap2-backed storage.
//! Supports typed allocations, reset, and memory statistics.

use std::fs::OpenOptions;
use std::mem;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use memmap2::{MmapMut, MmapOptions};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// Memory statistics snapshot
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryStats {
    pub capacity_bytes: usize,
    pub used_bytes: usize,
    pub free_bytes: usize,
    pub utilization: f64,
    pub allocation_count: u64,
    pub snapshot_count: u64,
}

/// Header stored at the beginning of the memory-mapped file
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemoryHeader {
    magic: [u8; 8],
    version: u32,
    capacity: usize,
    used: usize,
    allocation_count: u64,
    snapshot_count: u64,
    created_at: DateTime<Utc>,
    last_modified: DateTime<Utc>,
}

impl MemoryHeader {
    const MAGIC: [u8; 8] = *b"OMEGAMEM";
    const SERIALIZED_SIZE: usize = 256; // Fixed size for header region

    fn new(capacity: usize) -> Self {
        Self {
            magic: Self::MAGIC,
            version: 1,
            capacity,
            used: 0,
            allocation_count: 0,
            snapshot_count: 0,
            created_at: Utc::now(),
            last_modified: Utc::now(),
        }
    }

    fn from_bytes(data: &[u8]) -> Result<Self> {
        let json_str = std::str::from_utf8(data)
            .context("Header bytes are not valid UTF-8")?;
        // Find the end of JSON (null terminator or end of data)
        let end = json_str.find('\0').unwrap_or(json_str.len());
        let header: MemoryHeader = serde_json::from_str(&json_str[..end])
            .context("Failed to deserialize memory header")?;
        if header.magic != Self::MAGIC {
            anyhow::bail!("Invalid memory file magic bytes");
        }
        Ok(header)
    }

    fn to_bytes(&self) -> Vec<u8> {
        let json = serde_json::to_string(self).expect("Header serialization should not fail");
        let mut bytes = json.into_bytes();
        bytes.resize(Self::SERIALIZED_SIZE, 0);
        bytes
    }
}

/// Persistent memory pool backed by memory-mapped file
pub struct MemoryPool {
    path: PathBuf,
    mmap: MmapMut,
    header: Mutex<MemoryHeader>,
    data_offset: usize,
}

impl MemoryPool {
    /// Create a new memory pool or open an existing one
    pub fn open<P: AsRef<Path>>(path: P, capacity: usize) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let file_exists = path.exists();

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)
            .context("Failed to open memory pool file")?;

        let total_size = MemoryHeader::SERIALIZED_SIZE + capacity;
        file.set_len(total_size as u64)
            .context("Failed to set memory pool file size")?;

        let mut mmap = unsafe { MmapOptions::new().map_mut(&file) }
            .context("Failed to create memory mapping")?;

        let header = if file_exists {
            // Check if file has actual content (not just created empty by tempfile)
            let mut has_content = false;
            for &byte in &mmap[..MemoryHeader::SERIALIZED_SIZE.min(mmap.len())] {
                if byte != 0 {
                    has_content = true;
                    break;
                }
            }

            if has_content {
                let existing = MemoryHeader::from_bytes(&mmap)?;
                if existing.capacity != capacity {
                    anyhow::bail!(
                        "Capacity mismatch: file has {} bytes, requested {} bytes",
                        existing.capacity,
                        capacity
                    );
                }
                info!(
                    path = %path.display(),
                    used = existing.used,
                    allocations = existing.allocation_count,
                    "Opened existing memory pool"
                );
                existing
            } else {
                // File exists but is empty — treat as new
                let h = MemoryHeader::new(capacity);
                let bytes = h.to_bytes();
                mmap[..bytes.len()].copy_from_slice(&bytes);
                mmap.flush().context("Failed to flush initial header")?;
                info!(
                    path = %path.display(),
                    capacity,
                    "Created new memory pool (empty file)"
                );
                h
            }
        } else {
            let h = MemoryHeader::new(capacity);
            let bytes = h.to_bytes();
            mmap[..bytes.len()].copy_from_slice(&bytes);
            mmap.flush().context("Failed to flush initial header")?;
            info!(
                path = %path.display(),
                capacity,
                "Created new memory pool"
            );
            h
        };

        Ok(Self {
            path,
            mmap,
            header: Mutex::new(header),
            data_offset: MemoryHeader::SERIALIZED_SIZE,
        })
    }

    /// Write raw bytes into the memory pool. Returns the relative offset.
    pub fn write(&mut self, data: &[u8]) -> Result<usize> {
        let mut header = self.header.lock();
        let available = header.capacity - header.used;
        if data.len() > available {
            anyhow::bail!(
                "Not enough memory: need {} bytes, have {} bytes free",
                data.len(),
                available
            );
        }

        let relative_offset = header.used;
        let abs_offset = self.data_offset + relative_offset;
        self.mmap[abs_offset..abs_offset + data.len()].copy_from_slice(data);
        header.used += data.len();
        header.allocation_count += 1;
        header.last_modified = Utc::now();

        // Update header in mmap
        let header_bytes = header.to_bytes();
        self.mmap[..header_bytes.len()].copy_from_slice(&header_bytes);
        self.mmap.flush().context("Failed to flush after write")?;

        debug!(offset = relative_offset, len = data.len(), "Memory write");
        Ok(relative_offset)
    }

    /// Read bytes from the memory pool at given offset
    pub fn read(&self, offset: usize, len: usize) -> Result<Vec<u8>> {
        let header = self.header.lock();
        let abs_offset = self.data_offset + offset;
        if abs_offset + len > self.data_offset + header.used {
            anyhow::bail!(
                "Read out of bounds: offset={}, len={}, used={}",
                offset,
                len,
                header.used
            );
        }
        Ok(self.mmap[abs_offset..abs_offset + len].to_vec())
    }

    /// Write a serializable value and return its offset
    pub fn store<T: Serialize>(&mut self, value: &T) -> Result<usize> {
        let data = serde_json::to_vec(value).context("Failed to serialize value")?;
        self.write(&data)
    }

    /// Persist all changes to disk
    pub fn persist(&self) -> Result<()> {
        self.mmap.flush().context("Failed to flush memory pool to disk")?;
        debug!("Memory pool persisted to disk");
        Ok(())
    }

    /// Get current memory statistics
    pub fn stats(&self) -> MemoryStats {
        let header = self.header.lock();
        let used = header.used;
        let capacity = header.capacity;
        MemoryStats {
            capacity_bytes: capacity,
            used_bytes: used,
            free_bytes: capacity - used,
            utilization: if capacity > 0 {
                used as f64 / capacity as f64
            } else {
                0.0
            },
            allocation_count: header.allocation_count,
            snapshot_count: header.snapshot_count,
        }
    }

    /// Get the path of the memory pool file
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reset the memory pool (clear all data)
    pub fn reset(&mut self) -> Result<()> {
        let mut header = self.header.lock();
        header.used = 0;
        header.allocation_count = 0;
        header.snapshot_count = 0;
        header.last_modified = Utc::now();

        let header_bytes = header.to_bytes();
        self.mmap[..header_bytes.len()].copy_from_slice(&header_bytes);

        // Zero out data region
        for byte in &mut self.mmap[self.data_offset..] {
            *byte = 0;
        }

        self.mmap.flush().context("Failed to flush after reset")?;
        info!("Memory pool reset");
        Ok(())
    }
}

impl Drop for MemoryPool {
    fn drop(&mut self) {
        let _ = self.persist();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn create_temp_pool(capacity: usize) -> (MemoryPool, NamedTempFile) {
        let tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        let pool = MemoryPool::open(&path, capacity).unwrap();
        (pool, tmp)
    }

    #[test]
    fn test_create_pool() {
        let (pool, _tmp) = create_temp_pool(4096);
        let stats = pool.stats();
        assert_eq!(stats.capacity_bytes, 4096);
        assert_eq!(stats.used_bytes, 0);
        assert_eq!(stats.utilization, 0.0);
    }

    #[test]
    fn test_write_and_read() {
        let (mut pool, _tmp) = create_temp_pool(4096);
        let data = b"hello, omega!";
        let offset = pool.write(data).unwrap();

        let read_data = pool.read(offset, data.len()).unwrap();
        assert_eq!(read_data, data);

        let stats = pool.stats();
        assert_eq!(stats.used_bytes, data.len());
        assert_eq!(stats.allocation_count, 1);
    }

    #[test]
    fn test_multiple_writes() {
        let (mut pool, _tmp) = create_temp_pool(4096);

        let offset1 = pool.write(b"first").unwrap();
        let offset2 = pool.write(b"second").unwrap();
        let offset3 = pool.write(b"third").unwrap();

        assert_eq!(pool.read(offset1, 5).unwrap(), b"first");
        assert_eq!(pool.read(offset2, 6).unwrap(), b"second");
        assert_eq!(pool.read(offset3, 5).unwrap(), b"third");

        let stats = pool.stats();
        assert_eq!(stats.allocation_count, 3);
    }

    #[test]
    fn test_store_and_load() {
        let (mut pool, _tmp) = create_temp_pool(4096);

        #[derive(Serialize, Deserialize, Debug, PartialEq)]
        struct TestEntry {
            name: String,
            value: i64,
        }

        let entry = TestEntry {
            name: "test".to_string(),
            value: 42,
        };

        let offset = pool.store(&entry).unwrap();
        let stats = pool.stats();
        // Read back all written data
        let raw = pool.read(offset, stats.used_bytes).unwrap();
        let loaded: TestEntry = serde_json::from_slice(&raw).unwrap();
        assert_eq!(loaded, entry);
    }

    #[test]
    fn test_out_of_bounds() {
        let (mut pool, _tmp) = create_temp_pool(16);
        let data = [0u8; 100];
        let result = pool.write(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_reset() {
        let (mut pool, _tmp) = create_temp_pool(4096);
        pool.write(b"some data").unwrap();
        assert!(pool.stats().used_bytes > 0);

        pool.reset().unwrap();
        let stats = pool.stats();
        assert_eq!(stats.used_bytes, 0);
        assert_eq!(stats.allocation_count, 0);
    }

    #[test]
    fn test_persist() {
        let (mut pool, _tmp) = create_temp_pool(4096);
        pool.write(b"persistent data").unwrap();
        pool.persist().unwrap();
    }

    #[test]
    fn test_read_out_of_bounds() {
        let (pool, _tmp) = create_temp_pool(4096);
        let result = pool.read(0, 1);
        assert!(result.is_err());
    }
}
