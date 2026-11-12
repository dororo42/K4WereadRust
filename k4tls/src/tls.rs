//! TLS 连接器：webpki-roots 基础信任库（进程内缓存）+ 可选 per-request PEM 追加根
//! （供 B-3 mock 网关/受控 TLS 夹具用；生产 Lua 路径不传）。
//!
//! 注意：`extra_pem` 是「在公共根库之上**追加**自定义根」，不是证书 pinning——
//! 基础信任库始终生效。若将来需要真正的 pinning，应提供独立的替换模式开关。

use std::sync::OnceLock;

use rustls::ClientConfig;
use rustls::RootCertStore;
use rustls_pki_types::{pem::PemObject, CertificateDer, ServerName};
use webpki_roots::TLS_SERVER_ROOTS;

fn base_roots() -> &'static RootCertStore {
    static BASE: OnceLock<RootCertStore> = OnceLock::new();
    BASE.get_or_init(|| {
        let mut store = RootCertStore::empty();
        store.extend(TLS_SERVER_ROOTS.iter().cloned());
        store
    })
}

pub enum TrustError {
    InvalidPem(String),
}

pub fn build_config(extra_pem: Option<&[u8]>) -> Result<ClientConfig, TrustError> {
    // 2026-11-12 audit (F-12): this expect is reachable from k4tls_perform,
    // whose catch_unwind turns it into rc=-99 instead of unwinding across the
    // FFI boundary; the response is lost though. A later hardening batch should
    // return TrustError instead. No local Rust builds are allowed by project
    // discipline, so that change needs CI verification before landing.
    let provider = rustls::crypto::ring::default_provider();
    let builder = ClientConfig::builder_with_provider(provider.into())
        .with_protocol_versions(&[&rustls::version::TLS13, &rustls::version::TLS12])
        .expect("protocol versions valid");
    let builder = match extra_pem {
        None => builder.with_root_certificates(base_roots().clone()),
        Some(pem) => {
            let mut store = base_roots().clone();
            let mut parsed = 0usize;
            for cert in CertificateDer::pem_slice_iter(pem) {
                let cert = cert.map_err(|e| TrustError::InvalidPem(e.to_string()))?;
                store
                    .add(cert)
                    .map_err(|e| TrustError::InvalidPem(e.to_string()))?;
                parsed += 1;
            }
            // fail-closed：提供了 PEM 却解不出任何证书 = 明确配置错误，不得静默降级
            if parsed == 0 {
                return Err(TrustError::InvalidPem(
                    "no certificates in trust pem".into(),
                ));
            }
            builder.with_root_certificates(store)
        }
    };
    Ok(builder.with_no_client_auth())
}

pub fn server_name(
    host: &str,
) -> Result<ServerName<'static>, rustls_pki_types::InvalidDnsNameError> {
    ServerName::try_from(host.to_string())
}
