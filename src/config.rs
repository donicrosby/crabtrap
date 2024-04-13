use crate::ContentType;
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};
use tokio::net::ToSocketAddrs;

#[derive(Debug, Clone)]
pub struct TarpitConfig {
    min_body_size: u64,
    max_body_size: u64,
    tick_duration: Duration,
    duration_per_byte: Duration,
    content_type: ContentType,
}

impl TarpitConfig {
    pub fn new(
        min_body_size: u64,
        max_body_size: u64,
        tick_duration: u64,
        duration_per_byte: u64,
        content_type: ContentType,
    ) -> Self {
        Self {
            min_body_size,
            max_body_size,
            tick_duration: Duration::from_millis(tick_duration),
            duration_per_byte: Duration::from_millis(duration_per_byte),
            content_type,
        }
    }

    pub fn min_body_size(&self) -> u64 {
        self.min_body_size
    }

    pub fn max_body_size(&self) -> u64 {
        self.max_body_size
    }

    pub fn tick_duration(&self) -> Duration {
        self.tick_duration
    }

    pub fn duration_per_byte(&self) -> Duration {
        self.duration_per_byte
    }

    pub fn content_type(&self) -> ContentType {
        self.content_type.clone()
    }
}

impl Default for TarpitConfig {
    fn default() -> Self {
        Self {
            min_body_size: 1048576,
            max_body_size: 10485760,
            tick_duration: Duration::from_millis(50),
            duration_per_byte: Duration::from_millis(1600),
            content_type: ContentType::from(mime::TEXT_PLAIN),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    addr: SocketAddr,
    tp_config: TarpitConfig,
}

impl Config {
    pub fn new<Addr: ToSocketAddrs>(addr: Addr, tp_config: TarpitConfig) -> Self
    where
        Addr: Into<SocketAddr>,
    {
        Self {
            addr: addr.into(),
            tp_config,
        }
    }

    pub fn bind_addr(&self) -> &SocketAddr {
        &self.addr
    }

    pub fn tarpit_config(&self) -> &TarpitConfig {
        &self.tp_config
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 3000),
            tp_config: TarpitConfig::default(),
        }
    }
}
