mod anytls;
mod direct;
mod http;
mod hysteria2;
mod reject;
mod shadowsocks;
mod socks5;
mod trait_def;
mod trojan;
mod vless;
mod vmess;
#[cfg(test)]
mod tests;

pub use anytls::AnyTlsProxy;
pub use direct::DirectProxy;
pub use http::HttpProxy;
pub use hysteria2::Hysteria2Proxy;
pub use reject::RejectProxy;
pub use shadowsocks::ShadowsocksProxy;
pub use socks5::Socks5Proxy;
pub use trait_def::{BoxedDatagram, Datagram, BoxedStream, OutboundProxy, Target};
pub use trojan::TrojanProxy;
pub use vless::VlessProxy;
pub use vmess::VMessProxy;

/// 初始化 rustls CryptoProvider
pub fn init_crypto_provider() {
    // 安装 ring 作为默认的 CryptoProvider
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok(); // 忽略重复安装的错误
}
