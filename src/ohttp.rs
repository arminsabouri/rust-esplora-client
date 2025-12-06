use bitcoin_ohttp as ohttp;
use reqwest::Client;

pub(crate) fn ohttp_encapsulate(
    method: &str,
    target_resource: &str,
    body: Option<&[u8]>,
    ohttp_keys: &mut ohttp::KeyConfig,
) -> Result<(Vec<u8>, ohttp::ClientResponse), ()> {
    use std::fmt::Write;

    let ctx =
        ohttp::ClientRequest::from_config(ohttp_keys).expect("Failed to create OHTTP context");
    let url = url::Url::parse(target_resource).expect("Failed to parse URL");
    let authority_bytes = url.host().map_or_else(Vec::new, |host| {
        let mut authority = host.to_string();
        if let Some(port) = url.port() {
            write!(authority, ":{port}").unwrap();
        }
        authority.into_bytes()
    });
    let mut bhttp_message = bhttp::Message::request(
        method.as_bytes().to_vec(),
        url.scheme().as_bytes().to_vec(),
        authority_bytes,
        url.path().as_bytes().to_vec(),
    );
    // None of our messages include headers, so we don't add them
    if let Some(body) = body {
        bhttp_message.write_content(body);
    }

    let mut bhttp_req = Vec::new();
    bhttp_message
        .write_bhttp(bhttp::Mode::IndeterminateLength, &mut bhttp_req)
        .expect("Failed to write BHTTP message");
    let (encapsulated, ohttp_ctx) = ctx.encapsulate(&bhttp_req).expect("Failed to encapsulate");

    return Ok((encapsulated, ohttp_ctx));
}

pub(crate) fn ohttp_decapsulate(
    res_ctx: ohttp::ClientResponse,
    ohttp_body: Vec<u8>,
) -> Result<http::Response<Vec<u8>>, ()> {
    let bhttp_body = res_ctx
        .decapsulate(&ohttp_body)
        .expect("Failed to decapsulate");
    let mut r = std::io::Cursor::new(bhttp_body);
    let m: bhttp::Message =
        bhttp::Message::read_bhttp(&mut r).expect("Failed to read BHTTP message");
    let mut builder = http::Response::builder();
    for field in m.header().iter() {
        builder = builder.header(field.name(), field.value());
    }
    Ok(builder
        .status({
            let code = m
                .control()
                .status()
                .ok_or(bhttp::Error::InvalidStatus)
                .expect("Failed to get status");

            http::StatusCode::from_u16(code.code())
                .map_err(|_| bhttp::Error::InvalidStatus)
                .expect("Failed to convert status code")
        })
        .body(m.content().to_vec())
        .expect("Failed to build HTTP response"))
}

pub(crate) async fn fetch_keys(client: &Client, url: &str) -> ohttp::KeyConfig {
    let res = client
        .get(format!("{}/ohttp-configs", url))
        .send()
        .await
        .expect("Failed to send request");
    let body = res.bytes().await.expect("Failed to get body");
    let keys = ohttp::KeyConfig::decode(&body).expect("Failed to parse keys");
    keys
}

