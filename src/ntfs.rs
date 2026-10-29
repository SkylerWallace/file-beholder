//! Read-only NTFS MFT index. Unsupported, inaccessible, or inconsistent volumes
//! return an error so the caller can use ordinary directory traversal.
use std::{
    collections::{HashMap, VecDeque},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FolderSize {
    pub bytes: u64,
    pub partial: bool,
}
#[derive(Debug, Default)]
struct Record {
    id: u64,
    base: Option<u64>,
    directory: bool,
    reparse: bool,
    bytes: Option<u64>,
    names: Vec<(u64, Vec<u16>)>,
}
fn invalid() -> String {
    "Unsupported or inconsistent NTFS metadata".into()
}
fn u16_at(b: &[u8], p: usize) -> Result<u16, String> {
    Ok(u16::from_le_bytes(
        b.get(p..p + 2).ok_or_else(invalid)?.try_into().unwrap(),
    ))
}
fn u32_at(b: &[u8], p: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        b.get(p..p + 4).ok_or_else(invalid)?.try_into().unwrap(),
    ))
}
fn u64_at(b: &[u8], p: usize) -> Result<u64, String> {
    Ok(u64::from_le_bytes(
        b.get(p..p + 8).ok_or_else(invalid)?.try_into().unwrap(),
    ))
}

/// Apply multi-sector update-sequence fixups before inspecting attributes.
fn fixup(b: &mut [u8], sector: usize) -> Result<(), String> {
    let offset = u16_at(b, 4)? as usize;
    let count = u16_at(b, 6)? as usize;
    if sector < 256 || b.len() % sector != 0 || count != b.len() / sector + 1 {
        return Err(invalid());
    }
    let signature = u16_at(b, offset)?;
    for i in 1..count {
        let end = i * sector - 2;
        if u16_at(b, end)? != signature {
            return Err("NTFS record changed during read".into());
        }
        let original = u16_at(b, offset + i * 2)?.to_le_bytes();
        b[end..end + 2].copy_from_slice(&original);
    }
    Ok(())
}
fn record(b: &mut [u8], number: u64, sector: usize) -> Result<Option<Record>, String> {
    if b.iter().all(|&v| v == 0) {
        return Ok(None);
    }
    if b.get(..4) != Some(b"FILE") {
        return Err(invalid());
    }
    let flags = u16_at(b, 22)?;
    if flags & 1 == 0 {
        return Ok(None);
    }
    fixup(b, sector)?;
    let end = u32_at(b, 24)? as usize;
    if end > b.len() || end < 48 {
        return Err(invalid());
    }
    let b = &b[..end];
    let base = u64_at(b, 32)?;
    let mut out = Record {
        id: number | ((u16_at(b, 16)? as u64) << 48),
        base: (base != 0).then_some(base),
        directory: flags & 2 != 0,
        ..Default::default()
    };
    let mut pos = u16_at(b, 20)? as usize;
    if pos < 48 || pos % 8 != 0 {
        return Err(invalid());
    }
    while pos + 4 <= end {
        let kind = u32_at(b, pos)?;
        if kind == u32::MAX {
            return Ok(Some(out));
        }
        let len = u32_at(b, pos + 4)? as usize;
        if len < 24 || len % 8 != 0 || pos.checked_add(len).is_none_or(|n| n > end) {
            return Err(invalid());
        }
        let a = &b[pos..pos + len];
        let nonresident = a[8] != 0;
        if !nonresident {
            let n = u32_at(a, 16)? as usize;
            let start = u16_at(a, 20)? as usize;
            if start < 24 {
                return Err(invalid());
            }
            let value = a
                .get(start..start.checked_add(n).ok_or_else(invalid)?)
                .ok_or_else(invalid)?;
            if kind == 0x10 {
                out.reparse |= u32_at(value, 32)? & 0x400 != 0;
            }
            if kind == 0x30 {
                if value.len() < 66 {
                    return Err(invalid());
                }
                // Ignore DOS 8.3 aliases; count each real hard-link name once.
                if value[65] != 2 {
                    let count = value[64] as usize;
                    let name = value
                        .get(66..66 + count * 2)
                        .ok_or_else(invalid)?
                        .chunks_exact(2)
                        .map(|v| u16::from_le_bytes([v[0], v[1]]))
                        .collect();
                    let link = (u64_at(value, 0)?, name);
                    if !out.names.contains(&link) {
                        out.names.push(link);
                    }
                }
            }
            if kind == 0x80 && a[9] == 0 {
                out.bytes = Some(n as u64);
            }
        } else if kind == 0x80 && a[9] == 0 && u64_at(a, 16)? == 0 {
            out.bytes = Some(u64_at(a, 48)?);
        }
        pos += len;
    }
    Err(invalid())
}
/// Decode NTFS mapping pairs. Relative LCN deltas are signed; sparse MFT
/// extents are rejected rather than being mistaken for physical cluster zero.
fn mapping_pairs(data: &[u8], start_vcn: u64) -> Result<Vec<(u64, u64, u64)>, String> {
    let mut pos = 0;
    let mut vcn = start_vcn;
    let mut lcn = 0i64;
    let mut out = Vec::new();
    loop {
        let header = *data.get(pos).ok_or_else(invalid)?;
        pos += 1;
        if header == 0 {
            return Ok(out);
        }
        let count = (header & 15) as usize;
        let offset = (header >> 4) as usize;
        if count == 0 || count > 8 || offset == 0 || offset > 8 {
            return Err(invalid());
        }
        let len = data.get(pos..pos + count).ok_or_else(invalid)?;
        pos += count;
        let delta = data.get(pos..pos + offset).ok_or_else(invalid)?;
        pos += offset;
        let mut length = [0; 8];
        length[..count].copy_from_slice(len);
        let mut relative = if delta[offset - 1] & 0x80 != 0 {
            [255; 8]
        } else {
            [0; 8]
        };
        relative[..offset].copy_from_slice(delta);
        let clusters = u64::from_le_bytes(length);
        lcn = lcn
            .checked_add(i64::from_le_bytes(relative))
            .ok_or_else(invalid)?;
        let next = vcn.checked_add(clusters).ok_or_else(invalid)?;
        if clusters == 0 || lcn < 0 {
            return Err(invalid());
        }
        out.push((vcn, next, lcn as u64));
        vcn = next;
    }
}
fn aggregate(
    mut records: HashMap<u64, Record>,
    cancel: &AtomicBool,
) -> Result<HashMap<u64, FolderSize>, String> {
    // Merge extension records, including nonresident data and attribute lists.
    let extensions: Vec<_> = records
        .iter()
        .filter_map(|(&id, r)| r.base.map(|_| id))
        .collect();
    for id in extensions {
        let extension = records.remove(&id).unwrap();
        let base = records
            .get_mut(&extension.base.unwrap())
            .ok_or_else(invalid)?;
        if extension.bytes.is_some() {
            base.bytes = extension.bytes;
        }
        base.reparse |= extension.reparse;
        for link in extension.names {
            if !base.names.contains(&link) {
                base.names.push(link);
            }
        }
    }
    let mut totals: HashMap<_, _> = records
        .values()
        .filter(|r| r.directory && !r.reparse)
        .map(|r| (r.id, FolderSize::default()))
        .collect();
    let mut children = HashMap::<u64, usize>::new();
    let mut parents = HashMap::<u64, Vec<u64>>::new();
    let mut incomplete = false;
    for r in records.values() {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        if r.id & 0x0000_ffff_ffff_ffff == 5 {
            continue;
        } // Root self-link.
        for &(parent, _) in &r.names {
            let Some(total) = totals.get_mut(&parent) else {
                incomplete = true;
                continue;
            };
            if r.reparse {
                total.partial = true;
                continue;
            }
            if r.directory {
                *children.entry(parent).or_default() += 1;
                parents.entry(r.id).or_default().push(parent);
            } else {
                total.bytes = total.bytes.saturating_add(r.bytes.unwrap_or(0));
                total.partial |= r.bytes.is_none();
            }
        }
    }
    let mut queue: VecDeque<_> = totals
        .keys()
        .copied()
        .filter(|id| children.get(id).copied().unwrap_or(0) == 0)
        .collect();
    let mut processed = 0;
    while let Some(id) = queue.pop_front() {
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        processed += 1;
        let size = totals[&id];
        for &parent in parents.get(&id).into_iter().flatten() {
            let total = totals.get_mut(&parent).ok_or_else(invalid)?;
            total.bytes = total.bytes.saturating_add(size.bytes);
            total.partial |= size.partial;
            let remaining = children.get_mut(&parent).ok_or_else(invalid)?;
            *remaining -= 1;
            if *remaining == 0 {
                queue.push_back(parent);
            }
        }
    }
    if processed != totals.len() {
        return Err("NTFS directory cycle detected".into());
    }
    if incomplete {
        for size in totals.values_mut() {
            size.partial = true;
        }
    }
    Ok(totals)
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::{
        fs::{File, OpenOptions},
        io::{Read, Seek, SeekFrom},
        os::windows::{ffi::OsStrExt, fs::OpenOptionsExt, io::AsRawHandle},
        sync::{Arc, Mutex, OnceLock},
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, GetDriveTypeW, GetFileInformationByHandle,
            GetVolumeInformationW, GetVolumeNameForVolumeMountPointW, GetVolumePathNameW,
        },
        System::IO::DeviceIoControl,
    };
    fn wide(p: &std::ffi::OsStr) -> Vec<u16> {
        p.encode_wide().chain(Some(0)).collect()
    }
    fn ioctl(file: &File, code: u32, input: &[u8], output: &mut [u8]) -> Result<usize, String> {
        let mut returned = 0;
        let ok = unsafe {
            DeviceIoControl(
                file.as_raw_handle() as HANDLE,
                code,
                input.as_ptr().cast(),
                input.len() as u32,
                output.as_mut_ptr().cast(),
                output.len() as u32,
                &mut returned,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            Err(std::io::Error::last_os_error().to_string())
        } else {
            Ok(returned as usize)
        }
    }
    fn volume(path: &Path) -> Result<(String, String), String> {
        let absolute = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
        let mut root = vec![0; 32768];
        if unsafe {
            GetVolumePathNameW(
                wide(absolute.as_os_str()).as_ptr(),
                root.as_mut_ptr(),
                root.len() as u32,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        if unsafe { GetDriveTypeW(root.as_ptr()) } == 4 {
            return Err("Network path".into());
        }
        let mut filesystem = [0u16; 32];
        if unsafe {
            GetVolumeInformationW(
                root.as_ptr(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                filesystem.as_mut_ptr(),
                32,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        if String::from_utf16_lossy(
            &filesystem[..filesystem.iter().position(|&c| c == 0).unwrap_or(32)],
        ) != "NTFS"
        {
            return Err("Non-NTFS volume".into());
        }
        let root = String::from_utf16_lossy(&root[..root.iter().position(|&c| c == 0).unwrap()]);
        let mut guid = [0u16; 128];
        if unsafe {
            GetVolumeNameForVolumeMountPointW(
                wide(std::ffi::OsStr::new(&root)).as_ptr(),
                guid.as_mut_ptr(),
                128,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let guid = String::from_utf16_lossy(&guid[..guid.iter().position(|&c| c == 0).unwrap()]);
        Ok((guid.trim_end_matches('\\').to_string(), root))
    }
    pub struct Index {
        totals: HashMap<u64, FolderSize>,
        pub records: usize,
    }
    impl Index {
        pub fn record_count(&self) -> usize {
            self.records
        }
        pub fn folder(&self, path: &Path) -> Option<FolderSize> {
            let file = OpenOptions::new()
                .access_mode(0)
                .share_mode(7)
                .custom_flags(0x0200_0000)
                .open(path)
                .ok()?;
            let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
            if unsafe { GetFileInformationByHandle(file.as_raw_handle() as HANDLE, &mut info) } == 0
            {
                return None;
            }
            let id = (info.nFileIndexHigh as u64) << 32 | info.nFileIndexLow as u64;
            self.totals.get(&id).copied()
        }
    }
    type Cached = Option<(Instant, Result<Arc<Index>, String>)>;
    type Slot = Arc<Mutex<Cached>>;
    static CACHE: OnceLock<Mutex<HashMap<String, Slot>>> = OnceLock::new();
    pub fn get(path: &Path, cancel: &AtomicBool, refresh: bool) -> Result<Arc<Index>, String> {
        let (guid, root) = volume(path)?;
        let slot = {
            let mut cache = CACHE
                .get_or_init(Default::default)
                .lock()
                .map_err(|_| invalid())?;
            cache.entry(guid.clone()).or_default().clone()
        };
        // Wait off the UI thread. Cancellation also interrupts cache contention.
        let mut guard = loop {
            if cancel.load(Ordering::Relaxed) {
                return Err("Cancelled".into());
            }
            if let Ok(guard) = slot.try_lock() {
                break guard;
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        if let Some((at, result)) = &*guard {
            let ttl = if result.is_ok() { 30 } else { 5 };
            if !refresh && at.elapsed() < Duration::from_secs(ttl) {
                return result.clone();
            }
        }
        let result = build(&guid, &root, cancel).map(Arc::new);
        if !cancel.load(Ordering::Relaxed) {
            *guard = Some((Instant::now(), result.clone()));
        }
        result
    }

    #[allow(clippy::too_many_arguments)]
    fn mft_extents(
        root: &str,
        volume: &mut File,
        geometry: &[u8],
        sector: usize,
        cluster: u64,
        record_size: usize,
        valid: u64,
        cancel: &AtomicBool,
    ) -> Result<Vec<(u64, u64, u64)>, String> {
        let retrieval = (|| {
            let mft = OpenOptions::new()
                .access_mode(0x80)
                .share_mode(7)
                .custom_flags(0x0200_0000)
                .open(format!("{root}$MFT"))
                .map_err(|e| e.to_string())?;
            let mut vcn = 0u64;
            let mut extents = Vec::new();
            loop {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Cancelled".into());
                }
                let mut buffer = vec![0u8; 64 * 1024];
                let mut returned = 0;
                let ok = unsafe {
                    DeviceIoControl(
                        mft.as_raw_handle() as HANDLE,
                        0x0009_0073,
                        (&vcn as *const u64).cast(),
                        8,
                        buffer.as_mut_ptr().cast(),
                        buffer.len() as u32,
                        &mut returned,
                        std::ptr::null_mut(),
                    )
                };
                if ok == 0 && std::io::Error::last_os_error().raw_os_error() != Some(234) {
                    return Err(std::io::Error::last_os_error().to_string());
                }
                let buffer = buffer.get(..returned as usize).ok_or_else(invalid)?;
                let count = u32_at(buffer, 0)? as usize;
                let mut start = u64_at(buffer, 8)?;
                if count == 0 || start != vcn || buffer.len() < 16 + count * 16 {
                    return Err(invalid());
                }
                for i in 0..count {
                    let next = u64_at(buffer, 16 + i * 16)?;
                    let lcn = u64_at(buffer, 24 + i * 16)?;
                    if next <= start || lcn > i64::MAX as u64 {
                        return Err(invalid());
                    }
                    extents.push((start, next, lcn));
                    start = next;
                }
                if start <= vcn {
                    return Err(invalid());
                }
                vcn = start;
                if ok != 0 {
                    return Ok(extents);
                }
            }
        })();
        if retrieval.is_ok() || cancel.load(Ordering::Relaxed) {
            return retrieval;
        }
        // Some Windows versions deny opening $MFT even with raw-volume access.
        // Bootstrap its unnamed DATA mapping directly from record zero. If an
        // attribute-list extension is needed, retain the traversal fallback.
        let mut first = vec![0u8; record_size];
        let offset = u64_at(geometry, 64)?
            .checked_mul(cluster)
            .ok_or_else(invalid)?;
        volume
            .seek(SeekFrom::Start(offset))
            .map_err(|e| e.to_string())?;
        volume.read_exact(&mut first).map_err(|e| e.to_string())?;
        if first.get(..4) != Some(b"FILE") {
            return Err(invalid());
        }
        fixup(&mut first, sector)?;
        let mut pos = u16_at(&first, 20)? as usize;
        let end = u32_at(&first, 24)? as usize;
        if end > first.len() {
            return Err(invalid());
        }
        while pos + 24 <= end {
            if u32_at(&first, pos)? == u32::MAX {
                break;
            }
            let len = u32_at(&first, pos + 4)? as usize;
            if len < 24 || pos + len > end {
                return Err(invalid());
            }
            let a = &first[pos..pos + len];
            if u32_at(a, 0)? == 0x80 && a[8] == 1 && a[9] == 0 && u64_at(a, 16)? == 0 {
                let start = u16_at(a, 32)? as usize;
                let runs = mapping_pairs(a.get(start..).ok_or_else(invalid)?, 0)?;
                if runs
                    .last()
                    .and_then(|r| r.1.checked_mul(cluster))
                    .is_some_and(|n| n >= valid)
                {
                    return Ok(runs);
                }
                return Err("MFT extent extensions unavailable; using traversal".into());
            }
            pos += len;
        }
        Err(invalid())
    }
    fn build(guid: &str, root: &str, cancel: &AtomicBool) -> Result<Index, String> {
        // No elevation prompt, write access, volume locks, or disk changes.
        let mut volume = OpenOptions::new()
            .read(true)
            .share_mode(7)
            .open(guid)
            .map_err(|e| format!("MFT access unavailable: {e}"))?;
        let mut geometry = [0u8; 96];
        ioctl(&volume, 0x0009_0064, &[], &mut geometry)?; // FSCTL_GET_NTFS_VOLUME_DATA
        let sector = u32_at(&geometry, 40)? as usize;
        let cluster = u32_at(&geometry, 44)? as u64;
        let record_size = u32_at(&geometry, 48)? as usize;
        let valid = u64_at(&geometry, 56)?;
        if !(512..=65536).contains(&sector)
            || !sector.is_power_of_two()
            || !(sector as u64..=2 * 1024 * 1024).contains(&cluster)
            || !cluster.is_power_of_two()
            || !(512..=65536).contains(&record_size)
            || record_size % sector != 0
            || valid == 0
            || valid / record_size as u64 > 16_000_000
        {
            return Err(invalid());
        }
        // Retrieval pointers handle fragmented MFT streams, including extents
        // described by extension records. FILE_READ_ATTRIBUTES is intentional.
        let extents = mft_extents(
            root,
            &mut volume,
            &geometry,
            sector,
            cluster,
            record_size,
            valid,
            cancel,
        )?;
        let mut stream_pos = 0u64;
        let mut records = HashMap::new();
        let mut carry = Vec::new();
        let mut batch = vec![0u8; 4 * 1024 * 1024];
        for (start, next, lcn) in extents {
            let mut physical = lcn.checked_mul(cluster).ok_or_else(invalid)?;
            let mut remaining = (next - start)
                .checked_mul(cluster)
                .ok_or_else(invalid)?
                .min(valid - stream_pos);
            while remaining > 0 {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Cancelled".into());
                }
                let n = remaining.min(batch.len() as u64) as usize;
                // Volume I/O must be sector aligned, even at the final extent.
                let read_n = n.div_ceil(sector) * sector;
                volume
                    .seek(SeekFrom::Start(physical))
                    .map_err(|e| e.to_string())?;
                volume
                    .read_exact(&mut batch[..read_n])
                    .map_err(|e| e.to_string())?;
                carry.extend_from_slice(&batch[..n]);
                let complete = carry.len() / record_size * record_size;
                let first_number = (stream_pos - (carry.len() - n) as u64) / record_size as u64;
                for (i, data) in carry[..complete].chunks_exact_mut(record_size).enumerate() {
                    if let Some(r) = record(data, first_number + i as u64, sector)? {
                        records.insert(r.id, r);
                    }
                }
                carry.drain(..complete);
                stream_pos += n as u64;
                physical += n as u64;
                remaining -= n as u64;
            }
            if stream_pos == valid {
                break;
            }
        }
        if stream_pos != valid {
            return Err(invalid());
        }
        if !carry.is_empty() {
            return Err(invalid());
        }
        let count = records.len();
        let totals = aggregate(records, cancel)?;
        Ok(Index {
            totals,
            records: count,
        })
    }
}
#[cfg(windows)]
pub use windows::get;
#[cfg(not(windows))]
pub fn get(_path: &Path, _cancel: &AtomicBool, _refresh: bool) -> Result<NoIndex, String> {
    Err("NTFS indexing requires Windows".into())
}
#[cfg(not(windows))]
pub struct NoIndex;
#[cfg(not(windows))]
impl NoIndex {
    pub fn record_count(&self) -> usize {
        0
    }
    pub fn folder(&self, _: &Path) -> Option<FolderSize> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn resident(kind: u32, value: &[u8]) -> Vec<u8> {
        let len = (24 + value.len()).div_ceil(8) * 8;
        let mut a = vec![0; len];
        a[..4].copy_from_slice(&kind.to_le_bytes());
        a[4..8].copy_from_slice(&(len as u32).to_le_bytes());
        a[16..20].copy_from_slice(&(value.len() as u32).to_le_bytes());
        a[20..22].copy_from_slice(&24u16.to_le_bytes());
        a[24..24 + value.len()].copy_from_slice(value);
        a
    }
    fn fixture(attrs: &[Vec<u8>], base: u64) -> Vec<u8> {
        let mut b = vec![0; 1024];
        b[..4].copy_from_slice(b"FILE");
        b[4..6].copy_from_slice(&48u16.to_le_bytes());
        b[6..8].copy_from_slice(&3u16.to_le_bytes());
        b[16..18].copy_from_slice(&7u16.to_le_bytes());
        b[20..22].copy_from_slice(&56u16.to_le_bytes());
        b[22..24].copy_from_slice(&1u16.to_le_bytes());
        b[32..40].copy_from_slice(&base.to_le_bytes());
        let mut at = 56;
        for a in attrs {
            b[at..at + a.len()].copy_from_slice(a);
            at += a.len();
        }
        b[at..at + 4].fill(255);
        b[24..28].copy_from_slice(&((at + 8) as u32).to_le_bytes());
        b[48..50].copy_from_slice(&[0xaa, 0xbb]);
        for i in 1..=2 {
            let end = i * 512 - 2;
            let original = [b[end], b[end + 1]];
            b[48 + i * 2..50 + i * 2].copy_from_slice(&original);
            b[end..end + 2].copy_from_slice(&[0xaa, 0xbb]);
        }
        b
    }
    fn filename(parent: u64, namespace: u8, name: &str) -> Vec<u8> {
        let name: Vec<_> = name.encode_utf16().collect();
        let mut v = vec![0; 66 + name.len() * 2];
        v[..8].copy_from_slice(&parent.to_le_bytes());
        v[64] = name.len() as u8;
        v[65] = namespace;
        for (i, c) in name.into_iter().enumerate() {
            v[66 + i * 2..68 + i * 2].copy_from_slice(&c.to_le_bytes());
        }
        resident(0x30, &v)
    }
    #[test]
    fn parses_resident_and_nonresident_sizes_without_dos_double_counting() {
        let attrs = vec![
            filename(5, 1, "Résumé.txt"),
            filename(5, 2, "RESUME~1.TXT"),
            resident(0x80, &[0; 37]),
        ];
        let r = record(&mut fixture(&attrs, 0), 42, 512).unwrap().unwrap();
        assert_eq!(r.bytes, Some(37));
        assert_eq!(r.names.len(), 1);
        assert_eq!(r.id, (7 << 48) | 42);
        let mut data = vec![0; 64];
        data[..4].copy_from_slice(&0x80u32.to_le_bytes());
        data[4..8].copy_from_slice(&64u32.to_le_bytes());
        data[8] = 1;
        data[48..56].copy_from_slice(&987654321u64.to_le_bytes());
        let r = record(&mut fixture(&[data], 123), 43, 512)
            .unwrap()
            .unwrap();
        assert_eq!(r.bytes, Some(987654321));
        assert_eq!(r.base, Some(123));
    }
    #[test]
    fn parses_signed_fragment_offsets_and_rejects_sparse_extents() {
        assert_eq!(
            mapping_pairs(&[0x11, 3, 100, 0x11, 2, 0xf6, 0], 0).unwrap(),
            vec![(0, 3, 100), (3, 5, 90)]
        );
        assert!(mapping_pairs(&[0x01, 3, 0], 0).is_err());
        assert!(mapping_pairs(&[0x11, 3, 0x80, 0], 0).is_err());
    }
    #[test]
    fn extension_data_is_merged_into_its_base() {
        let mut file = rec(10, false, &[5], 0);
        file.bytes = None;
        let mut ext = rec(11, false, &[], 4096);
        ext.base = Some(10);
        let records = [rec(5, true, &[5], 0), file, ext]
            .into_iter()
            .map(|r| (r.id, r))
            .collect();
        assert_eq!(
            aggregate(records, &AtomicBool::new(false)).unwrap()[&5].bytes,
            4096
        );
    }
    fn rec(id: u64, dir: bool, parents: &[u64], bytes: u64) -> Record {
        Record {
            id,
            directory: dir,
            bytes: Some(bytes),
            names: parents
                .iter()
                .enumerate()
                .map(|(i, &p)| (p, vec![65 + i as u16]))
                .collect(),
            ..Default::default()
        }
    }
    #[test]
    fn totals_include_hard_links_and_skip_reparse_subtrees() {
        let root = rec(5, true, &[5], 0);
        let child = rec(9, true, &[5], 0);
        let file = rec(10, false, &[9, 5], 37);
        let mut link = rec(11, true, &[9], 0);
        link.reparse = true;
        let records = [root, child, file, link]
            .into_iter()
            .map(|r| (r.id, r))
            .collect();
        let totals = aggregate(records, &AtomicBool::new(false)).unwrap();
        assert_eq!(
            totals[&9],
            FolderSize {
                bytes: 37,
                partial: true
            }
        );
        assert_eq!(
            totals[&5],
            FolderSize {
                bytes: 74,
                partial: true
            }
        );
        assert!(!totals.contains_key(&11));
    }
    #[test]
    fn cycles_and_missing_extensions_fall_back() {
        let records = [rec(7, true, &[8], 0), rec(8, true, &[7], 0)]
            .into_iter()
            .map(|r| (r.id, r))
            .collect();
        assert!(aggregate(records, &AtomicBool::new(false)).is_err());
        let mut r = rec(9, false, &[], 0);
        r.base = Some(10);
        assert!(aggregate([(9, r)].into(), &AtomicBool::new(false)).is_err());
    }
    #[test]
    fn update_sequence_rejects_torn_records() {
        let mut data = vec![0; 1024];
        data[4..6].copy_from_slice(&48u16.to_le_bytes());
        data[6..8].copy_from_slice(&3u16.to_le_bytes());
        data[48..54].copy_from_slice(&[0xaa, 0xbb, 1, 2, 3, 4]);
        data[510..512].copy_from_slice(&[0xaa, 0xbb]);
        data[1022..1024].copy_from_slice(&[0xaa, 0xbb]);
        let mut torn = data.clone();
        torn[510] = 0;
        assert!(fixup(&mut torn, 512).is_err());
        fixup(&mut data, 512).unwrap();
        assert_eq!(&data[510..512], &[1, 2]);
        assert_eq!(&data[1022..1024], &[3, 4]);
    }
    #[test]
    fn malformed_records_do_not_panic() {
        for len in 0..1100 {
            let mut b = vec![0xff; len];
            let _ = record(&mut b, 42, 512);
        }
    }
    #[cfg(windows)]
    #[test]
    #[ignore = "Read-only live volume smoke test; prints fallback if not elevated"]
    fn live_volume() {
        let path = std::env::current_dir().unwrap();
        let at = std::time::Instant::now();
        match get(&path, &AtomicBool::new(false), true) {
            Ok(index) => {
                println!(
                    "MFT indexed {} records in {:?}; folder {:?}",
                    index.records,
                    at.elapsed(),
                    index.folder(&path)
                );
                assert!(index.folder(&path).is_some());
            }
            Err(error) => println!("Recursive fallback required: {error}"),
        }
    }
}
