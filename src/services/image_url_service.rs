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
