//! Integration tests for the NaiveProxy padding protocol.
//!
//! These tests do not require libcronet — they test pure Rust logic.

use std::io::Cursor;

use cronet_rs::padding::PaddingManager;

/// Write helper: creates a padded payload buffer.
fn write_padded(data: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut cursor = Cursor::new(&mut buf);
    let mut pm = PaddingManager::new();
    pm.write_with_padding(&mut cursor, data).unwrap();
    buf
}

/// Read helper: extracts all data from a padded buffer (loops to drain).
fn read_padded(buf: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut tmp = vec![0u8; 131072]; // 2x max chunk size to handle chunked writes
    let mut reader = Cursor::new(buf);
    let mut pm = PaddingManager::new();
    loop {
        let (n, _) = pm.read_with_padding(&mut reader, &mut tmp).unwrap();
        if n == 0 {
            break;
        }
        out.extend_from_slice(&tmp[..n]);
    }
    out
}

#[test]
fn test_padding_roundtrip_small() {
    let data = b"hello";
    let padded = write_padded(data);
    let restored = read_padded(&padded);
    assert_eq!(restored, data);
}

#[test]
fn test_padding_roundtrip_large() {
    let data = vec![0xABu8; 65535];
    let padded = write_padded(&data);
    let restored = read_padded(&padded);
    assert_eq!(restored, data);
}

#[test]
fn test_padding_roundtrip_empty() {
    let data = b"";
    let padded = write_padded(data);
    let restored = read_padded(&padded);
    assert_eq!(restored, data);
}

#[test]
fn test_padding_8_chunks_framed_then_plain() {
    let mut writer_buf = Vec::new();
    let mut pm_write = PaddingManager::new();
    let mut cursor = Cursor::new(&mut writer_buf);

    // First 8 chunks get padded framing
    for i in 0..8 {
        let msg = vec![b'A' + i; 10];
        pm_write.write_with_padding(&mut cursor, &msg).unwrap();
    }

    // 9th chunk is plain (no framing)
    let plain_msg = b"this is plain data";
    pm_write.write_with_padding(&mut cursor, plain_msg).unwrap();

    // Read back
    let mut out = vec![0u8; 1024];
    let mut pm_read = PaddingManager::new();
    let mut reader = Cursor::new(&writer_buf);

    // First 8 should be framed (padding detected)
    for i in 0..8 {
        let (n, had_frame) = pm_read.read_with_padding(&mut reader, &mut out).unwrap();
        assert_eq!(out[..n], vec![b'A' + i; 10]);
        assert!(had_frame, "chunk {} should have had a frame", i);
    }

    // 9th chunk should be plain
    let (n, had_frame) = pm_read.read_with_padding(&mut reader, &mut out).unwrap();
    assert_eq!(&out[..n], plain_msg);
    assert!(!had_frame, "chunk 9 should be plain");
}

#[test]
fn test_padding_state_replaceable() {
    let mut pm = PaddingManager::new();

    // Write 8 chunks
    let mut buf = Vec::new();
    let mut cursor = Cursor::new(&mut buf);
    for _ in 0..8 {
        pm.write_with_padding(&mut cursor, b"test").unwrap();
    }

    // After 8 writes, writer should be replaceable
    assert!(pm.writer_replaceable());

    // Before any reads, reader is not replaceable (assuming PADDING_COUNT=8)
    assert!(!pm.reader_replaceable());
}

#[test]
fn test_padding_headroom() {
    let mut pm = PaddingManager::new();
    assert_eq!(pm.front_headroom(), 3, "before any write");
    assert_eq!(pm.rear_headroom(), 255, "before any write");

    // After 8 writes, headroom should be 0
    let mut buf = Vec::new();
    let mut cursor = Cursor::new(&mut buf);
    for _ in 0..8 {
        pm.write_with_padding(&mut cursor, b"test").unwrap();
    }
    assert_eq!(pm.front_headroom(), 0);
    assert_eq!(pm.rear_headroom(), 0);
}

#[test]
fn test_padding_mtu() {
    let mut pm = PaddingManager::new();
    assert!(pm.writer_mtu() > 0, "before padding done, mtu should be > 0");

    let mut buf = Vec::new();
    let mut cursor = Cursor::new(&mut buf);
    for _ in 0..8 {
        pm.write_with_padding(&mut cursor, b"test").unwrap();
    }
    assert_eq!(pm.writer_mtu(), 0, "after padding done, mtu should be 0");
}

#[test]
fn test_padding_chunked_write() {
    // Write a buffer larger than MAX_PADDING_CHUNK_SIZE
    let data = vec![0x42u8; 70000]; // > 65535
    let mut buf = Vec::new();
    let mut cursor = Cursor::new(&mut buf);
    let mut pm = PaddingManager::new();
    let written = pm.write_with_padding(&mut cursor, &data).unwrap();
    assert_eq!(written, data.len());

    // Read back (chunked → multiple reads)
    let mut out = Vec::new();
    let mut tmp = vec![0u8; 70000];
    let mut pm2 = PaddingManager::new();
    let mut reader = Cursor::new(&buf);
    loop {
        let (n, _) = pm2.read_with_padding(&mut reader, &mut tmp).unwrap();
        if n == 0 {
            break;
        }
        out.extend_from_slice(&tmp[..n]);
    }
    assert_eq!(out, data, "chunked write roundtrip mismatch");
}

#[test]
fn test_padding_deterministic_read_write_count() {
    let mut pm = PaddingManager::new();
    let initial = pm.clone();
    let mut buf = Vec::new();
    let mut cursor = Cursor::new(&mut buf);

    // Write once
    pm.write_with_padding(&mut cursor, b"data").unwrap();
    // The write buffer should have 3 bytes header + 4 bytes data + random padding
    assert!(buf.len() >= 7, "buffer should have header + data");
    assert!(buf.len() <= 3 + 4 + 255, "buffer should not exceed max frame");

    drop(pm);
    drop(initial);
}

#[test]
fn test_multiple_small_writes_reads() {
    let messages: &[&[u8]] = &[b"a", b"bb", b"ccc", b"dddd", b"eeeee"];
    let mut buf = Vec::new();
    let mut cursor = Cursor::new(&mut buf);
    let mut pm = PaddingManager::new();

    for msg in messages {
        pm.write_with_padding(&mut cursor, msg).unwrap();
    }

    let mut out = vec![0u8; 64];
    let mut pm2 = PaddingManager::new();
    let mut reader = Cursor::new(&buf);
    for msg in messages {
        let (n, _) = pm2.read_with_padding(&mut reader, &mut out).unwrap();
        assert_eq!(&out[..n], *msg, "roundtrip mismatch for {:?}", msg);
    }
}

/// Test that padding headers have random sizes (non-deterministic)
#[test]
fn test_padding_randomness() {
    let mut sizes = Vec::new();
    for _ in 0..10 {
        let mut buf = Vec::new();
        let mut cursor = Cursor::new(&mut buf);
        let mut pm = PaddingManager::new();
        pm.write_with_padding(&mut cursor, b"test").unwrap();
        sizes.push(buf.len());
    }

    // At least 2 different sizes means randomness is working
    let unique = sizes.iter().collect::<std::collections::HashSet<_>>();
    assert!(unique.len() >= 2, "padding should produce varying sizes");
}
