//! Traffic padding protocol for NaiveProxy.
//!
//! Maps to cronet-go's naive_conn.go padding logic. Adds random padding to
//! the first 8 chunks to obfuscate traffic patterns.

use std::io::{Read, Write};

const PADDING_COUNT: usize = 8;
const MAX_PADDING_CHUNK_SIZE: usize = 65535;

/// Manages padding for a NaiveProxy connection.
#[derive(Clone, Debug)]
pub struct PaddingManager {
    read_padding: usize,
    write_padding: usize,
    read_remaining: usize,
    padding_remaining: usize,
}

impl PaddingManager {
    pub fn new() -> Self {
        Self {
            read_padding: 0,
            write_padding: 0,
            read_remaining: 0,
            padding_remaining: 0,
        }
    }

    /// Read data with padding stripping.
    /// Padding-only frames are skipped. EOF is clean only between complete
    /// frames; truncated frames and transport errors are propagated.
    pub fn read_with_padding<R: Read>(
        &mut self,
        reader: &mut R,
        buffer: &mut [u8],
    ) -> std::io::Result<(usize, bool)> {
        if buffer.is_empty() {
            return Ok((0, false));
        }
        let mut had_frame = false;
        loop {
            if self.read_remaining > 0 {
                let end = self.read_remaining.min(buffer.len());
                let n = reader.read(&mut buffer[..end])?;
                if n == 0 {
                    return Err(std::io::ErrorKind::UnexpectedEof.into());
                }
                self.read_remaining -= n;
                return Ok((n, had_frame));
            }
            if self.padding_remaining > 0 {
                let mut padding = [0; 255];
                reader.read_exact(&mut padding[..self.padding_remaining])?;
                self.padding_remaining = 0;
            }
            if self.read_padding == PADDING_COUNT {
                return reader.read(buffer).map(|n| (n, had_frame));
            }
            let mut header = [0u8; 3];
            // Only EOF at a frame boundary is a clean end of the stream.
            loop {
                match reader.read(&mut header[..1]) {
                    Ok(0) => return Ok((0, had_frame)),
                    Ok(_) => break,
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(error) => return Err(error),
                }
            }
            reader.read_exact(&mut header[1..])?;
            self.read_remaining = u16::from_be_bytes([header[0], header[1]]) as usize;
            self.padding_remaining = header[2] as usize;
            self.read_padding += 1;
            had_frame = true;
            // Empty padding frames carry no application data. Consume them
            // and continue instead of returning Rust's EOF sentinel, Ok(0).
        }
    }

    /// Write data with padding.
    pub fn write_with_padding<W: Write>(
        &mut self,
        writer: &mut W,
        data: &[u8],
    ) -> std::io::Result<usize> {
        let len = data.len();

        if self.write_padding < PADDING_COUNT {
            if len > MAX_PADDING_CHUNK_SIZE {
                return self.write_chunked(writer, data);
            }

            use rand::Rng;
            let mut rng = rand::thread_rng();
            let padding_size = rng.gen_range(0..256);

            let mut packet = Vec::with_capacity(3 + len + padding_size);
            packet.extend_from_slice(&(len as u16).to_be_bytes());
            packet.push(padding_size as u8);
            packet.extend_from_slice(data);
            for _ in 0..padding_size {
                packet.push(0);
            }

            writer.write_all(&packet)?;
            self.write_padding += 1;
            return Ok(len);
        }

        writer.write_all(data)?;
        Ok(len)
    }

    /// Write large data by chunking.
    fn write_chunked<W: Write>(&mut self, writer: &mut W, data: &[u8]) -> std::io::Result<usize> {
        let mut written = 0;
        let mut remaining = data;

        while !remaining.is_empty() {
            let chunk_end = MAX_PADDING_CHUNK_SIZE.min(remaining.len());
            let chunk = &remaining[..chunk_end];
            remaining = &remaining[chunk_end..];

            let n = self.write_with_padding(writer, chunk)?;
            written += n;
        }

        Ok(written)
    }

    pub fn reader_replaceable(&self) -> bool {
        self.read_padding == PADDING_COUNT
    }

    pub fn writer_replaceable(&self) -> bool {
        self.write_padding == PADDING_COUNT
    }

    pub fn front_headroom(&self) -> usize {
        if self.write_padding < PADDING_COUNT {
            3
        } else {
            0
        }
    }

    pub fn rear_headroom(&self) -> usize {
        if self.write_padding < PADDING_COUNT {
            255
        } else {
            0
        }
    }

    pub fn writer_mtu(&self) -> usize {
        if self.write_padding < PADDING_COUNT {
            MAX_PADDING_CHUNK_SIZE
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_padding_roundtrip() {
        let data = b"Hello, NaiveProxy!";

        // Write with padding
        let mut padded_buf = Vec::new();
        {
            let mut cursor = Cursor::new(&mut padded_buf);
            let mut pm = PaddingManager::new();
            pm.write_with_padding(&mut cursor, data).unwrap();
        }

        // Read back with padding stripping
        let mut out = vec![0u8; 128];
        let mut pm2 = PaddingManager::new();
        let mut reader = Cursor::new(&padded_buf);
        let (n, _) = pm2.read_with_padding(&mut reader, &mut out).unwrap();
        assert_eq!(&out[..n], data);
    }

    #[test]
    fn test_padding_multiple_writes() {
        let data = b"Hello, NaiveProxy!";

        let mut padded_buf = Vec::new();

        // Write multiple times with padding (only first PADDING_COUNT get framed)
        {
            let mut cursor = Cursor::new(&mut padded_buf);
            let mut pm = PaddingManager::new();
            for _ in 0..PADDING_COUNT {
                pm.write_with_padding(&mut cursor, data).unwrap();
            }
        }

        // Read back
        let mut out = vec![0u8; 128];
        let mut pm2 = PaddingManager::new();
        let mut reader = Cursor::new(&padded_buf);
        for _ in 0..PADDING_COUNT {
            let (n, _) = pm2.read_with_padding(&mut reader, &mut out).unwrap();
            assert_eq!(&out[..n], data);
        }
    }
}
