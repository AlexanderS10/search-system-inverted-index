//!Loads collection statistics (collection_stats.json) and the document table (doc_table.tsv) created by the parser
//!To keep the memory footprint low documents are sotred ina contiguous vector
//!  

use common::CollectionStats;
use std::fs::File;
use std::io::{self, BufRead, BufReader};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DocEntry {
    pub external_id: u32,
    pub doc_len: u32,
}

/// This will provide O(1) lookups from internal doc_id to its metadata
pub struct DocTable {
    entries: Vec<DocEntry>,
}

impl DocTable {
    /// Loads the tsv document table form disk into the contiguous vector
    /// Argumetns:
    ///     path: Path to the doc_table.tsv
    ///     expected_docs: Capacity hint to avoid dynamic reallocation for the sake of speed really
    ///
    /// Errors
    ///     Returns an io::Error if th efile cannot be opened or parsed jsut in case
    pub fn load_from_tsv(path: &str, expected_docs: usize) -> io::Result<Self> {
        let file = File::open(path)?;
        let mut reader = BufReader::with_capacity(2 * 1024 * 1024, file); //this iwll use a 2MB buffer to speed up reading (avoid context switching)
        let mut entries = Vec::with_capacity(expected_docs);
        let mut line = String::new();

        while reader.read_line(&mut line)? > 0 {
            let trimmed = line.trim_end();
            if trimmed.is_empty() {
                line.clear();
                continue;
            }
            //since the expected schema is doc_id, external_id, doc_len (yes it is a tsv)
            let mut parts = trimmed.split('\t');
            let _doc_id_str = parts.next();
            let ext_id_str = parts.next();
            let doc_len_str = parts.next();
            
            //Use patter matching to prevent any corruption from breaking the program
            if let (Some(ext), Some(len)) = (ext_id_str, doc_len_str) {
                let external_id = ext.parse::<u32>().map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "invalid external_id")
                })?;
                let doc_len = len
                    .parse::<u32>()
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid doc_len"))?;

                entries.push(DocEntry {
                    external_id,
                    doc_len,
                });
            }
            line.clear();
        }
        Ok(Self { entries })
    }

    ///Returns the metadata for a given internal doc_id
    #[inline]
    pub fn get(&self, doc_id: u32) -> Option<&DocEntry> {
        return self.entries.get(doc_id as usize);
    }

    ///Total number of indexed documents loaded
    #[inline]
    pub fn len(&self) -> usize {
        return self.entries.len();
    }

    ///If the document table is empty
    #[inline]
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        return self.entries.is_empty();
    }
}

///Loads the flobal collection stats from collection_stats.json
/// Arguments:
///     path: path to the collection_stats.json
///
/// Errors:
///     Returns an io::error if the json file cannot be read or deserialized
pub fn load_stats(path: &str) -> io::Result<CollectionStats> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let stats: CollectionStats = serde_json::from_reader(reader)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(stats)
}
