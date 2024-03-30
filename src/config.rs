use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::ToSocketAddrs;

#[derive(Debug, Copy, Clone)]
pub struct Config {
    addr: SocketAddr,
}

impl Config {
    pub fn new<Addr: ToSocketAddrs>(addr: Addr) -> Self
    where
        Addr: Into<SocketAddr>,
    {
        Self { addr: addr.into() }
    }

    pub fn bind_addr(&self) -> &SocketAddr {
        &self.addr
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 3000),
        }
    }
}
