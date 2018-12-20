use crate::errors::Error;

use crate::packet::Packet;

use futures::Poll;
use ipc_channel::ipc::{IpcOneShotServer, IpcSender};
use log::*;
use std::pin::Pin;
use std::task::LocalWaker;

pub struct Server {
    server: IpcOneShotServer<IpcSender<Option<Vec<Packet>>>>,
    name: String,
}

impl Server {
    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn new() -> Result<Server, Error> {
        let (server, server_name) = IpcOneShotServer::new().map_err(Error::Io)?;

        Ok(Server {
            server: server,
            name: server_name,
        })
    }

    pub async fn accept(self) -> Result<ConnectedIpc, Error> {
        let (_, tx): (_, IpcSender<Option<Vec<Packet>>>) =
            self.server.accept().map_err(Error::Bincode)?;

        info!("Accepted connection from {:?}", tx);

        Ok(ConnectedIpc { connection: tx })
    }
}

pub struct ConnectedIpc {
    connection: IpcSender<Option<Vec<Packet>>>,
}

impl futures::sink::Sink for ConnectedIpc {
    type SinkItem = Vec<Packet>;
    type SinkError = Error;

    fn poll_ready(self: Pin<&mut Self>, _: &LocalWaker) -> Poll<Result<(), Self::SinkError>> {
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: Self::SinkItem) -> Result<(), Self::SinkError> {
        self.get_mut().connection.send(Some(item)).map_err(|e| {
            error!("Failed to send {:?}", e);
            Error::Bincode(e)
        })
    }

    fn poll_flush(self: Pin<&mut Self>, _: &LocalWaker) -> Poll<Result<(), Self::SinkError>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _: &LocalWaker) -> Poll<Result<(), Self::SinkError>> {
        info!("Closing IPC Server");
        Poll::Ready(self.get_mut().connection.send(None).map_err(Error::Bincode))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use futures::{SinkExt, StreamExt};
    use ipc_channel::ipc::{self, IpcSender};

    #[test]
    fn test_connection() {
        let server = Server::new().expect("Failed to build server");

        let server_name = server.name().clone();

        let future_accept = server.accept();

        let (tx, _rx) = ipc::channel::<Option<Vec<Packet>>>().expect("Failed to create channel");
        let server_sender: IpcSender<IpcSender<Option<Vec<Packet>>>> =
            IpcSender::connect(server_name).expect("Server failed to connect");

        let connected_thread = std::thread::spawn(move || {
            let f = async { await!(future_accept) };
            futures::executor::block_on(f)
        });

        server_sender
            .send(tx)
            .expect("Failed to send client sender");

        connected_thread
            .join()
            .expect("Failed to join")
            .expect("No connection");
    }

    #[test]
    fn test_sending() {
        let server = Server::new().expect("Failed to build server");

        let server_name = server.name().clone();

        let future_accept = server.accept();

        let (tx, rx) = ipc::channel::<Option<Vec<Packet>>>().expect("Failed to create channel");
        let server_sender: IpcSender<IpcSender<Option<Vec<Packet>>>> =
            IpcSender::connect(server_name).expect("Server failed to connect");

        let connected_thread = std::thread::spawn(move || {
            let f = async { await!(future_accept) };
            futures::executor::block_on(f).expect("Failed to accept")
        });

        server_sender
            .send(tx)
            .expect("Failed to send client sender");

        let mut connection = connected_thread.join().expect("No connection");

        let client_result = std::thread::spawn(move || {
            let mut count = 0;
            while let Some(p) = rx.recv().expect("Failed to receive packets") {
                count += p.len();
                if p.is_empty() {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            }
            count
        });

        let f = async {
            await!(connection.send(vec![Packet::new(std::time::UNIX_EPOCH, vec![2u8])]))
                .expect("Failed to send");
            await!(connection.close()).expect("Failed to close");
        };

        futures::executor::block_on(f);

        assert_eq!(client_result.join().expect("Failed to receive"), 1);
    }

    #[test]
    fn test_sink() {
        let server = Server::new().expect("Failed to build server");

        let server_name = server.name().clone();

        let future_accept = server.accept();

        let (tx, rx) = ipc::channel::<Option<Vec<Packet>>>().expect("Failed to create channel");
        let server_sender: IpcSender<IpcSender<Option<Vec<Packet>>>> =
            IpcSender::connect(server_name).expect("Server failed to connect");

        let connected_thread = std::thread::spawn(move || {
            let f = async { await!(future_accept) };
            futures::executor::block_on(f).expect("Failed to accept")
        });

        server_sender
            .send(tx)
            .expect("Failed to send client sender");

        let connection = connected_thread.join().expect("No connection");

        let client_result = std::thread::spawn(move || {
            let mut count = 0;
            while let Some(p) = rx.recv().expect("Failed to receive packets") {
                count += p.len();
                if p.is_empty() {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            }
            count
        });

        let packets_sent_fut =
            futures::stream::iter(vec![vec![Packet::new(std::time::UNIX_EPOCH, vec![0u8])]])
                .map(|packets| Ok(packets))
                .forward(connection);

        futures::executor::block_on(packets_sent_fut).expect("Failed to run");

        assert_eq!(client_result.join().expect("Failed to receive"), 1);
    }
}
