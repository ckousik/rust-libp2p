use async_trait::async_trait;
use futures::io::BufReader;
use futures::prelude::*;
use libp2p::swarm::SwarmEvent;
use libp2p::{PeerId, Transport};
use libp2p_core::muxing::StreamMuxerBox;
use libp2p_request_response::{
    ProtocolName, ProtocolSupport, RequestResponse, RequestResponseCodec, RequestResponseConfig,
    RequestResponseEvent, RequestResponseMessage,
};
use libp2p_swarm::SwarmBuilder;
use std::error::Error;
use std::{io, iter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let (local_peer_id, transport) = make_webrtc_transport();
    let protocols = iter::once((EchoProtocol(), ProtocolSupport::Full));
    let echo_proto = RequestResponse::new(EchoCodec(), protocols, RequestResponseConfig::default());
    println!("Local peer id: {:?}", local_peer_id);

    let mut swarm = SwarmBuilder::new(transport, echo_proto, local_peer_id)
        .executor(Box::new(|fut| {
            tokio::spawn(fut);
        }))
        .build();

    swarm
        .listen_on("/ip4/0.0.0.0/udp/0/webrtc".parse().unwrap())
        .unwrap();
    // Tell the swarm to listen on all interfaces and a random, OS-assigned
    // port.

    loop {
        match swarm.select_next_some().await {
            SwarmEvent::NewListenAddr { address, .. } => {
                println!("Listening on {}", address)
            }
            SwarmEvent::Behaviour(event) => match event {
                RequestResponseEvent::Message {
                    message:
                        RequestResponseMessage::Request {
                            request: EchoRequest(request_data),
                            channel,
                            ..
                        },
                    ..
                } => {
                    match swarm
                        .behaviour_mut()
                        .send_response(channel, EchoResponse(request_data))
                    {
                        Ok(()) => println!("wrote echo response"),
                        Err(err) => println!("error writing response: {:?}", err),
                    }
                }
                e => println!("got event: {:?}", e),
            },
            e => println!("swarm event: {:?}", e),
        }
    }
}

fn make_webrtc_transport() -> (
    PeerId,
    libp2p_core::transport::Boxed<(PeerId, StreamMuxerBox)>,
) {
    let id_keys = libp2p_core::identity::Keypair::generate_ed25519();
    let peer_id = id_keys.public().to_peer_id();
    let transport = libp2p_webrtc::tokio::Transport::new(
        id_keys,
        libp2p_webrtc::tokio::Transport::gen_ceritificate(),
    );
    let transport = transport
        .map(|(peer_id, conn), _| (peer_id, StreamMuxerBox::new(conn)))
        .boxed();
    (peer_id, transport)
}

#[derive(Debug, Clone)]
struct EchoProtocol();

#[derive(Debug, Clone)]
struct EchoRequest(Vec<u8>);

#[derive(Debug, Clone)]
struct EchoResponse(Vec<u8>);

#[derive(Debug, Clone)]
struct EchoCodec();

impl ProtocolName for EchoProtocol {
    fn protocol_name(&self) -> &[u8] {
        "/echo/1.0.0".as_bytes()
    }
}

#[async_trait]
impl RequestResponseCodec for EchoCodec {
    type Protocol = EchoProtocol;
    type Request = EchoRequest;
    type Response = EchoResponse;

    async fn read_request<T>(&mut self, _: &EchoProtocol, io: &mut T) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        let mut result = String::new();
        let mut reader = BufReader::new(io);
        match reader.read_line(&mut result).await {
            Ok(_) => io::Result::Ok(EchoRequest(result.into_bytes())),
            Err(e) => Err(e),
        }
    }

    async fn read_response<T>(&mut self, _: &EchoProtocol, io: &mut T) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        let mut result = String::new();
        let mut reader = BufReader::new(io);
        match reader.read_line(&mut result).await {
            Ok(_) => io::Result::Ok(EchoResponse(result.into_bytes())),
            Err(e) => Err(e),
        }
    }

    async fn write_request<T>(
        &mut self,
        _: &EchoProtocol,
        io: &mut T,
        EchoRequest(data): EchoRequest,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        match io.write(&data).await {
            Ok(_) => Ok(()),
            Err(err) => Err(err),
        }
    }

    async fn write_response<T>(
        &mut self,
        _: &EchoProtocol,
        io: &mut T,
        EchoResponse(data): EchoResponse,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        match io.write(&data).await {
            Ok(_) => Ok(()),
            Err(err) => Err(err),
        }
    }
}
