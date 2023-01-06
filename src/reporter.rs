use crate::aws::LiveKey;
use crate::scanners::ScannerMatch;
use crate::sources::{PackageToProcess, SourceType};
use anyhow::Result;
use itertools::Itertools;
use serde::Serialize;
use std::fs;

use tinytemplate::TinyTemplate;

#[derive(Serialize)]
struct TemplateContext {
    package: PackageToProcess,
    findings: Vec<Finding>,
}

#[derive(Serialize)]
struct Finding {
    line_number: usize,
    file_path: String,
    role_name: String,
    access_key: String,
    secret_key: String,
    public_url: Option<String>,
}

fn url_for_finding(package: &PackageToProcess, scanner_match: &ScannerMatch) -> Option<String> {
    match package.source {
        SourceType::PyPi => {
            let public_path = format!(
                "https://inspector.pypi.io/project/{}/{}/{}/{}#line.{}",
                package.name,
                package.version,
                package.download_url.path().strip_prefix('/').unwrap(),
                scanner_match.relative_path(),
                scanner_match.rg_match.line_number
            );
            Some(public_path)
        }
        _ => None,
    }
}

pub fn create_findings(items: Vec<LiveKey>) -> Result<()> {
    let mut template = TinyTemplate::new();
    template
        .add_template("markdown", include_str!("template.md"))
        .unwrap();

    // A single package may contain multiple keys. We ideally want a single file per release file,
    // So we need to sort and group the keys.

    let sorted_keys = items.into_iter().sorted().group_by(|v| v.clone());

    for (k, v) in sorted_keys.into_iter() {
        println!("Live Key: {:?}", k.ordering_tuple());
        let ctx = TemplateContext {
            package: k.scanner_match.downloaded_package.package.clone(),
            findings: v
                .into_iter()
                .map(|v| Finding {
                    public_url: url_for_finding(
                        &v.scanner_match.downloaded_package.package,
                        &v.scanner_match,
                    ),
                    line_number: v.scanner_match.rg_match.line_number,
                    file_path: v.scanner_match.relative_path(),
                    role_name: v.role_name,
                    access_key: v.scanner_match.access_key,
                    secret_key: v.scanner_match.secret_key,
                })
                .collect(),
        };
        let rendered = template.render("markdown", &ctx)?;

        let report_path = k
            .scanner_match
            .downloaded_package
            .package
            .source
            .report_path();

        let output_dir = report_path.join(&k.scanner_match.downloaded_package.package.name);
        let output_path = output_dir.join(format!(
            "{}.md",
            k.scanner_match.downloaded_package.package.file_name()
        ));
        let _ = fs::create_dir_all(output_dir);
        fs::write(&output_path, rendered).unwrap();
        println!("Created file {output_path:?}");
    }
    Ok(())
}
