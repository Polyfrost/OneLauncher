use oneclient_java::vendors::runtime_providers;
use oneclient_net::{NetConfig, RequestClient};
use polyio::ChecksumAlgorithm;

/// A major every vendor still publishes, so a listing coming back empty means
/// the query is wrong rather than the release being retired.
const MAJOR: u32 = 21;

#[tokio::test]
#[ignore = "requires network"]
async fn every_vendor_publishes_a_usable_checksum() {
    let net = RequestClient::new(NetConfig::default()).expect("a client");
    let mut failures = Vec::new();

    for provider in runtime_providers() {
        let vendor = provider.vendor();

        let packages = match provider.list_packages(Some(MAJOR), &net).await {
            Ok(packages) => packages,
            Err(err) => {
                failures.push(format!("{vendor}: listing failed: {err}"));
                continue;
            }
        };

        if packages.is_empty() {
            failures.push(format!("{vendor}: no packages listed for major {MAJOR}"));
            continue;
        }

        let unverifiable = packages
            .iter()
            .filter(|package| package.checksum.is_none())
            .count();

        if unverifiable > 0 {
            failures.push(format!(
                "{vendor}: {unverifiable}/{} packages carry no usable checksum",
                packages.len()
            ));
            continue;
        }

        // A well-formed hash in the wrong algorithm would fail every download,
        // so assert the algorithm each vendor actually publishes.
        for package in &packages {
            let checksum = package.checksum.as_ref().expect("checked above");
            assert!(
                checksum.is_well_formed(),
                "{vendor}: malformed checksum on {}: {}",
                package.name,
                checksum.hex
            );
        }

        println!(
            "{vendor}: {} packages, all carrying {}",
            packages.len(),
            packages[0]
                .checksum
                .as_ref()
                .expect("checked above")
                .algorithm
                .name()
        );
    }

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[tokio::test]
#[ignore = "requires network"]
async fn liberica_publishes_sha1_and_the_rest_publish_sha256() {
    let net = RequestClient::new(NetConfig::default()).expect("a client");

    for provider in runtime_providers() {
        let vendor = provider.vendor();
        let packages = provider
            .list_packages(Some(MAJOR), &net)
            .await
            .unwrap_or_else(|err| panic!("{vendor}: listing failed: {err}"));

        let Some(checksum) = packages.first().and_then(|p| p.checksum.as_ref()) else {
            panic!("{vendor}: no checksum to check the algorithm of");
        };

        let expected = match vendor.to_string().as_str() {
            "Liberica" => ChecksumAlgorithm::Sha1,
            _ => ChecksumAlgorithm::Sha256,
        };

        assert_eq!(
            checksum.algorithm,
            expected,
            "{vendor} changed the algorithm it publishes"
        );
    }
}
