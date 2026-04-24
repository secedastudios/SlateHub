//! End-to-end roundtrip test for the S3 service against the running RustFS
//! container. Exercises every public method: upload → exists → list →
//! download → presigned URLs → delete.
//!
//! Marked `#[ignore]` because it requires the RustFS container to be up:
//!
//!   make services
//!   cd server && cargo test --test s3_roundtrip_test -- --ignored --test-threads=1

use bytes::Bytes;
use slatehub::services::s3::S3Service;

const TEST_KEY: &str = "test/s3-roundtrip/hello.txt";
const TEST_BODY: &[u8] = b"hello from the s3 roundtrip test";
const TEST_CT: &str = "text/plain; charset=utf-8";

async fn svc() -> S3Service {
    // Point at the local RustFS by default; can be overridden via env.
    unsafe {
        std::env::set_var(
            "S3_ENDPOINT",
            std::env::var("S3_ENDPOINT").unwrap_or_else(|_| "http://localhost:9000".into()),
        );
        std::env::set_var(
            "S3_ACCESS_KEY",
            std::env::var("S3_ACCESS_KEY").unwrap_or_else(|_| "admin".into()),
        );
        std::env::set_var(
            "S3_SECRET_KEY",
            std::env::var("S3_SECRET_KEY").unwrap_or_else(|_| "password".into()),
        );
        std::env::set_var(
            "S3_BUCKET",
            std::env::var("S3_BUCKET").unwrap_or_else(|_| "slatehub".into()),
        );
        std::env::set_var(
            "S3_REGION",
            std::env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".into()),
        );
    }
    S3Service::new().await.expect("init S3 service")
}

#[tokio::test]
#[ignore = "requires running RustFS container (`make services`)"]
async fn test_s3_full_roundtrip() {
    let s3 = svc().await;

    // Cleanup any leftover from a previous failed run.
    let _ = s3.delete_file(TEST_KEY).await;

    // ---- upload ----
    let data = Bytes::from_static(TEST_BODY);
    let url = s3
        .upload_file(TEST_KEY, data.clone(), TEST_CT)
        .await
        .expect("upload should succeed");
    assert!(
        url.contains(TEST_KEY),
        "returned URL should reference the key, got: {url}"
    );

    // ---- exists ----
    assert!(
        s3.file_exists(TEST_KEY).await.expect("head_object"),
        "file_exists should be true after upload"
    );

    // ---- list ----
    let all_keys = s3.list_all_objects().await.expect("list_all_objects");
    assert!(
        all_keys.iter().any(|k| k == TEST_KEY),
        "list_all_objects should contain the uploaded key, got: {:?}",
        all_keys
            .iter()
            .filter(|k| k.starts_with("test/"))
            .collect::<Vec<_>>()
    );

    // ---- download ----
    let (bytes, ct) = s3.download_file(TEST_KEY).await.expect("download");
    assert_eq!(
        bytes.as_ref(),
        TEST_BODY,
        "downloaded bytes should match uploaded"
    );
    assert!(
        ct.to_lowercase().starts_with("text/plain"),
        "content-type should round-trip, got: {ct}"
    );

    // ---- presigned GET ----
    let get_url = s3
        .generate_download_url(TEST_KEY)
        .await
        .expect("presign_get");
    assert!(
        get_url.contains(TEST_KEY) && get_url.contains("X-Amz-Signature"),
        "presigned GET URL looks wrong: {get_url}"
    );
    let resp = reqwest::get(&get_url).await.expect("presigned GET");
    assert!(
        resp.status().is_success(),
        "presigned GET status: {}",
        resp.status()
    );
    let body = resp.bytes().await.expect("presigned GET body");
    assert_eq!(body.as_ref(), TEST_BODY, "presigned GET bytes should match");

    // ---- presigned PUT (upload new content via URL) ----
    let put_key = "test/s3-roundtrip/presigned-put.txt";
    let _ = s3.delete_file(put_key).await;
    let put_url = s3
        .generate_upload_url(put_key, "text/plain")
        .await
        .expect("presign_put");
    assert!(
        put_url.contains(put_key) && put_url.contains("X-Amz-Signature"),
        "presigned PUT URL looks wrong: {put_url}"
    );
    let put_body = b"uploaded via presigned PUT";
    let put_resp = reqwest::Client::new()
        .put(&put_url)
        .body(put_body.to_vec())
        .send()
        .await
        .expect("presigned PUT");
    assert!(
        put_resp.status().is_success(),
        "presigned PUT status: {}",
        put_resp.status()
    );

    // Confirm it landed.
    let (puts_bytes, _) = s3
        .download_file(put_key)
        .await
        .expect("download after presigned PUT");
    assert_eq!(puts_bytes.as_ref(), put_body);

    // ---- bucket_name ----
    assert!(!s3.bucket_name().is_empty());

    // ---- delete ----
    s3.delete_file(TEST_KEY).await.expect("delete");
    s3.delete_file(put_key)
        .await
        .expect("delete presigned-put file");
    assert!(
        !s3.file_exists(TEST_KEY).await.expect("head_object"),
        "file_exists should be false after delete"
    );
}
