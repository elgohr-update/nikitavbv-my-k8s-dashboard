use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use thrussh::CryptoVec;
use thrussh::{ChannelId, server, server::{Auth, Session}};
use futures::Future;

#[tokio::main]
async fn main() {
    println!("Hello, docker proxy!");

    let client_key = thrussh_keys::key::KeyPair::generate_ed25519().unwrap();
    let client_pubkey = Arc::new(client_key.clone_public_key());
    let mut config = thrussh::server::Config::default();
    config.connection_timeout = Some(std::time::Duration::from_secs(3));
    config.auth_rejection_time = std::time::Duration::from_secs(3);
    config.keys.push(thrussh_keys::key::KeyPair::generate_ed25519().unwrap());
    let config = Arc::new(config);
    let sh = Server {
        client_pubkey,
        clients: Arc::new(Mutex::new(HashMap::new())),
        id: 0,
    };

    thrussh::server::run(config, "0.0.0.0:2222", sh).await.unwrap();
}

#[derive(Clone)]
struct Server {
    client_pubkey: Arc<thrussh_keys::key::PublicKey>,
    clients: Arc<Mutex<HashMap<(usize, ChannelId), thrussh::server::Handle>>>,
    id: usize,
}

impl server::Server for Server {
    type Handler = Self;

    fn new(&mut self, _: Option<std::net::SocketAddr>) -> Self {
        let s = self.clone();
        self.id += 1;
        s
    }
}

impl server::Handler for Server {
    type Error = anyhow::Error;
    type FutureAuth = futures::future::Ready<Result<(Self, server::Auth), anyhow::Error>>;
    type FutureUnit = futures::future::Ready<Result<(Self, Session), anyhow::Error>>;
    type FutureBool = futures::future::Ready<Result<(Self, Session, bool), anyhow::Error>>;

    fn finished_auth(mut self, auth: Auth) -> Self::FutureAuth {
        println!("auth finished");

        futures::future::ready(Ok((self, auth)))
    }

    fn finished_bool(self, b: bool, session: server::Session) -> Self::FutureBool {
        futures::future::ready(Ok((self, session, b)))
    }

    fn finished(self, session: server::Session) -> Self::FutureUnit {
        println!("session finished");

        futures::future::ready(Ok((self, session)))
    }

    fn channel_open_session(self, channel: ChannelId, session: server::Session) -> Self::FutureUnit {
        println!("openning session to a new client");
        
        {
            let mut clients = self.clients.lock().unwrap();
            clients.insert((self.id, channel), session.handle());
        }

        futures::future::ready(Ok((self, session)))
        // self.finished(session)
    }

    fn auth_publickey(self, user: &str, public_key: &thrussh_keys::key::PublicKey) -> Self::FutureAuth {
        println!("handling auth with publickey");
        self.finished_auth(server::Auth::Accept)
    }

    fn auth_keyboard_interactive(self, user: &str, submethods: &str, response: Option<server::Response>) -> Self::FutureAuth {
        println!("handling auth keyboard interactive");
        self.finished_auth(server::Auth::Accept)
    }

    fn data(self, channel: ChannelId, data: &[u8], mut session: Session) -> Self::FutureUnit {
        println!("got data from client");
        
        {
            let mut clients = self.clients.lock().unwrap();
            for ((id, channel), ref mut s) in clients.iter_mut() {
                if *id != self.id {
                    s.data(*channel, CryptoVec::from_slice(data));
                }
            }
        }
        session.data(channel, CryptoVec::from_slice(data));

        futures::future::ready(Ok((self, session)))
        //self.finished(session)
    }
}