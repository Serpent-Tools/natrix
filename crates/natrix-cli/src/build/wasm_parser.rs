//! Shared WASM parsing utilities for streaming and parsing WASM files

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use crate::prelude::*;

/// Result of parsing a WASM file containing both data and custom sections
#[derive(Debug)]
pub struct WasmParseResult {
    /// Custom sections keyed by name
    pub custom_sections: HashMap<String, Vec<u8>>,
    /// Strings extracted from data sections
    pub data_strings: Vec<String>,
    /// Offset of the code section in the file
    pub code_section_offset: u64,
}

/// Parse a WASM file from a reader using streaming mode
pub fn parse_wasm_stream<R: Read>(mut reader: R) -> Result<WasmParseResult> {
    let mut custom_sections = HashMap::new();
    let mut data_strings = Vec::new();
    let mut code_section_offset = 0;

    let mut parser = wasmparser::Parser::new(0);
    let mut current_data_length = 0;
    let mut buffer = vec![0; 8192]; // 8KB buffer for streaming
    let mut eof = false;

    loop {
        let slice = buffer.get(..current_data_length).ok_or_else(|| {
            anyhow!("Buffer state corruption: current_data_length > buffer.len()")
        })?;
        let (payload, consumed) = match parser.parse(slice, eof)? {
            wasmparser::Chunk::NeedMoreData(hint) => {
                if eof {
                    break;
                }

                let hint = hint.try_into().unwrap_or(usize::MAX);
                let target_size = hint.saturating_add(current_data_length);

                if target_size > buffer.len() {
                    buffer.resize(target_size, 0);
                }

                let write_slice = buffer
                    .get_mut(current_data_length..)
                    .ok_or_else(|| anyhow!("Buffer state corruption: cannot get write slice"))?;
                let bytes_read = reader.read(write_slice)?;
                if bytes_read == 0 {
                    eof = true;
                } else {
                    current_data_length = current_data_length.saturating_add(bytes_read);
                }
                continue;
            }
            wasmparser::Chunk::Parsed { consumed, payload } => (payload, consumed),
        };

        match payload {
            wasmparser::Payload::CustomSection(section_reader) => {
                custom_sections.insert(
                    section_reader.name().to_owned(),
                    section_reader.data().to_vec(),
                );
            }
            wasmparser::Payload::DataSection(data_section_reader) => {
                for data in data_section_reader {
                    let data = data?;
                    if let Some(bytes) = data.data.get(0..) {
                        if let Ok(string) = std::str::from_utf8(bytes) {
                            data_strings.push(string.to_owned());
                        } else {
                            // Clean out problematic bytes
                            //
                            // Natrix generated class and id names are always ASCII
                            // So this is generally safe.
                            let cleaned = bytes
                                .iter()
                                .filter(|&&x| x.is_ascii())
                                .copied()
                                .collect::<Vec<u8>>();
                            if let Ok(string) = String::from_utf8(cleaned) {
                                data_strings.push(string);
                            } else {
                                return Err(anyhow!(
                                    "Failed to extract string from wasm, this might lead to wrongful DCE optimization"
                                ));
                            }
                        }
                    }
                }
            }
            wasmparser::Payload::CodeSectionStart { range, .. } => {
                code_section_offset = range.start as u64;
            }
            wasmparser::Payload::End(_) => break,
            _ => {}
        }

        current_data_length = current_data_length.saturating_sub(consumed);
        if current_data_length != 0 {
            buffer.drain(..consumed);
        }
    }

    Ok(WasmParseResult {
        custom_sections,
        data_strings,
        code_section_offset,
    })
}

/// Parse a WASM file from a file path
pub fn parse_wasm_file(wasm_file: &Path) -> Result<WasmParseResult> {
    let file = std::fs::File::open(wasm_file)?;
    parse_wasm_stream(file)
}
