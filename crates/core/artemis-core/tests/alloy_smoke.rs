#![cfg(feature = "sdk-alloy")]

use artemis_core::eth::alloy_support::helpers;

#[tokio::test]
async fn alloy_helpers_handle_invalid_urls() {
    let wallet = helpers::parse_local_wallet(
        "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )
    .expect("wallet parses");
    assert_ne!(wallet.address().to_string().len(), 0);

    helpers::create_ws_provider("ws://127.0.0.1:0")
        .await
        .expect_err("ws connection should fail");
    helpers::create_http_provider("http://127.0.0.1:0")
        .await
        .expect_err("http connection should fail");
}
