// use std::fs;
// use std::path::PathBuf;
//
// fn create_findings(items: Vec<LiveKey>) {
//     let mut template = TinyTemplate::new();
//     template
//         .add_template("markdown", include_str!("template.md"))
//         .unwrap();
//
//     // A single package may contain multiple keys. We ideally want a single file per release file,
//     // So we need to sort and group the keys.
//
//     let sorted_keys = items
//         .into_iter()
//         .sorted_by_key(|v| v.key.pypi_file.filename.clone())
//         .group_by(|v| v.key.pypi_file.clone());
//
//     for (project_file, keys) in &sorted_keys {
//         #[derive(Serialize)]
//         struct TemplateContext {
//             project_file: ProjectFile,
//             first_key: FoundKey,
//             keys: Vec<LiveKey>,
//         }
//
//         let keys: Vec<_> = keys
//             .into_iter()
//             .unique_by(|v| (v.key.access_key.clone(), v.key.secret_key.clone()))
//             .collect();
//         let first_key = keys.first().unwrap().key.clone();
//
//         let ctx = TemplateContext {
//             project_file,
//             first_key: first_key.clone(),
//             keys,
//         };
//
//         let rendered = template.render("markdown", &ctx).unwrap();
//         let output_dir = PathBuf::from(format!("keys/{}/", first_key.name));
//         let output_path = output_dir.join(format!("{}.md", first_key.pypi_file.filename));
//         let _ = fs::create_dir_all(output_dir);
//         fs::write(&output_path, rendered).unwrap();
//         println!("Created file {:?}", output_path);
//     }
// }
