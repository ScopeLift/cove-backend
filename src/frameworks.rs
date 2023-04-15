/// Module for abstracting framework-specific logic.
use crate::bytecode::{
    parse_metadata, ExpectedCreationBytecode, ExpectedDeployedBytecode, FoundCreationBytecode,
    FoundDeployedBytecode, ImmutableReferences, MetadataInfo,
};
use ethers::{
    solc::{
        artifacts::{BytecodeHash, BytecodeObject, LosslessAbi, SettingsMetadata},
        ConfigurableContractArtifact,
    },
    types::Bytes,
};
use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
    result::Result,
};
use walkdir::WalkDir;

pub trait Framework {
    fn new(path: &Path) -> Result<Self, Box<dyn Error + Send + Sync>>
    where
        Self: Sized;
    fn is_supported(path: &Path) -> bool;
    fn build_commands(&self) -> Result<Vec<Command>, Box<dyn Error>>;
    fn get_artifacts(&self) -> Result<Vec<PathBuf>, Box<dyn Error>>;

    // Bytecode structuring.
    fn structure_found_creation_code(
        &self,
        artifact: &Path,
    ) -> Result<FoundCreationBytecode, Box<dyn Error>>;
    fn structure_expected_creation_code(
        &self,
        artifact: &Path,
        found: &FoundCreationBytecode,
        expected: &Bytes,
    ) -> Result<ExpectedCreationBytecode, Box<dyn Error>>;
    fn structure_found_deployed_code(
        &self,
        artifact: &Path,
    ) -> Result<FoundDeployedBytecode, Box<dyn Error>>;
    fn structure_expected_deployed_code(
        &self,
        artifact: &Path,
        found: &FoundDeployedBytecode,
        expected: &Bytes,
    ) -> Result<ExpectedDeployedBytecode, Box<dyn Error>>;

    // Artifact parsing.
    fn get_artifact_abi(artifact: &Path) -> Result<LosslessAbi, Box<dyn Error>>;
    fn get_artifact_creation_code(artifact: &Path) -> Result<Bytes, Box<dyn Error>>;
    fn get_artifact_deployed_code(
        artifact: &Path,
    ) -> Result<(Bytes, ImmutableReferences), Box<dyn Error>>;
    fn get_artifact_metadata_settings(artifact: &Path) -> Result<SettingsMetadata, Box<dyn Error>>;
}

pub struct Foundry {
    path: PathBuf,
}

impl Foundry {
    fn filter_artifacts(artifacts: Vec<PathBuf>) -> Vec<PathBuf> {
        // Filter out artifacts where all sources are in the `lib/` directory.
        artifacts
            .into_iter()
            .filter(|a| {
                let content = fs::read_to_string(a).unwrap();
                let json: serde_json::Value = serde_json::from_str(&content).unwrap();
                if let Some(metadata) = json.get("metadata") {
                    if let Some(sources) = metadata.get("sources") {
                        let sources_obj = sources.as_object().unwrap();
                        let all_sources_are_libs =
                            sources_obj.keys().all(|key| key.starts_with("lib/"));
                        return !all_sources_are_libs
                    }
                }
                false // If metadata and sources are missing, this can't be the right contract.
            })
            .collect::<Vec<_>>()
    }

    fn foundry_profiles(config_file: &PathBuf) -> Result<Vec<String>, Box<dyn Error>> {
        let contents = fs::read_to_string(config_file).unwrap();
        let data = contents.parse::<toml::Value>();
        if data.is_err() {
            return Err("Unable to parse foundry.toml file".into())
        }

        let mut profiles = Vec::new();
        if let Some(profiles_table) =
            data.unwrap().as_table().unwrap().get("profile").and_then(|v| v.as_table())
        {
            for key in profiles_table.keys() {
                profiles.push(key.to_string());
            }
        }

        if !profiles.contains(&"default".to_string()) {
            profiles.push("default".to_string());
        }
        Ok(profiles)
    }
}

impl Framework for Foundry {
    // Return an instance of the framework if the path is a supported project.
    fn new(path: &Path) -> Result<Self, Box<dyn Error + Send + Sync>> {
        if !Self::is_supported(path) {
            return Err("Not a foundry project.".into())
        }
        Ok(Self { path: path.to_path_buf() })
    }

    // Verify this is a foundry project by looking for the presence of a `foundry.toml` file.
    fn is_supported(path: &Path) -> bool {
        let file = path.join("foundry.toml");
        file.exists() && file.is_file()
    }

    // TODO We currently only support forge projects and assume the user is using the default forge
    // directory structure of `src/`, `lib/`, and `out/`.
    fn build_commands(&self) -> Result<Vec<Command>, Box<dyn Error>> {
        let config_file = self.path.join("foundry.toml");
        let profile_names = Self::foundry_profiles(&config_file)?;
        println!("  Found profiles: {:?}", profile_names);

        let commands = profile_names
            .into_iter()
            .map(|profile_name| {
                let mut command = Command::new("forge");
                command
                    .arg("build")
                    .arg("--skip")
                    .arg("test")
                    .arg("script")
                    .arg("--build-info")
                    .arg("--build-info-path")
                    .arg("build_info")
                    .env("FOUNDRY_PROFILE", profile_name)
                    .env("FOUNDRY_BYTECODE_HASH", "none"); // TODO Account for bytecode hash later.
                command
            })
            .collect::<Vec<Command>>();
        Ok(commands)
    }

    fn get_artifacts(&self) -> Result<Vec<PathBuf>, Box<dyn Error>> {
        let mut artifacts = Vec::new();

        let out_dirs =
            WalkDir::new(&self.path).min_depth(1).max_depth(1).into_iter().filter_entry(|entry| {
                entry.file_type().is_dir()
                    && entry.file_name().to_string_lossy().to_lowercase().contains("out")
            });

        for entry in out_dirs.into_iter().filter_map(Result::ok) {
            if entry.path().is_dir() {
                for inner_entry in WalkDir::new(entry.path()).into_iter().filter_map(Result::ok) {
                    if inner_entry.file_type().is_file()
                        && inner_entry.path().extension().map_or(false, |ext| ext == "json")
                    {
                        artifacts.push(inner_entry.into_path());
                    }
                }
            }
        }

        Ok(Self::filter_artifacts(artifacts))
    }

    fn structure_found_creation_code(
        &self,
        artifact: &Path,
    ) -> Result<FoundCreationBytecode, Box<dyn Error>> {
        let metadata_settings = Self::get_artifact_metadata_settings(artifact)?;
        let raw_code = Self::get_artifact_creation_code(artifact)?;
        let bytecode_hash = metadata_settings.bytecode_hash.unwrap_or(BytecodeHash::None);
        let append_cbor = metadata_settings.cbor_metadata.unwrap_or(false);

        let (leading_code, metadata) = if bytecode_hash == BytecodeHash::None && !append_cbor {
            // If `bytecodeHash = none` AND `appendCBOR = false`, there is no metadata, so
            // everything we have is the leading code.
            (raw_code.clone(), MetadataInfo::default())
        } else {
            // If bytecodeHash != none OR appendCBOR = true, some metadata hash is present,
            // so we slice the bytes based on metadata length to get the leading code and metadata.
            let metadata = parse_metadata(&raw_code);
            let (leading_code, _) =
                raw_code.split_at(metadata.start_index.unwrap_or(raw_code.len()));
            (leading_code.to_vec().into(), metadata)
        };

        Ok(FoundCreationBytecode { raw_code, leading_code, metadata })
    }

    fn structure_expected_creation_code(
        &self,
        _artifact: &Path,
        found: &FoundCreationBytecode,
        expected: &Bytes,
    ) -> Result<ExpectedCreationBytecode, Box<dyn Error>> {
        if expected.len() < found.leading_code.len() {
            return Err("Expected creation bytecode is shorter than found creation bytecode.".into())
        }

        // Leading code is everything up until the found's metadata hash start index.
        let raw_code_len = found.raw_code.len();
        let leading_code: Bytes =
            expected.split_at(found.metadata.start_index.unwrap_or(raw_code_len)).0.to_vec().into();

        // Metadata hash is given by the found's metadata hash start and end indices, if they are
        // present, otherwise it's None.
        let metadata_hash: Option<Bytes> = if let (Some(start_index), Some(end_index)) =
            (found.metadata.start_index, found.metadata.end_index)
        {
            Some(expected[start_index..end_index].to_vec().into())
        } else {
            None
        };

        // The encoded constructor arguments are everything that's left.
        let accumulated_len =
            leading_code.len() + metadata_hash.as_ref().map_or(0, |hash| hash.len());
        let encoded_constructor_args: Option<Bytes> = if expected.len() > accumulated_len {
            // The remaining bytes are the encoded constructor arguments.
            Some(expected.split_at(accumulated_len).1.to_vec().into())
        } else {
            None
        };

        let metadata = MetadataInfo {
            hash: metadata_hash,
            start_index: found.metadata.start_index,
            end_index: found.metadata.end_index,
        };

        Ok(ExpectedCreationBytecode {
            raw_code: expected.clone(),
            leading_code,
            metadata,
            constructor_args: encoded_constructor_args,
        })
    }

    fn structure_found_deployed_code(
        &self,
        artifact: &Path,
    ) -> Result<FoundDeployedBytecode, Box<dyn Error>> {
        let metadata_settings = Self::get_artifact_metadata_settings(artifact)?;
        let (raw_code, immutable_references) = Self::get_artifact_deployed_code(artifact)?;
        let bytecode_hash = metadata_settings.bytecode_hash.unwrap_or(BytecodeHash::None);
        let append_cbor = metadata_settings.cbor_metadata.unwrap_or(false);

        let (leading_code, metadata) = if bytecode_hash == BytecodeHash::None && !append_cbor {
            // If `bytecodeHash = none` AND `appendCBOR = false`, there is no metadata, so
            // everything we have is the leading code.
            (raw_code.clone(), MetadataInfo::default())
        } else {
            // If bytecodeHash != none OR appendCBOR = true, some metadata hash is present,
            // so we slice the bytes based on metadata length to get the leading code and metadata.
            let metadata = parse_metadata(&raw_code);
            let (leading_code, _) =
                raw_code.split_at(metadata.start_index.unwrap_or(raw_code.len()));
            (leading_code.to_vec().into(), metadata)
        };

        Ok(FoundDeployedBytecode { raw_code, leading_code, metadata, immutable_references })
    }

    fn structure_expected_deployed_code(
        &self,
        _artifact: &Path, // todo remove these unused args.
        found: &FoundDeployedBytecode,
        expected: &Bytes,
    ) -> Result<ExpectedDeployedBytecode, Box<dyn Error>> {
        if expected.len() < found.leading_code.len() {
            return Err("Expected deployed bytecode is shorter than found deployed bytecode.".into())
        }

        // Leading code is everything up until the found's metadata hash start index.
        let raw_code_len = found.raw_code.len();
        let leading_code: Bytes =
            expected.split_at(found.metadata.start_index.unwrap_or(raw_code_len)).0.to_vec().into();

        // Metadata hash is given by the found's metadata hash start and end indices, if they are
        // present, otherwise it's None.
        let metadata_hash: Option<Bytes> = if let (Some(start_index), Some(end_index)) =
            (found.metadata.start_index, found.metadata.end_index)
        {
            Some(expected[start_index..end_index].to_vec().into())
        } else {
            None
        };

        let metadata = MetadataInfo {
            hash: metadata_hash,
            start_index: found.metadata.start_index,
            end_index: found.metadata.end_index,
        };

        Ok(ExpectedDeployedBytecode {
            raw_code: expected.clone(),
            leading_code,
            metadata,
            immutable_references: found.immutable_references.clone(),
        })
    }

    fn get_artifact_abi(artifact: &Path) -> Result<LosslessAbi, Box<dyn Error>> {
        let file_content = fs::read_to_string(artifact)?;
        let json_content: serde_json::Value = serde_json::from_str(&file_content)?;
        let abi_value = json_content
            .get("abi")
            .ok_or(format!("Missing 'bytecode' field in artifact JSON: {}", artifact.display()))?;
        Ok(serde_json::from_value(abi_value.clone())?)
    }

    fn get_artifact_creation_code(artifact: &Path) -> Result<Bytes, Box<dyn Error>> {
        let file_content = fs::read_to_string(artifact)?;
        let json_content: serde_json::Value = serde_json::from_str(&file_content)?;
        let creation_code_value = json_content
            .get("bytecode")
            .ok_or_else(|| {
                format!("Missing 'bytecode' field in artifact JSON: {}", artifact.display())
            })?
            .get("object")
            .ok_or_else(|| {
                format!("Missing 'object' field in bytecode JSON: {}", artifact.display())
            })?;
        let creation_code: Bytes = serde_json::from_value(creation_code_value.clone())?;
        Ok(creation_code)
    }

    fn get_artifact_deployed_code(
        artifact: &Path,
    ) -> Result<(Bytes, ImmutableReferences), Box<dyn Error>> {
        let file_content = fs::read_to_string(artifact)?;
        let artifact: ConfigurableContractArtifact = serde_json::from_str(&file_content)?;

        let deployed_code_object = artifact.deployed_bytecode.ok_or("No deployedBytecode found")?;
        let deployed_code =
            match deployed_code_object.bytecode.ok_or("No bytecode object found")?.object {
                BytecodeObject::Bytecode(bytes) => bytes,
                BytecodeObject::Unlinked(_) => {
                    return Err("Linked libraries not yet supported".into())
                }
            };
        let immutable_references = deployed_code_object.immutable_references;
        Ok((deployed_code, immutable_references))
    }

    fn get_artifact_metadata_settings(artifact: &Path) -> Result<SettingsMetadata, Box<dyn Error>> {
        let file_content = fs::read_to_string(artifact)?;
        let json_content: serde_json::Value = serde_json::from_str(&file_content)?;
        let settings_value = json_content
            .get("metadata")
            .ok_or_else(|| {
                format!("Missing 'metadata' field in artifact JSON: {}", artifact.display())
            })?
            .get("settings")
            .ok_or_else(|| {
                format!("Missing 'settings' field in metadata JSON: {}", artifact.display())
            })?;
        let settings_metadata: SettingsMetadata = serde_json::from_value(settings_value.clone())?;
        Ok(settings_metadata)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethers::solc::artifacts::{BytecodeHash, SettingsMetadata};
    use serde_json::json;
    use std::{error::Error, fs::File, io::Write, path::PathBuf, str::FromStr};
    use tempfile::NamedTempFile;

    // Helper function to create a temporary file with the given JSON content.
    fn create_test_artifact(
        artifact: &NamedTempFile,
        content: &serde_json::Value,
    ) -> Result<PathBuf, Box<dyn Error>> {
        let path = artifact.path().to_path_buf();
        let mut file = File::create(&path)?;
        let content_str = content.to_string();
        file.write_all(content_str.as_bytes())?;
        Ok(path)
    }

    #[test]
    fn structure_found_creation_code() -> Result<(), Box<dyn Error>> {
        struct TestCase {
            content: serde_json::Value,
            expected: FoundCreationBytecode,
        }

        let test_cases = vec![
            // Test case 1: BytecodeHash::None and appendCBOR = false
            TestCase {
                content: json!({
                    "bytecode": { "object": "0x1234" },
                    "metadata": { "settings": { "bytecodeHash": "none", "appendCBOR": false } },
                }),
                expected: FoundCreationBytecode {
                    raw_code: Bytes::from_str("0x1234")?,
                    leading_code: Bytes::from_str("0x1234")?,
                    metadata: MetadataInfo::default(),
                },
            },
            // Test case 2: BytecodeHash::Ipfs and appendCBOR = true
            TestCase {
                content: json!({
                    "bytecode": { "object": "0x1234567890abcdef0002" },
                    "metadata": { "settings": { "bytecodeHash": "ipfs", "appendCBOR": true } },
                }),
                expected: FoundCreationBytecode {
                    raw_code: Bytes::from_str("0x1234567890abcdef0002")?,
                    leading_code: Bytes::from_str("0x1234567890ab")?,
                    metadata: MetadataInfo {
                        hash: Some(Bytes::from_str("0xcdef0002")?),
                        start_index: Some(6),
                        end_index: Some(10),
                    },
                },
            },
        ];

        let foundry = Foundry { path: PathBuf::new() };
        let artifact_path = tempfile::NamedTempFile::new()?;
        for test_case in test_cases {
            let artifact = create_test_artifact(&artifact_path, &test_case.content)?;
            let result = foundry.structure_found_creation_code(&artifact)?;
            assert_eq!(result, test_case.expected);
        }

        Ok(())
    }

    #[test]
    fn structure_expected_creation_code() -> Result<(), Box<dyn Error>> {
        let foundry = Foundry { path: PathBuf::new() };

        // First test the case where expected code is too short to structure.
        struct FailureTestCase {
            description: String,
            found: FoundCreationBytecode,
            expected: Bytes,
        }

        let test_cases = vec![FailureTestCase {
            description: "Test case error 1: Expected code too short.".to_string(),
            found: FoundCreationBytecode {
                raw_code: Bytes::from_str("0x123456")?,
                leading_code: Bytes::from_str("0x123456")?,
                metadata: MetadataInfo::default(),
            },
            expected: Bytes::from_str("0x1234")?,
        }];

        for test_case in test_cases {
            let result = foundry.structure_expected_creation_code(
                &PathBuf::new(),
                &test_case.found,
                &test_case.expected,
            );
            assert!(result.is_err(), "{}", test_case.description);
        }

        // Now test success cases.
        struct TestCase {
            description: String,
            found: FoundCreationBytecode,
            expected: Bytes,
            expected_output: ExpectedCreationBytecode,
        }

        let test_cases = vec![
            TestCase {
                description: "Test case 1: No metadata, no constructor args.".to_string(),
                found: FoundCreationBytecode {
                    raw_code: Bytes::from_str("0x1234")?,
                    leading_code: Bytes::from_str("0x1234")?,
                    metadata: MetadataInfo::default(),
                },
                expected: Bytes::from_str("0x1234")?,
                expected_output: ExpectedCreationBytecode {
                    raw_code: Bytes::from_str("0x1234")?,
                    leading_code: Bytes::from_str("0x1234")?,
                    metadata: MetadataInfo::default(),
                    constructor_args: None,
                },
            },
            TestCase {
                description: "Test case 2: Same metadata hash, no constructor args.".to_string(),
                found: FoundCreationBytecode {
                    raw_code: Bytes::from_str("0x1234567890abcdef0002")?,
                    leading_code: Bytes::from_str("0x1234567890ab")?,
                    metadata: MetadataInfo {
                        hash: Some(Bytes::from_str("0xcdef0002")?),
                        start_index: Some(6),
                        end_index: Some(10),
                    },
                },
                expected: Bytes::from_str("0x1234567890abcdef0002")?,
                expected_output: ExpectedCreationBytecode {
                    raw_code: Bytes::from_str("0x1234567890abcdef0002")?,
                    leading_code: Bytes::from_str("0x1234567890ab")?,
                    metadata: MetadataInfo {
                        hash: Some(Bytes::from_str("0xcdef0002")?),
                        start_index: Some(6),
                        end_index: Some(10),
                    },
                    constructor_args: None,
                },
            },
            TestCase {
                description: "Test case 3: Different metadata hash, no constructor args."
                    .to_string(),
                found: FoundCreationBytecode {
                    raw_code: Bytes::from_str("0x1234567890abcdef0002")?,
                    leading_code: Bytes::from_str("0x1234567890ab")?,
                    metadata: MetadataInfo {
                        hash: Some(Bytes::from_str("0xcdef0002")?),
                        start_index: Some(6),
                        end_index: Some(10),
                    },
                },
                expected: Bytes::from_str("0x1234567890abffff0002")?,
                expected_output: ExpectedCreationBytecode {
                    raw_code: Bytes::from_str("0x1234567890abffff0002")?,
                    leading_code: Bytes::from_str("0x1234567890ab")?,
                    metadata: MetadataInfo {
                        hash: Some(Bytes::from_str("0xffff0002")?),
                        start_index: Some(6),
                        end_index: Some(10),
                    },
                    constructor_args: None,
                },
            },
            TestCase {
                description: "Test case 4: No metadata hash, constructor args.".to_string(),
                found: FoundCreationBytecode {
                    raw_code: Bytes::from_str("0x1234")?,
                    leading_code: Bytes::from_str("0x1234")?,
                    metadata: MetadataInfo::default(),
                },
                expected: Bytes::from_str("0x12345678")?,
                expected_output: ExpectedCreationBytecode {
                    raw_code: Bytes::from_str("0x12345678")?,
                    leading_code: Bytes::from_str("0x1234")?,
                    metadata: MetadataInfo::default(),
                    constructor_args: Some(Bytes::from_str("0x5678")?),
                },
            },
            TestCase {
                description: "Test case 5: Different metadata hash, constructor args.".to_string(),
                found: FoundCreationBytecode {
                    raw_code: Bytes::from_str("0x1234567890abcdef0002")?,
                    leading_code: Bytes::from_str("0x1234567890ab")?,
                    metadata: MetadataInfo {
                        hash: Some(Bytes::from_str("0xcdef0002")?),
                        start_index: Some(6),
                        end_index: Some(10),
                    },
                },
                expected: Bytes::from_str("0x1234567890abffff0002aaaaaa")?,
                expected_output: ExpectedCreationBytecode {
                    raw_code: Bytes::from_str("0x1234567890abffff0002aaaaaa")?,
                    leading_code: Bytes::from_str("0x1234567890ab")?,
                    metadata: MetadataInfo {
                        hash: Some(Bytes::from_str("0xffff0002")?),
                        start_index: Some(6),
                        end_index: Some(10),
                    },
                    constructor_args: Some(Bytes::from_str("0xaaaaaa")?),
                },
            },
        ];

        for test_case in test_cases {
            let result = foundry.structure_expected_creation_code(
                &PathBuf::new(),
                &test_case.found,
                &test_case.expected,
            )?;
            assert_eq!(result, test_case.expected_output, "{}", test_case.description);
        }

        Ok(())
    }

    #[test]
    fn get_artifact_abi() -> Result<(), Box<dyn Error>> {
        struct TestCase {
            content: serde_json::Value,
            expected_num_methods: usize,
            expected_constructor: bool,
        }

        #[rustfmt::skip]
        let test_cases = vec![
            // Test case 1: Counter contract with no constructor.
            TestCase {
                content: json!({ "abi": [ { "inputs": [], "name": "increment", "outputs": [], "stateMutability": "nonpayable", "type": "function" }, { "inputs": [], "name": "number", "outputs": [ { "internalType": "uint256", "name": "", "type": "uint256" } ], "stateMutability": "view", "type": "function" }, { "inputs": [ { "internalType": "uint256", "name": "newNumber", "type": "uint256" } ], "name": "setNumber", "outputs": [], "stateMutability": "nonpayable", "type": "function" } ] }),
                expected_num_methods: 3,
                expected_constructor: false,
            },
            // Test case 1: Counter contract with constructor.
            TestCase {
                content: json!({ "abi": [ { "inputs": [ { "internalType": "uint256", "name": "initialNumber", "type": "uint256" } ], "stateMutability": "nonpayable", "type": "constructor" }, { "inputs": [], "name": "increment", "outputs": [], "stateMutability": "nonpayable", "type": "function" }, { "inputs": [], "name": "number", "outputs": [ { "internalType": "uint256", "name": "", "type": "uint256" } ], "stateMutability": "view", "type": "function" }, { "inputs": [ { "internalType": "uint256", "name": "newNumber", "type": "uint256" } ], "name": "setNumber", "outputs": [], "stateMutability": "nonpayable", "type": "function" } ] }),
                expected_num_methods: 3,
                expected_constructor: true,
            },
        ];

        for test_case in test_cases {
            let artifact = NamedTempFile::new()?;
            let path = create_test_artifact(&artifact, &test_case.content)?;
            let abi = Foundry::get_artifact_abi(&path)?;
            assert_eq!(abi.abi.functions.len(), test_case.expected_num_methods);
            assert_eq!(abi.abi.constructor.is_some(), test_case.expected_constructor);
        }

        Ok(())
    }

    #[test]
    fn get_artifact_creation_code() -> Result<(), Box<dyn Error>> {
        struct TestCase {
            content: serde_json::Value,
            expected: Bytes,
        }

        let test_cases = vec![
            // Test case 1: Creation code is present.
            TestCase {
                content: json!({ "bytecode": { "object": "0x1234" }}),
                expected: Bytes::from_str("0x1234")?,
            },
            // Test case 2: Creation code is missing.
            TestCase {
                content: json!({ "bytecode": { "object": "" }}),
                expected: Bytes::from_str("")?,
            },
        ];

        for test_case in test_cases {
            let artifact = NamedTempFile::new()?;
            let path = create_test_artifact(&artifact, &test_case.content)?;
            let creation_code = Foundry::get_artifact_creation_code(&path)?;
            assert_eq!(creation_code, test_case.expected);
        }

        Ok(())
    }

    #[test]
    fn get_artifact_deployed_code() -> Result<(), Box<dyn Error>> {
        struct TestCase {
            content: serde_json::Value,
            expected: Bytes,
        }

        let test_cases = vec![
            // Test case 1: Deployed code is present.
            TestCase {
                content: json!({ "deployedBytecode": { "object": "0x1234" }}),
                expected: Bytes::from_str("0x1234")?,
            },
            // Test case 2: Deployed code is missing.
            TestCase {
                content: json!({ "deployedBytecode": { "object": "" }}),
                expected: Bytes::from_str("")?,
            },
        ];

        for test_case in test_cases {
            let artifact = NamedTempFile::new()?;
            let path = create_test_artifact(&artifact, &test_case.content)?;
            let (creation_code, _) = Foundry::get_artifact_deployed_code(&path)?;
            assert_eq!(creation_code, test_case.expected);
        }

        Ok(())
    }

    #[test]
    fn get_artifact_metadata_settings() -> Result<(), Box<dyn Error>> {
        struct TestCase {
            content: serde_json::Value,
            expected: SettingsMetadata,
        }

        let test_cases = vec![
            // Test case 1: Both `bytecodeHash` and `appendCBOR` fields are present.
            TestCase {
                content: json!({ "metadata": { "settings": { "bytecodeHash": "ipfs", "appendCBOR": true }}}),
                expected: SettingsMetadata {
                    use_literal_content: None,
                    bytecode_hash: Some(BytecodeHash::Ipfs),
                    cbor_metadata: Some(true),
                },
            },
            // Test case 2: both `bytecodeHash` and `appendCBOR` fields are missing
            TestCase {
                content: json!({ "metadata": { "settings": {} } }),
                expected: SettingsMetadata {
                    use_literal_content: None,
                    bytecode_hash: None,
                    cbor_metadata: None,
                },
            },
            // Test case 3: `bytecodeHash` field is present, `appendCBOR` field is missing
            TestCase {
                content: json!({ "metadata": { "settings": { "bytecodeHash": "bzzr1" } } }),
                expected: SettingsMetadata {
                    use_literal_content: None,
                    bytecode_hash: Some(BytecodeHash::Bzzr1),
                    cbor_metadata: None,
                },
            },
            // Test case 4: `bytecodeHash` field is missing, `appendCBOR` field is present
            TestCase {
                content: json!({ "metadata": { "settings": { "appendCBOR": false } } }),
                expected: SettingsMetadata {
                    use_literal_content: None,
                    bytecode_hash: None,
                    cbor_metadata: Some(false),
                },
            },
        ];

        let artifact_path = tempfile::NamedTempFile::new()?;
        for test_case in test_cases {
            let artifact = create_test_artifact(&artifact_path, &test_case.content)?;
            let result = Foundry::get_artifact_metadata_settings(&artifact)?;
            assert_eq!(result, test_case.expected);
        }

        Ok(())
    }
}
