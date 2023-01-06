use crate::scanners::ScannerMatch;
use crate::sources::SourceType;
use anyhow::Result;
use std::cmp::Ordering;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct LiveKey {
    pub scanner_match: ScannerMatch,
    pub role_name: String,
}

impl Eq for LiveKey {}

impl PartialEq for LiveKey {
    fn eq(&self, other: &Self) -> bool {
        self.scanner_match == other.scanner_match
    }
}

impl LiveKey {
    pub fn ordering_tuple(&self) -> (&SourceType, &String, &String, &String, &PathBuf, &usize) {
        (
            &self.scanner_match.downloaded_package.package.source,
            &self.scanner_match.downloaded_package.package.name,
            &self.scanner_match.downloaded_package.package.version,
            &self.scanner_match.access_key,
            &self.scanner_match.rg_match.path,
            &self.scanner_match.rg_match.line_number,
        )
    }
}

impl Ord for LiveKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.ordering_tuple().cmp(&other.ordering_tuple())
    }
}

impl PartialOrd for LiveKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.ordering_tuple().partial_cmp(&other.ordering_tuple())
    }
}

pub fn check_aws_keys(matches: Vec<ScannerMatch>) -> Result<Vec<LiveKey>> {
    // Aws SDK is all async. Bit annoying.
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let checker = runtime.spawn(async {
        let mut valid_keys = vec![];
        println!("Trying keys...");
        for scanner_match in matches {
            println!("Key: {}", &scanner_match.access_key);
            println!("Sec: {}", &scanner_match.secret_key);
            std::env::set_var("AWS_ACCESS_KEY_ID", &scanner_match.access_key);
            std::env::set_var("AWS_SECRET_ACCESS_KEY", &scanner_match.secret_key);
            std::env::set_var("AWS_DEFAULT_REGION", "us-east-1");
            let config = aws_config::load_from_env().await;
            let client = aws_sdk_sts::Client::new(&config);
            match client.get_caller_identity().send().await {
                Ok(identity) => {
                    let arn = identity.arn().unwrap();
                    let identity_without_account = arn.split(':').last().unwrap();
                    valid_keys.push(LiveKey {
                        scanner_match,
                        role_name: identity_without_account.to_string(),
                    });
                }
                Err(e) => {
                    eprintln!("sts error: {e:?}");
                    continue;
                }
            }
        }
        valid_keys
    });
    Ok(runtime.block_on(checker)?)
}
