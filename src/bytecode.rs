use ethers::{solc::artifacts::Offsets, types::Bytes};
use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Eq)]
pub enum MatchType {
    Full,
    Partial,
    None,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct MetadataInfo {
    pub hash: Option<Bytes>,
    pub start_index: Option<usize>,
    pub end_index: Option<usize>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct FoundCreationBytecode {
    pub raw_code: Bytes,
    pub leading_code: Bytes,
    pub metadata: MetadataInfo,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ExpectedCreationBytecode {
    pub raw_code: Bytes,
    pub leading_code: Bytes,
    pub metadata: MetadataInfo,
    pub constructor_args: Option<Bytes>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct FoundDeployedBytecode {
    pub raw_code: Bytes,
    pub leading_code: Bytes,
    pub metadata: MetadataInfo,
    pub immutable_references: BTreeMap<String, Vec<Offsets>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ExpectedDeployedBytecode {
    pub raw_code: Bytes,
    pub leading_code: Bytes,
    pub metadata: MetadataInfo,
    pub immutable_references: BTreeMap<String, Vec<Offsets>>,
}

pub fn creation_code_equality_check(
    found: &FoundCreationBytecode,
    expected: &ExpectedCreationBytecode,
) -> MatchType {
    // Expected code might contain appended constructor arguments, so if code matches then expected
    // can only be equal to or longer than found code.
    if found.raw_code.len() > expected.raw_code.len() {
        return MatchType::None
    }
    if found.raw_code == expected.raw_code {
        return MatchType::Full
    }
    if found.leading_code == expected.leading_code {
        return MatchType::Partial
    }

    MatchType::None
}

pub fn deployed_code_equality_check(
    found: &FoundDeployedBytecode,
    expected: &ExpectedDeployedBytecode,
) -> MatchType {
    // Expected and found code must have the same length.
    if found.raw_code.len() != expected.raw_code.len() {
        return MatchType::None
    }

    if found.raw_code == expected.raw_code {
        return MatchType::Full
    }

    // Compare the leading code, but skip all chunks that contain immutables.
    if found.immutable_references == expected.immutable_references {
        // Flatten the map to just a vec of the references. Since we know found and expected have
        // equal immutable references, we can just use the found ones.
        let mut offsets: Vec<Offsets> = Vec::new();
        for new_offsets in found.immutable_references.values() {
            offsets.extend(new_offsets.iter().cloned());
        }

        let mut start: usize = 0;
        let mut matches = true;

        for offset in offsets {
            // Slice the bytecode from the `start` index until the immutable's start index.
            let immutable_start: usize = offset.start.try_into().unwrap();
            let found_chunk = &found.leading_code[start..immutable_start];
            let expected_chunk = &expected.leading_code[start..immutable_start];

            // If the chunks don't match, code does not match.
            if found_chunk != expected_chunk {
                matches = false;
                break
            }

            // If the chunks do match, update `start` to be after the immutable and keep looping
            // through the offsets.
            let immutable_length: usize = offset.length.try_into().unwrap();
            start += immutable_length;
        }

        // Now we check the final chunk of the bytecode after the last immutable.
        if matches && found.leading_code[start..] == expected.leading_code[start..] {
            return MatchType::Partial
        }
    }

    MatchType::None
}

// The implied length returned by this method, i.e. `end_index - start_index`, is the decimal value
// of the last two bytes plus 2 bytes for the length itself. In other words, this returns the total
// length of the metadata hash.
pub fn parse_metadata(code: &Bytes) -> MetadataInfo {
    let (leading_code, metadata_hash) = split_at_metadata_hash(code);
    let metadata_start_index =
        if metadata_hash.is_some() { Some(leading_code.len()) } else { None };
    let metadata_end_index = if metadata_start_index.is_some() { Some(code.len()) } else { None };

    MetadataInfo {
        hash: metadata_hash,
        start_index: metadata_start_index,
        end_index: metadata_end_index,
    }
}

// Returns a tuple of (everything before the metadata hash, everything after the metadata hash)
fn split_at_metadata_hash(code: &Bytes) -> (Bytes, Option<Bytes>) {
    // Read the length of the metadata hash from the last two bytes.
    let metadata_hash_length = get_metadata_hash_length(code);
    if metadata_hash_length.is_none() {
        return (code.to_vec().into(), None)
    }

    // Split the code. We subtract 2 to get the split index because the last two bytes are the
    // metadata length, and that value is exclusive of those two bytes. So the total length of
    // the metadata hash that we want needs to include those two bytes.
    let split_index = code.len() - metadata_hash_length.unwrap() - 2;
    let (code_before, maybe_metadata_hash) = code.split_at(split_index);
    if maybe_metadata_hash.is_empty() {
        (code.to_vec().into(), None)
    } else {
        (code_before.to_vec().into(), Some(maybe_metadata_hash.to_vec().into()))
    }
}

// The length returned by this method is the decimal value of the last two bytes. The total length
// of the metadata hash is this value plus 2 bytes for the length itself.
fn get_metadata_hash_length(code: &Bytes) -> Option<usize> {
    if code.len() <= 2 {
        return None
    }
    // We take the second-to-last byte and shift it by 8 bits, since it's the larger value byte.
    // Then we take bitwise-or this with the last byte to get the length.
    let len = ((code[code.len() - 2] as usize) << 8) | (code[code.len() - 1] as usize);

    // If the last two bytes give a length longer than the code, then it wasn't actually the
    // metadata hash.
    if len > code.len() - 2 {
        None
    } else {
        Some(len)
    }
}

#[cfg(test)]
mod tests {
    // Some test data taken from https://playground.sourcify.dev/.
    use super::*;
    use std::str::FromStr;

    #[test]
    fn parse_metadata() -> Result<(), Box<dyn std::error::Error>> {
        let test_cases = vec![
            (19, Bytes::from_str("ffffffffffffffffffffffffffffffffff0011")?),
            (258, Bytes::from_str("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff0100")?)
        ];
        for (expected_len, data) in test_cases {
            let metadata = super::parse_metadata(&data);
            assert_eq!(expected_len, metadata.end_index.unwrap() - metadata.start_index.unwrap());
        }
        Ok(())
    }

    #[test]
    fn get_metadata_hash_length() -> Result<(), Box<dyn std::error::Error>> {
        #[rustfmt::skip]
        let test_cases = vec![
            (Some(17), Bytes::from_str("ffffffffffffffffffffffffffffffffff0011")?),
            (Some(256), Bytes::from_str("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff0100")?),
            (Some(10), Bytes::from_str("ffffffffffffffffffffff000a")?),
            (None, Bytes::from_str("")?),
            (None, Bytes::from_str("ff")?),
            (None, Bytes::from_str("")?),
            (None, Bytes::from_str("0000000000ff")?),
        ];

        for (expected_length, code) in test_cases {
            let length = super::get_metadata_hash_length(&code);
            assert_eq!(length, expected_length);
        }
        Ok(())
    }

    #[test]
    fn split_at_metadata_hash() -> Result<(), Box<dyn std::error::Error>> {
        #[rustfmt::skip]
        let test_cases = vec![
            ("676e6174757265206c656e677468a2646970667358221220dceca8706b29e917dacf25fceef95acac8d90d765ac926663ce4096195952b6164736f6c634300060b0033","676e6174757265206c656e677468","a2646970667358221220dceca8706b29e917dacf25fceef95acac8d90d765ac926663ce4096195952b6164736f6c634300060b0033"),
            ("57600080fd5b5056fea164736f6c6343000706000a","57600080fd5b5056fe","a164736f6c6343000706000a"),
        ];

        for (code, expected_leading_code, expected_metadata_hash) in test_cases {
            let (leading_code, metadata_hash) =
                super::split_at_metadata_hash(&Bytes::from_str(code)?);
            assert_eq!(leading_code, Bytes::from_str(expected_leading_code)?);
            assert_eq!(metadata_hash, Bytes::from_str(expected_metadata_hash).ok());
        }

        Ok(())
    }
}
