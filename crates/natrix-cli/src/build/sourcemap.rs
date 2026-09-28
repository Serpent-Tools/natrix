//! Generate a source map from DWARF debug info.
//!
//! Based on: <https://github.com/mtolmacs/wasm2map> (MIT-licensed, © Márk Tolmács)
//! Based on: <https://github.com/emscripten-core/emscripten/blob/main/tools/wasm-sourcemap.py>
//!     (MIT-licensed, © 2018 The Emscripten Authors)

use std::collections::HashMap;
use std::io::{Seek, Write};
use std::path::{Path, PathBuf};

use anyhow::anyhow;
use gimli::{EndianSlice, LittleEndian};

use crate::prelude::*;

/// The section id for custom wasm sections
const WASM_CUSTOM_SECTION_ID: u8 = 0;

/// The section name for source maps
const SOURCEMAP_SECTION_NAME: &str = "sourceMappingURL";

/// Create and embed a source map in the given wasm file.
pub fn create_sourcemap(
    wasm_file: &Path,
    parse_result: &super::wasm_parser::WasmParseResult,
) -> Result<()> {
    let mut sourcemap = sourcemap::SourceMapBuilder::new(Some(&wasm_file.display().to_string()));

    let wasm_file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .truncate(false)
        .open(wasm_file)?;

    // Convert parse result to the format expected by populate_sourcemap
    let sections: HashMap<&str, &[u8]> = parse_result
        .custom_sections
        .iter()
        .map(|(section, content)| (section.as_str(), content.as_slice()))
        .collect();

    populate_sourcemap(&mut sourcemap, &sections, parse_result.code_section_offset)?;
    inject_sourcemap(sourcemap, wasm_file)?;

    Ok(())
}

/// Inject the sourcemap into the given wasm file
fn inject_sourcemap(
    sourcemap: sourcemap::SourceMapBuilder,
    mut wasm_file: std::fs::File,
) -> Result<()> {
    let sourcemap = sourcemap.into_sourcemap();
    let data = sourcemap.to_data_url()?;

    let section_content_length = needed_space_for_u32(SOURCEMAP_SECTION_NAME.len().try_into()?)
        .saturating_add(SOURCEMAP_SECTION_NAME.len())
        .saturating_add(needed_space_for_u32(data.len().try_into()?))
        .saturating_add(data.len());
    let section_length = (1_usize)
        .saturating_add(needed_space_for_u32(section_content_length.try_into()?))
        .saturating_add(section_content_length);

    let mut section = Vec::with_capacity(section_length);
    section.push(WASM_CUSTOM_SECTION_ID);
    encode_u32_vlq(section_content_length.try_into()?, &mut section);

    encode_u32_vlq(SOURCEMAP_SECTION_NAME.len().try_into()?, &mut section);
    section.extend(SOURCEMAP_SECTION_NAME.bytes());
    encode_u32_vlq(data.len().try_into()?, &mut section);
    section.extend(data.into_bytes());

    wasm_file.seek(std::io::SeekFrom::End(0))?;
    wasm_file.write_all(&section)?;
    Ok(())
}

/// Parse the debugging sections of the wasm file and use it to populate the sourcemap
fn populate_sourcemap(
    sourcemap: &mut sourcemap::SourceMapBuilder,
    sections: &HashMap<&str, &[u8]>,
    code_section_offset: u64,
) -> Result<()> {
    let empty: [u8; 0] = [];
    let debug_info = gimli::Dwarf::load(
        |id| -> Result<EndianSlice<'_, LittleEndian>, std::convert::Infallible> {
            Ok(EndianSlice::new(
                sections.get(id.name()).unwrap_or(&(&empty as &[u8])),
                LittleEndian,
            ))
        },
    )?;
    let mut seen_files = HashMap::new();
    let mut units = debug_info.units();
    while let Some(unit) = units.next()? {
        let unit = debug_info.unit(unit)?;
        if let Some(line_program) = unit.line_program.clone() {
            let mut rows = line_program.rows();
            while let Some((header, row)) = rows.next_row()? {
                if let Some(file) = row.file(header) {
                    let mut path = PathBuf::new();

                    if let Some(directory) = file.directory(header) {
                        let directory = debug_info.attr_string(&unit, directory)?.to_string_lossy();
                        let directory = Path::new(directory.as_ref());
                        if directory.is_relative()
                            && let Some(comp_dir) = unit.comp_dir
                        {
                            path.push(comp_dir.to_string_lossy().as_ref());
                        }
                        path.push(directory);
                    }

                    let name = debug_info
                        .attr_string(&unit, file.path_name())?
                        .to_string_lossy();
                    path.push(name.as_ref());

                    let mut address = row
                        .address()
                        .checked_add(code_section_offset)
                        .ok_or(anyhow!("Code address overflow"))?;
                    if row.end_sequence() {
                        address = address.saturating_sub(1);
                    }

                    let line = if let Some(line) = row.line() {
                        line.get().saturating_sub(1)
                    } else {
                        0
                    };

                    let column = match row.column() {
                        gimli::ColumnType::LeftEdge => 0,
                        gimli::ColumnType::Column(column) => column.get().saturating_sub(1),
                    };

                    let source_id = if let Some(source_id) = seen_files.get(&path) {
                        *source_id
                    } else {
                        let source_id = sourcemap.add_source(&path.display().to_string());
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            sourcemap.set_source_contents(source_id, Some(&content));
                        } else {
                            sourcemap.add_to_ignore_list(source_id);
                        }
                        seen_files.insert(path.clone(), source_id);

                        source_id
                    };

                    let address = address.try_into()?;
                    let line = line.try_into()?;
                    let column = column.try_into()?;

                    sourcemap.add_raw(0, address, line, column, Some(source_id), None, false);
                }
            }
        }
    }
    Ok(())
}

/// Calculate the size of encoding a u32
const fn needed_space_for_u32(value: u32) -> usize {
    if value < (1 << 7) {
        1
    } else if value < (1 << 14) {
        2
    } else if value < (1 << 21) {
        3
    } else if value < (1 << 28) {
        4
    } else {
        5
    }
}

/// Encode a u32 for wasm
fn encode_u32_vlq(mut value: u32, target: &mut Vec<u8>) {
    while value > 0b0111_1111 {
        let lower_bits = (value & 0b0111_1111) as u8;
        target.push(lower_bits | 0b1000_0000);
        value >>= 7;
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "The above loop means that this is valid"
    )]
    target.push(value as u8);
}
