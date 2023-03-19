# GCP Token found published to {package.source}

* Package Name: {package.name}
* Package Version: {package.version}
* Public URL to package: [{package.download_url}]({package.download_url})

## Key Details
{{ for key in findings }}
### `{key.access_key}`

* AWS Access Key ID: `{key.access_key}`
* AWS Secret Access Key: `{key.secret_key}` 
* AWS role name: `{key.role_name}`
* File in package: `{key.file_path}`
* Line number: `{key.line_number}`
{{if key.public_url}}
* Public URL to key: {key.public_url}
{{endif}}

{{ endfor }}