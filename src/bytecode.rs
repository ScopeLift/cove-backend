use ethers::types::Bytes;
use ethers_solc::artifacts::Offsets;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]

/// Defines the types of bytecode matches that can occur.
pub enum MatchType {
    /// A full match means the bytecode and the metadata hash match.
    Full,
    /// A partial match means the bytecode matches, but the metadata hash does not.
    Partial,
    /// No match means the bytecode does not match.
    #[default]
    None,
}

/// Contains info about the the bytecode's metadata hash.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct MetadataInfo {
    /// The metadata hash if present.
    pub hash: Option<Bytes>,
    /// Start index of the metadata within bytecode.
    pub start_index: Option<usize>,
    /// End index of the metadata within bytecode.
    pub end_index: Option<usize>,
}

/// Data about found creation bytecode, where "found" bytecode is bytecode from an artifact that was
/// output when compiling the repo.
#[derive(Debug, PartialEq, Eq)]
pub struct FoundCreationBytecode {
    /// The raw, unadjusted bytecode.
    pub raw_code: Bytes,
    /// All bytecode leading up to the metadata hash.
    pub leading_code: Bytes,
    /// Information about the metadata hash.
    pub metadata: MetadataInfo,
}

/// Data about expected creation bytecode, where "expected" bytecode is the bytecode that exists
/// on-chain that is being verified against.
#[derive(Debug, PartialEq, Eq)]
pub struct ExpectedCreationBytecode {
    /// The raw, unadjusted bytecode.
    pub raw_code: Bytes,
    /// The bytecode leading up to the metadata hash.
    pub leading_code: Bytes,
    /// Information about the metadata hash.
    pub metadata: MetadataInfo,
    /// Optional constructor arguments, which are typically appended to the creation code before
    /// the metadata hash.
    pub constructor_args: Option<Bytes>,
}

/// Type alias for a mapping between immutable reference identifiers and their offsets within
/// bytecode. This contains data from the `immutableReferences` field of an artifact.
pub type ImmutableReferences = BTreeMap<String, Vec<Offsets>>;

/// Data about found deployed bytecode, where "found" bytecode is bytecode from an artifact that was
/// output when compiling the repo.
#[derive(Debug, PartialEq, Eq)]
pub struct FoundDeployedBytecode {
    /// The raw, unadjusted bytecode.
    pub raw_code: Bytes,
    /// All bytecode leading up to the metadata hash.
    pub leading_code: Bytes,
    /// Information about the metadata hash.
    pub metadata: MetadataInfo,
    /// Immutable references and their offsets within bytecode.
    pub immutable_references: ImmutableReferences,
}

/// Data about expected deployed bytecode, where "expected" bytecode is the bytecode that exists
/// on-chain that is being verified against.
#[derive(Debug, PartialEq, Eq)]
pub struct ExpectedDeployedBytecode {
    /// The raw, unadjusted bytecode.
    pub raw_code: Bytes,
    /// The bytecode leading up to the metadata hash.
    pub leading_code: Bytes,
    /// Information about the metadata hash.
    pub metadata: MetadataInfo,
    /// Immutable references and their offsets within bytecode.
    pub immutable_references: ImmutableReferences,
}

/// Checks for equality between found and expected creation bytecode and returns the type of match.
pub fn creation_code_equality_check(
    found: &FoundCreationBytecode,
    expected: &ExpectedCreationBytecode,
) -> MatchType {
    // If bytecode is empty, we have an interface, and we can't match with an interface.
    if found.raw_code.is_empty() {
        return MatchType::None
    }

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

/// Checks for equality between found and expected deployed bytecode and returns the type of match.
pub fn deployed_code_equality_check(
    found: &FoundDeployedBytecode,
    expected: &ExpectedDeployedBytecode,
) -> MatchType {
    // If bytecode is empty, we have an interface, and we can't match with an interface.
    if found.raw_code.is_empty() {
        return MatchType::None
    }

    // Expected and found code must have the same length.
    if found.raw_code.len() != expected.raw_code.len() {
        return MatchType::None
    }

    // Simple check for exact match.
    if found.raw_code == expected.raw_code {
        return MatchType::Full
    }

    // Compare the leading code, but skip all chunks that contain immutables.
    if found.immutable_references == expected.immutable_references {
        // Flatten the map to just a vec of the references. Since we know found and expected have
        // equal immutable references due to how the structs were constructed, we can just use the
        // found ones.
        let mut offsets: Vec<Offsets> = Vec::new();
        for new_offsets in found.immutable_references.values() {
            offsets.extend(new_offsets.iter().cloned());
        }

        // The expected bytecode is deployed and therefore has real values for the immutables. The
        // found bytecode uses zeroes as placeholders for the immutables (this is how solc works).
        // Therefore for each immutable reference in the expected bytecode, we can replace the
        // bytecode with zeroes, then compare the found bytecode with the expected bytecode.
        // It's likely the metadata hashes won't match, so we adjust both the raw and leading code
        // so we only have to loop through the offsets once.
        let mut adjusted_expected_raw_code = expected.raw_code.to_vec();
        let mut adjusted_expected_leading_code = expected.leading_code.to_vec();
        for offset in offsets {
            let immutable_start: usize = offset.start.try_into().unwrap();
            let immutable_length: usize = offset.length.try_into().unwrap();
            let immutable_end = immutable_start + immutable_length;
            for i in immutable_start..immutable_end {
                adjusted_expected_raw_code[i] = 0;
                adjusted_expected_leading_code[i] = 0;
            }
        }

        // This matched with the metadata hash, so it's a full match.
        if adjusted_expected_raw_code == found.raw_code {
            return MatchType::Full
        }

        // Had to remove the metadata hash, so it's a partial match.
        if adjusted_expected_leading_code == found.leading_code {
            return MatchType::Partial
        }
    }

    MatchType::None
}

/// Given code, infers and returns the metadata details.
///
/// The implied length returned by this method, i.e. `end_index - start_index`, is the decimal value
/// of the last two bytes plus 2 bytes for the length itself. In other words, this returns the total
/// length of the metadata hash.
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

/// Returns a tuple of `(everything before the metadata hash, everything after the metadata hash)`.
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

/// The length returned by this method is the decimal value of the last two bytes. The total length
/// of the metadata hash is this value plus 2 bytes for the length itself.
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
    fn test_creation_code_equality_check() -> Result<(), Box<dyn std::error::Error>> {
        let found_code = Bytes::from_str("60606040525b6102c05b60")?;
        let partial_match_code = Bytes::from_str("60606040525b6102c05b600000000000")?;
        let no_match_code = Bytes::from_str("ff")?;

        let found = FoundCreationBytecode {
            raw_code: found_code.clone(),
            leading_code: found_code.clone(),
            metadata: MetadataInfo::default(),
        };

        let expected_full = ExpectedCreationBytecode {
            raw_code: found_code.clone(),
            leading_code: found_code.clone(),
            metadata: MetadataInfo::default(),
            constructor_args: None,
        };

        let expected_partial = ExpectedCreationBytecode {
            raw_code: partial_match_code.clone(),
            leading_code: found_code.clone(),
            metadata: MetadataInfo::default(),
            constructor_args: None,
        };

        let expected_none = ExpectedCreationBytecode {
            raw_code: no_match_code.clone(),
            leading_code: no_match_code.clone(),
            metadata: MetadataInfo::default(),
            constructor_args: None,
        };

        assert_eq!(creation_code_equality_check(&found, &expected_none), MatchType::None);
        assert_eq!(creation_code_equality_check(&found, &expected_full), MatchType::Full);
        assert_eq!(creation_code_equality_check(&found, &expected_partial), MatchType::Partial);

        Ok(())
    }

    #[test]
    #[ignore = "TODO"]
    fn test_deployed_code_equality_check() -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    #[test]
    fn test_parse_metadata() -> Result<(), Box<dyn std::error::Error>> {
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
}
