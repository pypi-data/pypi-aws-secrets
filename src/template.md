# AWS keys found published to PyPi

* Package Name: {first_key.name}
* Package Version: {first_key.version}
* Upload date: {project_file.upload_time_iso_8601}
* PyPi release URL: [{project_file.filename}]({project_file.url})

## Key Details
{{ for key in keys }}
### `{key.key.access_key}`

Public URL to key material: [{key.key.public_path}]({key.key.public_path})

* AWS Access Key ID: `{key.key.access_key}`
* AWS Secret Access Key: `{key.key.secret_key}` 
* AWS role name: `{key.role_name}`
{{ endfor }}