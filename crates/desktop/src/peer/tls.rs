//! Source previous release `92e3fe4c:desktop/peer-network.mjs`: TLS 1.3, AES-128-GCM and HKDF-SHA256 PSK.
//! OpenSSL's supported PSK callback also serves TLS 1.3 with its SHA256 digest.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use openssl::{
    error::ErrorStack,
    md::Md,
    pkey::Id,
    pkey_ctx::PkeyCtx,
    ssl::{Ssl, SslContext, SslContextBuilder, SslMethod, SslVerifyMode, SslVersion},
};
use std::pin::Pin;
use tokio::net::TcpStream;
use tokio_openssl::SslStream;

use super::{CONNECT_TIMEOUT, Invite};

pub(super) fn transport_key(secret: &str) -> Result<[u8; 32], String> {
    let secret = URL_SAFE_NO_PAD.decode(secret).map_err(|_| "Invalid invite key.")?;
    let mut key = [0_u8; 32];
    let mut derive = || -> Result<usize, ErrorStack> {
        let mut hkdf = PkeyCtx::new_id(Id::HKDF)?;
        hkdf.derive_init()?;
        hkdf.set_hkdf_md(Md::sha256())?;
        hkdf.set_hkdf_salt(b"")?;
        hkdf.set_hkdf_key(&secret)?;
        hkdf.add_hkdf_info(b"solomon-darker-peer-v1")?;
        hkdf.derive(Some(&mut key))
    };
    if derive().map_err(|error| format!("Peer encryption: {error}"))? != key.len() {
        return Err("Peer encryption returned an invalid key size".into());
    }
    Ok(key)
}

fn context(method: SslMethod) -> Result<SslContextBuilder, ErrorStack> {
    let mut context = SslContextBuilder::new(method)?;
    context.set_min_proto_version(Some(SslVersion::TLS1_3))?;
    context.set_max_proto_version(Some(SslVersion::TLS1_3))?;
    context.set_ciphersuites("TLS_AES_128_GCM_SHA256")?;
    Ok(context)
}

pub(super) fn host(invite: &Invite) -> Result<SslContext, String> {
    let key = transport_key(&invite.secret)?;
    let identity = format!("sdr-v1:{}", invite.revision).into_bytes();
    let mut context = context(SslMethod::tls_server()).map_err(|error| error.to_string())?;
    // The published host requests no X.509 client certificate; PSK possession authenticates it.
    context.set_verify(SslVerifyMode::NONE);
    context.set_psk_server_callback(move |_, offered, output| {
        if offered != Some(identity.as_slice()) || output.len() < key.len() {
            return Ok(0);
        }
        output[..key.len()].copy_from_slice(&key);
        Ok(key.len())
    });
    Ok(context.build())
}

pub(super) fn guest(invite: &Invite) -> Result<SslContext, String> {
    let key = transport_key(&invite.secret)?;
    let identity = format!("sdr-v1:{}", invite.revision).into_bytes();
    let mut context = context(SslMethod::tls_client()).map_err(|error| error.to_string())?;
    context.set_verify(SslVerifyMode::PEER);
    context.set_psk_client_callback(move |_, _, name, output| {
        if name.len() <= identity.len() || output.len() < key.len() {
            return Ok(0);
        }
        name[..identity.len()].copy_from_slice(&identity);
        name[identity.len()] = 0;
        output[..key.len()].copy_from_slice(&key);
        Ok(key.len())
    });
    Ok(context.build())
}

pub(super) async fn connect(
    invite: &Invite,
    context: &SslContext,
) -> Result<SslStream<TcpStream>, String> {
    tokio::time::timeout(CONNECT_TIMEOUT, async {
        let tcp = TcpStream::connect((invite.host.as_str(), invite.port))
            .await
            .map_err(|error| error.to_string())?;
        tcp.set_nodelay(true).map_err(|error| error.to_string())?;
        let ssl = Ssl::new(context).map_err(|error| error.to_string())?;
        let mut stream = SslStream::new(ssl, tcp).map_err(|error| error.to_string())?;
        Pin::new(&mut stream).connect().await.map_err(|error| error.to_string())?;
        Ok(stream)
    })
    .await
    .map_err(|_| "Peer connection timed out.".to_owned())?
}

pub(super) async fn accept(
    tcp: TcpStream,
    context: &SslContext,
) -> Result<SslStream<TcpStream>, String> {
    tcp.set_nodelay(true).map_err(|error| error.to_string())?;
    let ssl = Ssl::new(context).map_err(|error| error.to_string())?;
    let mut stream = SslStream::new(ssl, tcp).map_err(|error| error.to_string())?;
    tokio::time::timeout(CONNECT_TIMEOUT, Pin::new(&mut stream).accept())
        .await
        .map_err(|_| "Peer handshake timed out.".to_owned())?
        .map_err(|error| error.to_string())?;
    Ok(stream)
}
