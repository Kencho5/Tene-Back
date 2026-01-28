use aws_sdk_s3 as s3;
use aws_sdk_s3::presigning::PresigningConfig;
use std::time::Duration;

pub async fn put_object_url(
    client: &s3::Client,
    bucket: &str,
    object: &str,
    content_type: &str,
    expires_in: u64,
) -> Result<String, s3::Error> {
    let expires_in: std::time::Duration = Duration::from_secs(expires_in);
    let expires_in: s3::presigning::PresigningConfig =
        PresigningConfig::expires_in(expires_in).unwrap();

    let presigned_request = client
        .put_object()
        .bucket(bucket)
        .key(object)
        .content_type(content_type)
        .presigned(expires_in)
        .await?;

    Ok(presigned_request.uri().into())
}

pub async fn delete_objects_by_prefix(
    client: &s3::Client,
    bucket: &str,
    prefix: &str,
) -> Result<usize, s3::Error> {
    let mut objects_to_delete = Vec::new();

    let mut continuation_token: Option<String> = None;

    loop {
        let mut list_request = client
            .list_objects_v2()
            .bucket(bucket)
            .prefix(prefix);

        if let Some(token) = continuation_token {
            list_request = list_request.continuation_token(token);
        }

        let response = list_request.send().await?;

        if let Some(contents) = response.contents {
            for object in contents {
                if let Some(key) = object.key {
                    objects_to_delete.push(key);
                }
            }
        }

        if !response.is_truncated.unwrap_or(false) {
            break;
        }

        continuation_token = response.next_continuation_token;
    }

    if objects_to_delete.is_empty() {
        return Ok(0);
    }

    let delete_count = objects_to_delete.len();

    for key in objects_to_delete {
        client
            .delete_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await?;
    }

    Ok(delete_count)
}
