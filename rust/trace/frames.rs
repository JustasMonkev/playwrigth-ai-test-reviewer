use anyhow::{Context, Result, bail};
use std::io::{BufRead, BufReader, Read};

const MAX_EVENT_BYTES: usize = 8 * 1024 * 1024;

pub(crate) struct Frames<R> {
    reader: BufReader<R>,
    pub(crate) bytes: u64,
    max_bytes: u64,
    started: bool,
    array: bool,
    needs_separator: bool,
    done: bool,
}

impl<R: Read> Frames<R> {
    pub(crate) fn new(reader: R, max_bytes: u64) -> Self {
        Self {
            reader: BufReader::new(reader),
            bytes: 0,
            max_bytes,
            started: false,
            array: false,
            needs_separator: false,
            done: false,
        }
    }

    fn byte(&mut self) -> Result<Option<u8>> {
        let Some(&byte) = self.reader.fill_buf()?.first() else {
            return Ok(None);
        };
        self.reader.consume(1);
        self.bytes += 1;
        if self.bytes > self.max_bytes {
            bail!(
                "Decompressed trace data exceeds max_bytes ({})",
                self.max_bytes
            );
        }
        Ok(Some(byte))
    }

    fn non_space(&mut self) -> Result<Option<u8>> {
        while let Some(byte) = self.byte()? {
            if !matches!(byte, b' ' | b'\n' | b'\r' | b'\t') {
                return Ok(Some(byte));
            }
        }
        Ok(None)
    }

    fn end_array(&mut self) -> Result<Option<Vec<u8>>> {
        if self.non_space()?.is_some() {
            bail!("Unexpected data following JSON array");
        }
        self.done = true;
        Ok(None)
    }

    pub(crate) fn next(&mut self) -> Result<Option<Vec<u8>>> {
        if self.done {
            return Ok(None);
        }
        let mut next = self.non_space()?;
        if !self.started {
            self.started = true;
            if next == Some(b'[') {
                self.array = true;
                next = self.non_space()?;
                if next == Some(b']') {
                    return self.end_array();
                }
            }
        }
        if self.array && self.needs_separator {
            match next {
                Some(b']') => return self.end_array(),
                Some(b',') => next = self.non_space()?,
                _ => bail!("Expected comma or closing bracket in JSON array"),
            }
        }
        match next {
            None if !self.array => {
                self.done = true;
                return Ok(None);
            }
            Some(b'{') => {}
            _ => bail!("Expected a JSON event object"),
        }
        let mut data = vec![b'{'];
        let mut depth = 1usize;
        let mut quoted = false;
        let mut escaped = false;
        while depth > 0 {
            let byte = self.byte()?.context("Unexpected end of JSON event")?;
            if data.len() >= MAX_EVENT_BYTES {
                bail!(
                    "JSON event exceeds the {} byte per-event limit",
                    MAX_EVENT_BYTES
                );
            }
            data.push(byte);
            if quoted {
                if escaped {
                    escaped = false
                } else if byte == b'\\' {
                    escaped = true
                } else if byte == b'"' {
                    quoted = false
                }
            } else {
                match byte {
                    b'"' => quoted = true,
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => depth -= 1,
                    _ => {}
                }
            }
        }
        self.needs_separator = true;
        Ok(Some(data))
    }
}
