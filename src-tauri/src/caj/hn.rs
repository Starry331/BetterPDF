//! Seek-based HN extraction: the input and encoded images are never read wholesale.
use super::*;
use std::io::{Seek, SeekFrom};

const MAX_FILE: u64 = 2 * 1024 * 1024 * 1024;
const MAX_IMAGE: u64 = 64 * 1024 * 1024;
struct Source {
    file: fs::File,
    len: u64,
}
impl Source {
    fn open(path: &Path) -> Result<Self, String> {
        let file = fs::File::open(path).map_err(|e| e.to_string())?;
        let len = file.metadata().map_err(|e| e.to_string())?.len();
        if len > MAX_FILE {
            return Err("HN exceeds the current 2 GiB file limit".into());
        }
        Ok(Self { file, len })
    }
    fn range(&self, at: u64, len: u64) -> Result<(), String> {
        if at.checked_add(len).is_none_or(|end| end > self.len) {
            return Err("Truncated HN image or page table".into());
        }
        Ok(())
    }
    fn read<const N: usize>(&mut self, at: u64) -> Result<[u8; N], String> {
        self.range(at, N as u64)?;
        self.file
            .seek(SeekFrom::Start(at))
            .map_err(|e| e.to_string())?;
        let mut bytes = [0; N];
        self.file
            .read_exact(&mut bytes)
            .map_err(|e| e.to_string())?;
        Ok(bytes)
    }
    fn number(&mut self, at: u64) -> Result<u64, String> {
        Ok(u32::from_le_bytes(self.read(at)?) as u64)
    }
    fn pages(&mut self, cancel: &Cancel) -> Result<Vec<(u64, u64, bool)>, String> {
        cancelled(cancel)?;
        let signature = self.read::<6>(0)?;
        let (count, table) = if signature[0] == 0xc8 {
            (self.number(8)?, 0x50)
        } else if signature.starts_with(b"HN") && signature[4..6] == [0xc8, 0] {
            (self.number(0x90)?, 0xd8)
        } else if signature.starts_with(b"HN") {
            (self.number(0x90)?, 0x15c + self.number(0x158)? * 0x134)
        } else {
            return Err("Unsupported HN signature".into());
        };
        if count == 0 || count > 20_000 {
            return Err("Invalid HN page count".into());
        }
        let table_end = table + count * 20;
        self.range(table, count * 20)?;
        let mut pages = Vec::with_capacity(count as usize);
        let mut ranges = Vec::with_capacity(count as usize);
        for i in 0..count {
            cancelled(cancel)?;
            let at = table + i * 20;
            let data = self.number(at)?;
            let text = self.number(at + 4)?;
            let fragments = u16::from_le_bytes(self.read(at + 8)?);
            if fragments != 1 {
                return Err("This HN variant contains text or multiple image fragments per page and is not yet supported".into());
            }
            let header = data + text;
            if data < table_end {
                return Err("HN page data overlaps its page table".into());
            }
            let kind = self.number(header)?;
            let image = self.number(header + 4)?;
            let length = self.number(header + 8)?;
            if !matches!(kind, 1 | 2) || image != header + 12 {
                return Err(
                    "This HN image encoding is not supported; no partial output was written".into(),
                );
            }
            self.range(image, length)?;
            if length < 4 || length > MAX_IMAGE {
                return Err("HN JPEG must be between 4 bytes and 64 MiB".into());
            }
            if self.read::<2>(image)? != [0xff, 0xd8] {
                return Err("Invalid HN JPEG image".into());
            }
            ranges.push((data, image + length));
            pages.push((image, length, kind == 2));
        }
        ranges.sort_unstable();
        if ranges.windows(2).any(|w| w[0].1 > w[1].0) {
            return Err("Overlapping HN page data; output rejected".into());
        }
        Ok(pages)
    }
    fn extract(&mut self, at: u64, len: u64, path: &Path, cancel: &Cancel) -> Result<(), String> {
        self.range(at, len)?;
        self.file
            .seek(SeekFrom::Start(at))
            .map_err(|e| e.to_string())?;
        let mut output = fs::File::create(path).map_err(|e| e.to_string())?;
        let mut remaining = len;
        let mut buffer = [0u8; 65536];
        while remaining > 0 {
            cancelled(cancel)?;
            let n = remaining.min(buffer.len() as u64) as usize;
            self.file
                .read_exact(&mut buffer[..n])
                .map_err(|e| e.to_string())?;
            output.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
            remaining -= n as u64;
        }
        Ok(())
    }
}
pub(super) fn convert(
    engine: &Engine,
    input: &Path,
    output: &Path,
    temp: &Path,
    cancel: &Cancel,
) -> Result<(), String> {
    let mut source = Source::open(input)?;
    let pages = source.pages(cancel)?;
    let mut images = Vec::with_capacity(pages.len());
    for (index, &(offset, length, _)) in pages.iter().enumerate() {
        cancelled(cancel)?;
        let image = temp.join(format!("hn-{index}.jpg"));
        source.extract(offset, length, &image, cancel)?;
        images.push(image);
    }
    engine.request(
        json!({"action":"images_pdf","inputs":images,"flipVertical":pages.iter().map(|page| page.2).collect::<Vec<_>>(),"output":output}),
        cancel,
    )?;
    let info = engine.request(json!({"action":"info","input":output}), cancel)?;
    if info["pages"].as_u64() != Some(pages.len() as u64) {
        return Err("HN conversion lost pages; output rejected".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(offset: u32) -> tempfile::NamedTempFile {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        let mut header = [0u8; 0x64];
        header[0] = 0xc8;
        header[8..12].copy_from_slice(&1u32.to_le_bytes());
        header[0x50..0x54].copy_from_slice(&offset.to_le_bytes());
        header[0x58] = 1;
        file.write_all(&header).unwrap();
        file.seek(SeekFrom::Start(offset as u64)).unwrap();
        for value in [1, offset + 12, 4] {
            file.write_all(&value.to_le_bytes()).unwrap();
        }
        file.write_all(&[0xff, 0xd8, 0xff, 0xd9]).unwrap();
        file
    }
    #[test]
    fn seeks_large_sparse_input_without_whole_file_allocation() {
        let file = fixture(150 * 1024 * 1024);
        let cancel = crate::engine::fresh_cancel();
        let mut source = Source::open(file.path()).unwrap();
        let pages = source.pages(&cancel).unwrap();
        assert_eq!(pages, vec![(150 * 1024 * 1024 + 12, 4, false)]);
        let extracted = tempfile::NamedTempFile::new().unwrap();
        source
            .extract(pages[0].0, 4, extracted.path(), &cancel)
            .unwrap();
        assert_eq!(
            fs::read(extracted.path()).unwrap(),
            [0xff, 0xd8, 0xff, 0xd9]
        );
    }
    #[test]
    fn rejects_truncation_table_overlap_large_images_and_cancellation() {
        let file = fixture(0x64);
        let cancel = crate::engine::fresh_cancel();
        let mut source = Source::open(file.path()).unwrap();
        cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(source.pages(&cancel).unwrap_err(), "CANCELLED");
        cancel.store(false, std::sync::atomic::Ordering::Relaxed);
        source.len -= 1;
        assert!(source.pages(&cancel).unwrap_err().contains("Truncated"));
        let mut file = fixture(0x64);
        file.seek(SeekFrom::Start(0x50)).unwrap();
        file.write_all(&0x60u32.to_le_bytes()).unwrap();
        assert!(Source::open(file.path())
            .unwrap()
            .pages(&cancel)
            .unwrap_err()
            .contains("overlaps"));
        let mut file = fixture(0x64);
        file.seek(SeekFrom::Start(0x6c)).unwrap();
        file.write_all(&(MAX_IMAGE as u32 + 1).to_le_bytes())
            .unwrap();
        file.as_file().set_len(MAX_IMAGE + 0x71).unwrap();
        assert!(Source::open(file.path())
            .unwrap()
            .pages(&cancel)
            .unwrap_err()
            .contains("64 MiB"));
    }
}
