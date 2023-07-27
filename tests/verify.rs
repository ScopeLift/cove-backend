use serde_json::json;
mod common;
use cove::routes::verify::SuccessfulVerification;
use serde_json::from_str;

#[tokio::test]
async fn verify_counters() -> Result<(), Box<dyn std::error::Error>> {
    let app = common::spawn_app().await;
    let client = reqwest::Client::new();

    // Define constants for all test cases.
    let repo_url = "https://github.com/ScopeLift/cove-test-repo";
    let repo_commit = "b268862cf1ccf495d6dc20a86c41940dfb386d9b";
    let framework = "foundry";

    // Define test-case specific inputs.
    struct TestCase {
        build_hint: String,
        contract_address: String,
        creation_tx_hashes: serde_json::Value,
    }

    // Test cases are from https://github.com/ScopeLift/cove-test-repo/blob/b1cbf52e77fe63351a267652d4df06f2e5b15952/deploys.txt
    let test_cases = vec![
        // -------- Profile: default --------
        // CounterBasic, create.
        TestCase {
            build_hint: "default".to_string(),
            contract_address: "0x8d56e3e001132d84488DbacDbB01AfB8C3171242".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0x59724cfbee93a0c10f7cbd312c1d159d62ea602003dd61a407a5cf842b4103d6",
                "sepolia": "0xf9899c9d982e7a7d074f6792c3689b1c0a25d14eaa9f065ce31bfa4ea59607b2",
            }),
        },
        // CounterBasic, create2.
        TestCase {
            build_hint: "default".to_string(),
            contract_address: "0x23B2b2134E9609E8e673e5578939ABd25263cf70".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0x5e6c78de030b85005d42fad3cdd3dfe153a2311ba66c10a4a576fec0b354d5dd",
                "sepolia": "0x998cd961293f1d27beff1a7f1ef4992de8fc5aeeb44b77547765a18c18cc16da",
            }),
        },
        // CounterWithImmutables, create.
        TestCase {
            build_hint: "default".to_string(),
            contract_address: "0xB264f440D77528320c74E215d5d18885060813fC".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0x99456f8b0f785dee828ca9c1ec76a25b758b728fd8b63d1e6cbf6329ed3faf2b",
                "sepolia": "0x77f13d8f7df7da31fabc485c2a2c7a58e9c39636e5877cb2e18df38e5986e56e",
            }),
        },
        // CounterWithImmutables, create2.
        TestCase {
            build_hint: "default".to_string(),
            contract_address: "0x296fae36116b8723BeB61aA59bc7156a098eC97e".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0x580287391681c6f5d44e139b3d55dba5b3cebeec62faef0bb5ecaec851772309",
                "sepolia": "0xe267e478674559adccb2198ffc598a5f9139f689841919904a9d423f51d1fb08",
            }),
        },
        // -------- Profile: no_optimizer_no_metadata --------
        // CounterBasic, create.
        TestCase {
            build_hint: "no_optimizer_no_metadata".to_string(),
            contract_address: "0x1F6891359Ac22adc47f8665F43dDcF4b6Ba107e9".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0xca8b3d45156e889ae1bd8aafe7aed5f70a6669d36490ac8faf195e06d65ebe03",
                "sepolia": "0xc43bb699bb31b7a5ff8e0af46a24999be30af10d45e21050649b847674d29b69",
            }),
        },
        // CounterBasic, create2.
        TestCase {
            build_hint: "no_optimizer_no_metadata".to_string(),
            contract_address: "0xeF1B513742c5A8129C8F8F0D69d8d16743E6612c".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0x9e70e4d5e0e2fa638df82b5e3832c96f58dcc73d640e94cea961fea7ecc37d55",
                "sepolia": "0x434d3d7ff2dbe8c2b3d96a8a842fcee85629a0ddbeaed271a7dbf1c3a182c08e",
            }),
        },
        // CounterWithImmutables, create.
        TestCase {
            build_hint: "no_optimizer_no_metadata".to_string(),
            contract_address: "0xEa594E57b5dE09A50AF91e7C627f7fC53951ead0".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0x0cc452e08881a9a58d7bbe4c014652a94bcf88b31c143cc4428431663177073a",
                "sepolia": "0x1191d36d56499747b8e87345a96a91f883c01bc2dd39df7949fe1b262ec406d3",
            }),
        },
        // CounterWithImmutables, create2.
        TestCase {
            build_hint: "no_optimizer_no_metadata".to_string(),
            contract_address: "0x99c73A95DA89EE6801FEb6d45B9aeD35deEd793F".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0x24fd152137d9d6522f9e0a2006d0d6171fed9a05b10c201952ebc9a72d185ce2",
                "sepolia": "0x4fe19aee59f1e0cf0498d995305ce329d9fc98b026d582aab721c2e0fe568e1a",
            }),
        },
        // -------- Profile: yes_optimizer_no_metadata --------
        // CounterBasic, create.
        TestCase {
            build_hint: "yes_optimizer_no_metadata".to_string(),
            contract_address: "0xcBAE200a36d3E5bB678d42B656af3ce932bb0aae".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0x82f8f49bb78d6ba8c74ffb8fafe26923d105bcfa4c06c25de591ca0441ec8b7a",
                "sepolia": "0xdd5d6520db700b634abd29bea9833b452f8e7e7ae6b46951b9d42b6791715672",
            }),
        },
        // CounterBasic, create2.
        TestCase {
            build_hint: "yes_optimizer_no_metadata".to_string(),
            contract_address: "0xE0128A75D6B9306bCdae38eD27956394F27C4e75".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0x3f69a0c0ecc3c5395ca39bd3368c2bce22e3b77480ab25abbd2ec217ef19fd7a",
                "sepolia": "0xaa543c61dae5798e00b1ffd10ae956e496065a2beeee6928e366ae131152c99a",
            }),
        },
        // CounterWithImmutables, create.
        TestCase {
            build_hint: "yes_optimizer_no_metadata".to_string(),
            contract_address: "0xb876A8D941a3850f1d4382F2F318717f0C940059".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0xe1bf93dec9daa69afee5d882d1667bfd979626eb62b03a2da46c89d4c60cbf4e",
                "sepolia": "0xd3ef8c6c6cd50c04245296431dc32f1f4c20039a27abe1d6338c80fee5c01853",
            }),
        },
        // CounterWithImmutables, create2.
        TestCase {
            build_hint: "yes_optimizer_no_metadata".to_string(),
            contract_address: "0x23EFeed2d1B6F397C50111AAA481B99CCA1Ac2dd".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0x9a8ce78a6a2ed44acdac29b7c39c66d96810f4135099f5503e119a3ef99cc374",
                "sepolia": "0xb1d15edd8fae46d20bc36c687bc59fbf9385081002c5da326d946097d06a7d57",
            }),
        },
        // -------- Profile: yes_optimizer_yes_metadata --------
        // CounterBasic, create.
        TestCase {
            build_hint: "yes_optimizer_yes_metadata".to_string(),
            contract_address: "0xB67ffD4D913629cAD57BDcCD489BE8e4b69e6542".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0x4f00e380c29083c46e3a4358a69829782fd93afc0301e842b0633a83762542eb",
                "sepolia": "0xfeca3266d47d9220b60ce16426f6507af3eb44cf3c980609e7f6899733591a6f",
            }),
        },
        // CounterBasic, create2.
        TestCase {
            build_hint: "yes_optimizer_yes_metadata".to_string(),
            contract_address: "0xDb1d70fe451A7a38708953d2846e3471194a8bCF".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0xd8249d92f1992e38010fc546c09197781b6b61d92b2eceb065f118f932ecbcfe",
                "sepolia": "0xf8532961f5da5c144549ee0a2813604d55f90047a7be7750f72f4ac67d5a10b7",
            }),
        },
        // CounterWithImmutables, create.
        TestCase {
            build_hint: "yes_optimizer_yes_metadata".to_string(),
            contract_address: "0x273A1E5b5A974B9f13A90fAf1aE16ed093031e25".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0x48a2af4d480096381c4f028ce123d540a5110f58c4c4fe9632e803c4140e90ba",
                "sepolia": "0xdacd1e2c1374b60cb916efb6f080a25050ef298866c520968b3aad6ee1b233d0",
            }),
        },
        // CounterWithImmutables, create2.
        TestCase {
            build_hint: "yes_optimizer_yes_metadata".to_string(),
            contract_address: "0x2c3c10Dcc5878630F179840A6c9bfD9D51E4f7e8".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0x66418c91590189cf48d8a33fcfb7e48c864b8852e33fd430338e38e4cbbd6a47",
                "sepolia": "0x2100422ea260e5a0a2b61ff708c3de04fd06309cdf794691c97f94d473665b75",
            }),
        },
        // -------- Profile: no_via_ir_no_metadata --------
        // CounterBasic, create.
        TestCase {
            build_hint: "no_via_ir_no_metadata".to_string(),
            contract_address: "0x2c958458d7F02C77CaeEb17Accc077B399937674".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0xd69337ce48a840f52ca7cc1937114c90716df4a89e4c75a86e49b59a6f8f92e5",
                "sepolia": "0x2697725e213a2e4259131529c488b3b2cb26947b89fd29fc68619b03f52638ae",
            }),
        },
        // CounterBasic, create2.
        TestCase {
            build_hint: "no_via_ir_no_metadata".to_string(),
            contract_address: "0x73a43c10e3B8B31FAae7bad82873E292000eeB5a".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0xc710fc96b773ba281fb7ef374b6e55c816136a96627dc372a125779b5eec7047",
                "sepolia": "0xfde24cfdc57f12ea729d918dc553538f5e81e636e847ed9da28bd22f85be116e",
            }),
        },
        // CounterWithImmutables, create.
        TestCase {
            build_hint: "no_via_ir_no_metadata".to_string(),
            contract_address: "0xFA571F462AF6eaBD434843571B2EcF4d3CF7E238".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0xf17d1f8b5ed73938671246b6a9e492777927272316b695336a38dd572d69d4e7",
                "sepolia": "0xe35ee9ab2c46dd186a62125c1593980bdb494ec6f55f7a1e83349e3c58c30558",
            }),
        },
        // CounterWithImmutables, create2.
        TestCase {
            build_hint: "no_via_ir_no_metadata".to_string(),
            contract_address: "0x3b1e1df3D0a3e69847F34a36F2Eb5c87b2aFB669".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0x9e8713310f91ded8fe06289bdf9afe418c12ee3282783b0a2b5cc39e5c7c0f4b",
                "sepolia": "0x864e876e42776cf585c69de06905d6ebf3eb502566c5064400d902d1cd5378e1",
            }),
        },
        // -------- Profile: yes_via_ir_no_metadata --------
        // CounterBasic, create.
        TestCase {
            build_hint: "yes_via_ir_no_metadata".to_string(),
            contract_address: "0x36c045f78e5BC314EaB908C0f67fdc40FA999CFF".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0x9e8713310f91ded8fe06289bdf9afe418c12ee3282783b0a2b5cc39e5c7c0f4b",
                "sepolia": "0x450e133ec2140020d0e370b67d2bc4acadf099cf496eb41a0a151bd04e716bcf",
            }),
        },
        // CounterBasic, create2.
        TestCase {
            build_hint: "yes_via_ir_no_metadata".to_string(),
            contract_address: "0xAd5523Fd7f4cbe8dd304146df7da30795e97994A".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0xbc4ce0957b90d7034bb0cef2836a6fac67156b1df2c2daab1620f7995a0a42ff",
                "sepolia": "0x09897cbdcf221c835e48b90aa3fb5d73f50d96989bc5c411196d12a30db6e78f",
            }),
        },
        // CounterWithImmutables, create.
        TestCase {
            build_hint: "yes_via_ir_no_metadata".to_string(),
            contract_address: "0x9EF024a53916E21943A04c9d5b502C6CC2F9B75E".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0xbd01ff3a979b1a14cb76192cc79e0987061860250ebb926e22b892e791ab763e",
                "sepolia": "0x85a5f8cc1aff64617269f659c52f8c796cbac19701bc04243c7345d36254d93d",
            }),
        },
        // CounterWithImmutables, create2.
        TestCase {
            build_hint: "yes_via_ir_no_metadata".to_string(),
            contract_address: "0xa40dADB10aB7DA93fC0B212c14A08011066B47E9".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0xf86d135d5a90392cde1646fc35c827cd70d6b8ed989a7f01150132c10c2fadec",
                "sepolia": "0x1ceb6d571b751a1b46842d9cb8fafd14b06a234c6fbfbb9c1b161ba4e751ea98",
            }),
        },
        // -------- Profile: yes_via_ir_yes_metadata --------
        // CounterBasic, create.
        TestCase {
            build_hint: "yes_via_ir_yes_metadata".to_string(),
            contract_address: "0x20aFd7eFBF41b26e461801FB3723e9695bF342d7".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0xd0dd3651ff0a36c24d9cb01dea5095409f1d6f434a3d1add0f29526cc9223e41",
                "sepolia": "0x4370d63a467279b26905aab91879ae43a79a0f325078be013c4b271711ac6db1",
            }),
        },
        // CounterBasic, create2.
        TestCase {
            build_hint: "yes_via_ir_yes_metadata".to_string(),
            contract_address: "0xaE5F5A0F8ad2F733c37294c2e7C7C835912fEB02".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0xdfc159bca0996637058435bed76bb8b5a575435dd433b7bff2aa70835f4274c3",
                "sepolia": "0xe863df7d15cbc64e11523281b5b5cb2d98642b8599a77253410f39812f8208e3",
            }),
        },
        // CounterWithImmutables, create.
        TestCase {
            build_hint: "yes_via_ir_yes_metadata".to_string(),
            contract_address: "0x8a5BB7A78c45E6780f66fA46aeEfa9942455EBE6".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0x78d2b4e455f196e3d490a861dc4563bbe64e976bf5a0fdf6dd60135e2c7177cf",
                "sepolia": "0xb2fb4ea3c20f975610ac59bc2541f8fbc8862be92c2104085e3ee81a9603d4ce",
            }),
        },
        // CounterWithImmutables, create2.
        TestCase {
            build_hint: "yes_via_ir_yes_metadata".to_string(),
            contract_address: "0x5E208884701eb020A4E7C797914FEa769c6B966E".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0xeb48f96848873838924b5437e7dd9280096bdefc545d48c0e96a02dad0036450",
                "sepolia": "0x59bcd1cb0895d702b128d2df2d7d9aed6e77679a4a818470d59fb128783e469d",
            }),
        },
        // -------- Profile: no_metadata_no_cbor --------
        // CounterBasic, create.
        TestCase {
            build_hint: "no_metadata_no_cbor".to_string(),
            contract_address: "0xF3352807F6F713397DC92D7Be7Cd4EC0DfEc2dCd".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0x711235085e5ecd0a203cf8a9322f7ef7ad9bbac6403af3b459b11bccacb1b19e",
                "sepolia": "0x86a9d38cd8ce9b4ebb92af5ca0bde7a43aacfb21129859823eafc36884458622",
            }),
        },
        // CounterBasic, create2.
        TestCase {
            build_hint: "no_metadata_no_cbor".to_string(),
            contract_address: "0x6EDF04b2C90E149144f47DF81D116236b2F8f28c".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0x103676bae9cc886ddc4cf2b14e30802e05f32acda5de9a4e5770f48491e135ff",
                "sepolia": "0xc952578fa89c13535858e4db41398b88680be9fd9c62a8f66ead64e574b7c86f",
            }),
        },
        // CounterWithImmutables, create.
        TestCase {
            build_hint: "no_metadata_no_cbor".to_string(),
            contract_address: "0xDD816E9B0dE20176C93f1FC135422101bc001748".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0x2ed87f43718628e12617b7a34fe1dac219ed1cca5ce258029004c16332e0d8f8",
                "sepolia": "0x47a4387413316c50c4984fba1637fc66445263a511d5ef85913bcd9f99ff39bf",
            }),
        },
        // CounterWithImmutables, create2.
        TestCase {
            build_hint: "no_metadata_no_cbor".to_string(),
            contract_address: "0x442b2Cd507B1F5621B631778D739C9C322F3243F".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0x8c04852c0db3ca8dca9bca646177094195b189b6da2416ce0e9af670e36faacd",
                "sepolia": "0x968a506284c9e3432c072e1f614657ef597166435d5a84340965263d524b50eb",
            }),
        },
        // -------- Profile: no_metadata_yes_cbor --------
        // CounterBasic, create.
        TestCase {
            build_hint: "no_metadata_yes_cbor".to_string(),
            contract_address: "0xe402617CBe2e1f2c2F2714c3163a6fDFEfd4312c".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0x8317876aadf5ab3c4fbbb619b7b7d09b204371a6c37aee917e2cf3cca0b26a0d",
                "sepolia": "0x91483d86dddb5b7a64ad7e4e73f668894b32cdb0f054d4c40ccb730b754f08df",
            }),
        },
        // CounterBasic, create2.
        TestCase {
            build_hint: "no_metadata_yes_cbor".to_string(),
            contract_address: "0xB077A78Ad98291046f7abF15562793F2A00c90F7".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0x6f67aa4902a2626526010bcb5604767c0e101dbf49521871134f5eb351d71376",
                "sepolia": "0x44fdf98574cc59373e80fadbad5913ed84e31267887f769a60f9352b91749f4c",
            }),
        },
        // CounterWithImmutables, create.
        TestCase {
            build_hint: "no_metadata_yes_cbor".to_string(),
            contract_address: "0xA6e889356B499FA1F58CCfB002B90A6B5dCce10b".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0xfb8307aaf8cc4245a137405b651f8de168f53e4ec7ec0306d804bf3c31606a14",
                "sepolia": "0xf96e130fb754b264698a2c0d77066b128e4caf14e873e81660288ad8dfaa42a0",
            }),
        },
        // CounterWithImmutables, create2.
        TestCase {
            build_hint: "no_metadata_yes_cbor".to_string(),
            contract_address: "0x0cE9fc5dBf779F5d0C743B85e7d9897066aF35e3".to_string(),
            creation_tx_hashes: json!({
                "goerli": "0x25b348820a7177e2d9a354eaa795958cc48ae7909179c2b6ac10e2702343e0d7",
                "sepolia": "0xcfb1a3423b8b44957580b5fbd4f8d70689359a777a2b229af435948383407728",
            }),
        },
    ];

    for (i, test_case) in test_cases.into_iter().enumerate() {
        let build_config = json!({
            "framework": framework,
            "buildHint": test_case.build_hint
        });
        let body = json!({
            "repoUrl": repo_url,
            "repoCommit": repo_commit,
            "contractAddress": test_case.contract_address,
            "buildConfig": build_config,
            "creationTxHashes": Some(test_case.creation_tx_hashes),
        });

        let response = client
            .post(&format!("{}/verify", app.address))
            .header("Content-Type", "application/json")
            .body(body.to_string())
            .send()
            .await?;

        // Assertions.
        // let response_body = response.text().await?;
        // println!("response_body {:?}", response_body);
        assert_eq!(200, response.status().as_u16(), "Test case {i} failed");
    }

    Ok(())
}

#[tokio::test]
#[ignore = "This fails because leading bytecode differs in two places. This did not used to happen, TBD what broke here. It's worth noting that Seaport actually uses Hardhat for the production build, which may be related (it used to be the same bytecode aside from the metadata hash, though)"]
async fn verify_seaport() -> Result<(), Box<dyn std::error::Error>> {
    run_integration_test(
        "https://github.com/ProjectOpenSea/seaport",
        "d58a91d218b0ab557543c8a292710aa36e693973",
        "0x00000000000001ad428e4906aE43D8F9852d0dD6",
        json!({
            "framework": "foundry",
            "buildHint": "optimized"
        }),
        json!({
            "mainnet": "0x4f5eae3d221fe4a572d722a57c2fbfd252139e7580b7959d93eb2a8b05b666f6",
            "polygon": "0x7c0a769c469d24859cbcb978caacd9b6d5eea1f50ae6c1b3c94d4819375e0b09",
            "optimism": "0x3a46979922e781895fae9cba54df645b813eb55447703f590d51af1993ad59d4",
            "arbitrum": "0xa150f5c8bf8b8a0fc5f4f64594d09d796476974280e566fe3899b56517cd11da",
            "gnosis_chain": "0xfc189820c60536e2ce90443ac3d39633583cfed6653d5f7edd7c0e115fd2a18b",
        }),
    )
    .await
}

#[tokio::test]
async fn verify_gitcoin_governor_alpha() -> Result<(), Box<dyn std::error::Error>> {
    run_integration_test(
        "https://github.com/gitcoinco/Alpha-Governor-Upgrade",
        "17f7717eec0604505da2faf3f65516a8619063a0",
        "0x1a84384e1f1b12D53E60C8C528178dC87767b488",
        json!({
            "framework": "foundry",
            "buildHint": "default"
        }),
        json!({
            "mainnet": "0x61d669c6c0b976637b8f4528b99b170f060227b2bc20892743f22c6a34c84e23"
        }),
    )
    .await
}

async fn run_integration_test(
    repo_url: &str,
    repo_commit: &str,
    contract_address: &str,
    build_config: serde_json::Value,
    creation_tx_hashes: serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let app = common::spawn_app().await;
    let client = reqwest::Client::new();

    // POST request inputs.
    let body = json!({
        "repoUrl": repo_url,
        "repoCommit": repo_commit,
        "contractAddress": contract_address,
        "buildConfig": build_config,
        "creationTxHashes": Some(creation_tx_hashes),
    });

    // Send request.
    let response = client
        .post(&format!("{}/verify", app.address))
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .await?;

    assert_eq!(200, response.status().as_u16());

    let response_body = response.text().await?;
    let verification_result: SuccessfulVerification =
        from_str(&response_body).expect("Failed to deserialize SuccessfulVerification");
    assert_eq!(repo_url, verification_result.repo_url);
    assert_eq!(repo_commit, verification_result.repo_commit);
    Ok(())
}
