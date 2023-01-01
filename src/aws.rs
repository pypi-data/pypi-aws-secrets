use crate::scanners::ScannerMatch;
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct LiveKey {
    scanner_match: ScannerMatch,
    role_name: String,
}

pub fn check_aws_keys(matches: Vec<ScannerMatch>) -> Result<Vec<LiveKey>> {
    // Aws SDK is all async. Bit annoying.
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let checker = runtime.spawn(async {
        let mut valid_keys = vec![];
        println!("Trying keys...");
        for scanner_match in matches {
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
                    eprintln!("sts error: {:?}", e);
                    continue;
                }
            }
        }
        valid_keys
    });
    Ok(runtime.block_on(checker)?)
}
