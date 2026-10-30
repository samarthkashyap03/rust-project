// Connects to the relay as a game, opens one slot for one phone
// and prints "A" every time that phone presses A.

#[allow(dead_code)]
mod protocol;

use protocol::{GameMsg, Layout, PROTO, ToGame};
use qrcode::QrCode;
use qrcode::render::unicode::Dense1x2;
use std::env;
use std::error::Error;
use std::net::TcpStream;
use tungstenite::client::IntoClientRequest;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

/// Our connection to the relay: a WebSocket over TCP, with or without TLS.
type Socket = WebSocket<MaybeTlsStream<TcpStream>>;

fn main() -> Result<(), Box<dyn Error>> {
    let mut socket = connect()?;

    let slot = String::from("p1");
    send(&mut socket, &GameMsg::OpenSlot { req_id: 1, slot })?;
    // A frame arrives every 20 ms, even without phones.
    // Skip them until the link comes.
    // Question: what happens here if the relay cannot open the slot?
    let link = loop {
        match recv(&mut socket)? {
            ToGame::SlotLink { url, .. } => break url,
            ToGame::Error { message, .. } => println!("error: {message}"),
            _ => {} // frames
        }
    };
    // Upper case makes the QR code smaller (fewer kinds of characters).
    // Question: does the QR code still scan in a dark terminal?
    let link = link.to_uppercase();
    let qr = QrCode::new(&link)?.render::<Dense1x2>().build();
    println!("{qr}\nopen {link} on your phone");

    // From now on: react to what the relay sends, forever.
    loop {
        match recv(&mut socket)? {
            ToGame::PhoneConnected { slot } => println!("{slot} connected"),
            ToGame::PhoneDisconnected { slot } => println!("{slot} left"),
            ToGame::Error { message, .. } => println!("error: {message}"),
            _ => {} // frames
        }
    }
}

/// Connects to the relay with our token and says hello.
fn connect() -> Result<Socket, Box<dyn Error>> {
    // The token is a password: it stays out of the code and the repo.
    let base = env_var("INPUT_MUX_URL")?;
    let token = env_var("INPUT_MUX_TOKEN")?;
    let url = game_url(&base)?;

    // rustls encrypts wss:// and can use several crypto libraries;
    // we choose ring. Err only means one was chosen already.
    let _ = rustls::crypto::ring::default_provider().install_default();

    // Only games need a token; a phone only needs its link.
    println!("connecting to {url}");
    let mut request = url.into_client_request()?;
    let bearer = format!("Bearer {token}").parse()?;
    request.headers_mut().insert("Authorization", bearer);
    let mut socket = match tungstenite::connect(request) {
        Ok((socket, _response)) => socket,
        // A wrong token: the relay refuses with "401 Unauthorized".
        Err(tungstenite::Error::Http(response)) => {
            return Err(format!("relay says {}", response.status()).into());
        }
        Err(e) => return Err(e.into()),
    };

    // Without hello within 5 s, the relay closes with code 4002.
    let hello = GameMsg::Hello {
        proto: PROTO,
        layout: Layout::Any,
    };
    send(&mut socket, &hello)?;
    match recv(&mut socket)? {
        ToGame::Session { session, .. } => println!("session {session}"),
        other => return Err(format!("expected a session: {other:?}").into()),
    }
    Ok(socket)
}

/// Reads an environment variable; the error names the variable.
fn env_var(name: &str) -> Result<String, String> {
    env::var(name).map_err(|_| format!("{name} is not set"))
}

/// The relay's address in the browser is https://host; its socket for
/// games is wss://host/ws/game (http:// and ws:// without encryption).
fn game_url(base: &str) -> Result<String, String> {
    let base = base.trim_end_matches('/');
    if let Some(host) = base.strip_prefix("https://") {
        Ok(format!("wss://{host}/ws/game"))
    } else if let Some(host) = base.strip_prefix("http://") {
        Ok(format!("ws://{host}/ws/game"))
    } else {
        Err(format!("INPUT_MUX_URL is not an http(s) URL: {base}"))
    }
}

/// Sends one message as a JSON text frame.
fn send(socket: &mut Socket, msg: &GameMsg) -> Result<(), Box<dyn Error>> {
    socket.send(Message::text(serde_json::to_string(msg)?))?;
    Ok(())
}

/// Waits for the next message from the relay.
fn recv(socket: &mut Socket) -> Result<ToGame, Box<dyn Error>> {
    loop {
        match socket.read()? {
            Message::Text(json) => return Ok(serde_json::from_str(&json)?),
            Message::Close(frame) => {
                return Err(format!("relay closed: {frame:?}").into());
            }
            _ => {} // pings: tungstenite answers them for us
        }
    }
}
