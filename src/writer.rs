use crate::{TarPitMetadata, TarpitConnection};
use hyper::body::Bytes;
use rand::distributions::Alphanumeric;
use rand::prelude::*;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::{
    sync::Mutex,
    time::{self, Duration},
};
use tokio_stream::{self as stream, StreamExt};
use tracing::{debug, info, trace, warn};

pub(crate) type TarpitConnRecv = UnboundedReceiver<TarpitConnection>;
pub(crate) type TarpitConnSend = UnboundedSender<TarpitConnection>;

pub(crate) struct TarpitWriter {
    connections: Arc<Mutex<VecDeque<TarpitConnection>>>,
    time_per_byte: Duration,
    tick_period: Duration,
    conn_recv: TarpitConnRecv,
}

impl TarpitWriter {
    pub fn new(tick_period: Duration, time_per_byte: Duration, conn_recv: TarpitConnRecv) -> Self {
        Self {
            connections: Arc::new(Mutex::new(VecDeque::new())),
            time_per_byte,
            tick_period,
            conn_recv,
        }
    }

    fn get_num_time_slices(&self) -> u128 {
        self.time_per_byte.as_millis() / self.tick_period.as_millis()
    }

    fn num_connections_per_time_slice(&self, num_slices: u128, num_conns: usize) -> usize {
        let div = num_conns / num_slices as usize;
        if div > 0 {
            div
        } else if num_conns > 0 {
            1
        } else {
            0
        }
    }

    async fn process_connections_inner(&mut self) {
        let new_conns = self.get_new_connections().await;
        let mut conns = self.connections.lock().await;
        if let Some(new_conns) = new_conns {
            conns.append(&mut new_conns.into())
        }
        debug!("Num Conns: {}", conns.len());

        let num_slices = self.get_num_time_slices();
        let num_connections_to_process =
            self.num_connections_per_time_slice(num_slices, conns.len());
        debug!("Processing {} conns...", num_connections_to_process);

        let to_process = conns.drain(..num_connections_to_process);
        let work_stream = stream::iter(to_process);
        let mut processed_connections = work_stream
            .filter_map(|mut conn| {
                if conn.should_send_byte(self.time_per_byte) {
                    let char: Bytes = rand::thread_rng()
                        .clone()
                        .sample_iter(&Alphanumeric)
                        .take(1)
                        .collect();
                    if conn.send_byte(char).is_ok() {
                        trace!("Byte sent successfully");
                    } else {
                        warn!("Byte failed to send");
                    }
                }
                if !conn.should_abort() {
                    Some(conn)
                } else {
                    info!("Connection complete, cleaning up...");
                    None
                }
            })
            .collect::<Vec<_>>()
            .await
            .into();
        conns.append(&mut processed_connections);
    }

    fn display_connection_info(metadata: &TarPitMetadata) {
        let host = metadata.host();
        let user_agent = metadata.user_agent_string();
        let path = metadata.location();
        let query = metadata.query();
        let method = metadata.method();
        info!(
            "Connection: {{ Host: {}, User Agent: {}, Method: {}, Path: {}, Query: {} }}",
            host, user_agent, method, path, query
        );
    }

    async fn get_new_connections(&mut self) -> Option<Vec<TarpitConnection>> {
        trace!("Writer looking for new connections...");
        let mut conns = Vec::new();
        while !self.conn_recv.is_empty() {
            if let Some(conn) = self.conn_recv.recv().await {
                debug!("Found connection...");
                Self::display_connection_info(conn.get_conn_metadata());
                conns.push(conn)
            }
        }
        if !conns.is_empty() {
            Some(conns)
        } else {
            None
        }
    }

    pub async fn process_connections(&mut self) {
        let mut time_slice = time::interval(self.tick_period);
        loop {
            time_slice.tick().await;
            trace!("Writer waking up from sleep...");
            self.process_connections_inner().await
        }
    }
}
