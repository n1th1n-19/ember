// SPDX-License-Identifier: GPL-3.0-or-later
//! Long history written by `emberd`, read by the app for the 1-hour charts.
//!
//! File: `$XDG_STATE_HOME/ember/history.bin` (default `~/.local/state/ember/`).
//! Format: a 4-byte magic + version, then fixed 48-byte little-endian records
//! (u64 unix seconds + 10 × f32, NaN = no reading). Append-only: one record per
//! sample (~2 MB/day of writes); rewritten to the newest `KEEP` records once it
//! holds twice that.
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::Snapshot;

const MAGIC: &[u8; 4] = b"EMH1";
pub const FIELDS: usize = 10;
pub const RECORD: usize = 8 + 4 * FIELDS;
/// One hour at one sample every 2 s.
pub const KEEP: usize = 1800;
pub const INTERVAL_S: u64 = 2;

/// Column order inside a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Cpu,
    Mem,
    Swap,
    Rx,
    Tx,
    DiskR,
    DiskW,
    CpuTemp,
    Battery,
    GpuClock,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Record {
    pub t: u64,
    pub v: [f32; FIELDS],
}

impl Record {
    pub fn get(&self, f: Field) -> Option<f32> {
        let x = self.v[f as usize];
        x.is_finite().then_some(x)
    }

    pub fn from_snapshot(s: &Snapshot, t: u64) -> Self {
        let pct = |used: u64, total: u64| (total > 0).then(|| used as f32 / total as f32 * 100.0);
        let sum = |f: fn(&crate::sample::DiskStat) -> Option<f64>| s.disks.iter().filter_map(f).reduce(|a, b| a + b).map(|v| v as f32);
        let vals = [
            s.cpu_percent,
            s.mem_percent(),
            pct(s.swap_used, s.swap_total),
            s.net_rx.map(|v| v as f32),
            s.net_tx.map(|v| v as f32),
            sum(|d| d.read_bps),
            sum(|d| d.write_bps),
            s.cpu_temp(),
            s.batteries.first().and_then(|b| b.percent),
            s.gpus.iter().find_map(|g| g.cur_mhz).map(|v| v as f32),
        ];
        Self { t, v: vals.map(|v| v.unwrap_or(f32::NAN)) }
    }

    fn encode(&self) -> [u8; RECORD] {
        let mut b = [0u8; RECORD];
        b[..8].copy_from_slice(&self.t.to_le_bytes());
        for (i, v) in self.v.iter().enumerate() {
            b[8 + i * 4..12 + i * 4].copy_from_slice(&v.to_le_bytes());
        }
        b
    }

    fn decode(b: &[u8]) -> Self {
        let t = u64::from_le_bytes(b[..8].try_into().unwrap());
        let v = std::array::from_fn(|i| f32::from_le_bytes(b[8 + i * 4..12 + i * 4].try_into().unwrap()));
        Self { t, v }
    }
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

pub fn default_path() -> PathBuf {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    state.join("ember/history.bin")
}

pub struct Writer {
    path: PathBuf,
    file: File,
    count: usize,
}

impl Writer {
    /// Open (or create) the history file; a file with a wrong header is replaced.
    pub fn open(path: PathBuf) -> io::Result<Self> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let valid = fs::read(&path).ok().is_some_and(|b| b.starts_with(MAGIC));
        if !valid {
            fs::write(&path, MAGIC)?;
        }
        let len = fs::metadata(&path)?.len() as usize;
        let file = OpenOptions::new().append(true).open(&path)?;
        Ok(Self { path, file, count: len.saturating_sub(MAGIC.len()) / RECORD })
    }

    pub fn append(&mut self, r: &Record) -> io::Result<()> {
        self.file.write_all(&r.encode())?;
        self.count += 1;
        if self.count >= KEEP * 2 {
            self.compact()?;
        }
        Ok(())
    }

    /// Rewrite to the newest KEEP records (tmp file + rename, so readers never see half a file).
    fn compact(&mut self) -> io::Result<()> {
        let recs = read_last(&self.path, KEEP)?;
        let tmp = self.path.with_extension("tmp");
        let mut out = Vec::with_capacity(MAGIC.len() + recs.len() * RECORD);
        out.extend_from_slice(MAGIC);
        recs.iter().for_each(|r| out.extend_from_slice(&r.encode()));
        fs::write(&tmp, out)?;
        fs::rename(&tmp, &self.path)?;
        self.file = OpenOptions::new().append(true).open(&self.path)?;
        self.count = recs.len();
        Ok(())
    }
}

/// Newest `n` records, oldest first. Ignores a trailing partial record.
pub fn read_last(path: &std::path::Path, n: usize) -> io::Result<Vec<Record>> {
    let mut f = File::open(path)?;
    let mut head = [0u8; 4];
    f.read_exact(&mut head)?;
    if &head != MAGIC {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "not an ember history file"));
    }
    let total = (f.metadata()?.len() as usize).saturating_sub(MAGIC.len()) / RECORD;
    let skip = total.saturating_sub(n);
    f.seek(SeekFrom::Start((MAGIC.len() + skip * RECORD) as u64))?;
    let mut buf = vec![0u8; (total - skip) * RECORD];
    f.read_exact(&mut buf)?;
    Ok(buf.chunks_exact(RECORD).map(Record::decode).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_read_and_compact() {
        let path = std::env::temp_dir().join(format!("ember-hist-{}/history.bin", std::process::id()));
        let _ = fs::remove_dir_all(path.parent().unwrap());
        let mut w = Writer::open(path.clone()).unwrap();
        let mut v = [f32::NAN; FIELDS];
        for t in 0..(KEEP * 2 + 5) as u64 {
            v[0] = t as f32;
            w.append(&Record { t, v }).unwrap();
        }
        let last = read_last(&path, 3).unwrap();
        assert_eq!(last.iter().map(|r| r.t).collect::<Vec<_>>(), [3604, 3603, 3602].iter().rev().copied().collect::<Vec<_>>());
        assert_eq!(last[2].get(Field::Cpu), Some(3604.0));
        assert_eq!(last[2].get(Field::Mem), None);
        // compacted once at 3600 → KEEP + 5 records left
        assert_eq!(read_last(&path, usize::MAX).unwrap().len(), KEEP + 5);
        // a partial trailing write is ignored
        OpenOptions::new().append(true).open(&path).unwrap().write_all(&[1, 2, 3]).unwrap();
        assert_eq!(read_last(&path, 2).unwrap().len(), 2);
        // reopening keeps the count
        assert_eq!(Writer::open(path.clone()).unwrap().count, KEEP + 5);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
