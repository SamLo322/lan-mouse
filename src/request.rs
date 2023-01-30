use std::{
    net::{TcpListener, SocketAddr, TcpStream},
    io::prelude::*,
    collections::HashMap, sync::{RwLock, Arc},
    error::Error,
    thread::{self, JoinHandle},
};

use memmap::Mmap;

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum Request {
    KeyMap,
    Connect,
}

impl TryFrom<[u8; 4]> for Request {
    fn try_from(buf: [u8; 4]) -> Result<Self, Self::Error> {
        let val = u32::from_ne_bytes(buf);
        match val {
            x if x == Request::KeyMap as u32 => Ok(Self::KeyMap),
            x if x == Request::Connect as u32 => Ok(Self::Connect),
            _ => Err("Bad Request"),
        }
    }

    type Error = &'static str;
}

#[derive(Clone)]
pub struct Server {
    data: Arc<RwLock<HashMap<Request, Mmap>>>,
}

impl Server {
    fn handle_request(&self, mut stream: TcpStream) {
        let mut buf = [0u8; 4];
        stream.read_exact(&mut buf).unwrap();
        match Request::try_from(buf) {
            Ok(Request::KeyMap) => {
                let data = self.data.read().unwrap();
                let buf = data.get(&Request::KeyMap);
                match buf {
                    None => {
                        stream.write(&0u32.to_ne_bytes()).unwrap();
                    }
                    Some(buf) => {
                        stream.write(&buf[..].len().to_ne_bytes()).unwrap();
                        stream.write(&buf[..]).unwrap();
                    }
                }
                stream.flush().unwrap();
            }
            Ok(Request::Connect) => todo!(),
            Err(msg) => eprintln!("{}", msg),
        }
    }

    pub fn listen(port: u16) -> Result<(Server, JoinHandle<()>), Box<dyn Error>> {
        let data: Arc<RwLock<HashMap<Request, Mmap>>> = Arc::new(RwLock::new(HashMap::new()));
        let listen_addr = SocketAddr::new("0.0.0.0".parse().unwrap(), port);
        let server = Server { data };
        let server_copy = server.clone();
        let thread = thread::spawn(move || {
            let listen_socket = TcpListener::bind(listen_addr).unwrap();
            for stream in listen_socket.incoming() {
                match stream {
                    Ok(stream) => {
                        server.handle_request(stream);
                    }
                    Err(e) => {
                        eprintln!("{}", e);
                    }
                }
            }
        });
        Ok((server_copy, thread))
    }

    pub fn offer_data(&self, req: Request, d: Mmap) {
        self.data.write().unwrap().insert(req, d);
    }

}

pub fn request_data(addr: SocketAddr, req: Request) -> Option<Vec<u8>> {
    // connect to server
    let mut sock = match TcpStream::connect(addr) {
        Ok(sock) => sock,
        Err(e) => {
            eprintln!("{}", e);
            return None;
        }
    };

    // write the request to the socket
    // convert to u32
    let req: u32 = req as u32;
    if let Err(e) =  sock.write(&req.to_ne_bytes()) {
        eprintln!("{}", e);
        return None;
    }
    if let Err(e) = sock.flush() {
        eprintln!("{}", e);
        return None;
    }

    // read the response = (len, data) - len 0 means no data / bad request
    // read len
    let mut buf = [0u8; 8];
    if let Err(e) = sock.read_exact(&mut buf[..]) {
        eprintln!("{}", e);
        return None;
    }
    let len = usize::from_ne_bytes(buf);

    // check for bad request
    if len == 0 {
        return None;
    }

    // read the data
    let mut data: Vec<u8> = vec![0u8; len];
    if let Err(e) = sock.read_exact(&mut data[..]) {
        eprintln!("{}", e);
        return None;
    }
    Some(data)
}
