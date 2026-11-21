//! Сетевая игра: обнаружение игроков в локальной сети и зашифрованный обмен.
//!
//! Транспорт — TCP на порту 47771. Шифрование — Noise_XX_25519_ChaChaPoly_BLAKE2s:
//! в рукопожатии каждая сторона отправляет ровно одно приветствие (свой ID, код
//! правил и свободный текст), после чего весь трафик идёт зашифрованным.
//! Обнаружение — UDP-маяки на порт 47772: каждый экземпляр игры раз в секунду
//! объявляет себя, и все видят «игроков рядом». Ход передаётся компактным
//! пакетом; после каждого полухода стороны сверяют хеш позиции — расхождение
//! останавливает партию. Чат живёт только в памяти (60 секунд) и не пишется
//! в логи. Криптография — чистый Rust (curve25519-dalek, chacha20poly1305,
//! blake2), без C-библиотек и внешних серверов.

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::game::{Kind, Square};
use snow::{Builder, HandshakeState, TransportState};

use crate::rulecode;

/// Порт прямого подключения (TCP).
pub const TCP_PORT: u16 = 47771;
/// Порт маяков обнаружения (UDP).
pub const UDP_PORT: u16 = 47772;
/// Сколько секунд живёт сообщение чата в памяти.
pub const CHAT_TTL_SECS: u64 = 60;
/// Максимум текста приветствия и сообщения чата.
pub const MAX_TEXT: usize = 300;
/// Максимум кадра в сети.
const MAX_FRAME: usize = 4096;
/// Таймаут одного чтения в сеансе.
const POLL_TIMEOUT: Duration = Duration::from_millis(50);
/// Таймаут рукопожатия.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
/// Таймаут прямого подключения.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

const MAGIC: &[u8; 4] = b"SMHB";
const BEACON_EVERY: Duration = Duration::from_millis(900);
const PEER_TTL: Duration = Duration::from_secs(6);

const NOISE_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
const GREETING_VERSION: u8 = 1;

const MSG_SEED: u8 = 2;
const MSG_MOVE: u8 = 3;
const MSG_HASH: u8 = 4;
const MSG_PAUSE: u8 = 5;
const MSG_CHAT: u8 = 6;
const MSG_BYE: u8 = 8;
const MSG_CHAIN_END: u8 = 9;
const MSG_PROMO_FREE: u8 = 10;

// ---------------------------------------------------------------------------
// Личность игрока
// ---------------------------------------------------------------------------

/// Постоянная личность: пара ключей Noise и короткий ID.
#[derive(Clone)]
pub struct Identity {
    pub private: [u8; 32],
    /// Открытый ключ Noise — уже проверяется в рукопожатии.
    #[allow(dead_code)]
    pub public: [u8; 32],
    /// 8 символов base32 из первых байт открытого ключа.
    pub id: String,
}

impl Identity {
    /// Загружает ключи из `net/keys.bin` рядом с игрой или создаёт их.
    /// ID выводится из ключа, поэтому отдельного файла ID не нужно.
    pub fn load_or_create(dir: &PathBuf) -> Identity {
        if let Ok(bytes) = std::fs::read(dir.join("keys.bin")) {
            if bytes.len() == 64 {
                let mut private = [0u8; 32];
                let mut public = [0u8; 32];
                private.copy_from_slice(&bytes[..32]);
                public.copy_from_slice(&bytes[32..]);
                if let Some(id) = id_from_public(&public) {
                    return Identity { private, public, id };
                }
            }
        }
        let params: snow::params::NoiseParams =
            NOISE_PATTERN.parse().expect("известный шаблон Noise");
        let kp = Builder::new(params)
            .generate_keypair()
            .expect("генерация ключей");
        let mut private = [0u8; 32];
        let mut public = [0u8; 32];
        private.copy_from_slice(&kp.private);
        public.copy_from_slice(&kp.public);
        let mut bytes = Vec::with_capacity(64);
        bytes.extend_from_slice(&private);
        bytes.extend_from_slice(&public);
        let _ = std::fs::create_dir_all(dir);
        let _ = std::fs::write(dir.join("keys.bin"), bytes);
        let id = id_from_public(&public).expect("валидный ключ");
        Identity { private, public, id }
    }

    /// ID для показа: `XXXX-XXXX`.
    pub fn id_display(&self) -> String {
        display_id(&self.id)
    }
}

/// ID из открытого ключа: первые 5 байт в base32.
pub fn id_from_public(public: &[u8]) -> Option<String> {
    if public.len() < 5 {
        return None;
    }
    Some(rulecode::base32(&public[..5]))
}

/// `XXXXXXXX` → `XXXX-XXXX` для показа.
pub fn display_id(id: &str) -> String {
    if id.len() == 8 {
        format!("{}-{}", &id[..4], &id[4..])
    } else {
        id.to_string()
    }
}

/// Проверяет, что строка — допустимый ID.
pub fn valid_id(id: &str) -> bool {
    rulecode::decode_id(id).is_some()
}

// ---------------------------------------------------------------------------
// Обнаружение игроков в локальной сети
// ---------------------------------------------------------------------------

/// Игрок, замеченный в локальной сети.
#[derive(Clone, Debug)]
pub struct PeerInfo {
    pub id: String,
    pub addr: SocketAddr,
    pub last_seen: Instant,
}

/// Фоновое обнаружение: маяки раз в секунду, список обновляется сам.
pub struct Discovery {
    peers: Arc<Mutex<HashMap<String, PeerInfo>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Discovery {
    fn start(identity: &Identity) -> io::Result<Discovery> {
        let socket = open_discovery_socket()?;
        let peers = Arc::new(Mutex::new(HashMap::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let my_id = identity.id.clone();
        let tcp_port = TCP_PORT;
        let peers2 = peers.clone();
        let stop2 = stop.clone();
        let thread = thread::spawn(move || {
            discovery_loop(socket, my_id, tcp_port, peers2, stop2);
        });
        Ok(Discovery {
            peers,
            stop,
            thread: Some(thread),
        })
    }

    /// Свежий список игроков рядом.
    pub fn list(&self) -> Vec<PeerInfo> {
        let mut peers = self.peers.lock().unwrap();
        peers.retain(|_, p| p.last_seen.elapsed() < PEER_TTL);
        let mut out: Vec<PeerInfo> = peers.values().cloned().collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// UDP-сокет на общем порту: SO_REUSEADDR позволяет нескольким копиям игры
/// на одной машине одновременно получать маяки.
fn open_discovery_socket() -> io::Result<UdpSocket> {
    let sock = socket2::Socket::new(
        socket2::Domain::IPV4,
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )?;
    let _ = sock.set_reuse_address(true);
    let _ = sock.set_broadcast(true);
    let _ = sock.set_nonblocking(true);
    let addr: std::net::SocketAddr = (Ipv4Addr::UNSPECIFIED, UDP_PORT).into();
    sock.bind(&addr.into())?;
    Ok(UdpSocket::from(sock))
}

fn discovery_loop(
    socket: UdpSocket,
    my_id: String,
    tcp_port: u16,
    peers: Arc<Mutex<HashMap<String, PeerInfo>>>,
    stop: Arc<AtomicBool>,
) {
    let mut last_beacon = Instant::now() - BEACON_EVERY;
    let mut last_prune = Instant::now();
    let mut buf = [0u8; 128];
    loop {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        if last_beacon.elapsed() >= BEACON_EVERY {
            let beacon = encode_beacon(&my_id, tcp_port);
            let _ = socket.send_to(&beacon, (Ipv4Addr::BROADCAST, UDP_PORT));
            // Для двух копий на одной машине broadcast может не дойти —
            // дублируем на loopback.
            let _ = socket.send_to(&beacon, (Ipv4Addr::LOCALHOST, UDP_PORT));
            last_beacon = Instant::now();
        }
        loop {
            match socket.recv_from(&mut buf) {
                Ok((n, src)) => {
                    if let Some((id, port)) = parse_beacon(&buf[..n]) {
                        if id != my_id {
                            let info = PeerInfo {
                                id: id.clone(),
                                addr: SocketAddr::new(src.ip(), port),
                                last_seen: Instant::now(),
                            };
                            peers.lock().unwrap().insert(id, info);
                        }
                    }
                }
                Err(_) => break,
            }
        }
        if last_prune.elapsed() >= PEER_TTL {
            peers.lock().unwrap().retain(|_, p| p.last_seen.elapsed() < PEER_TTL);
            last_prune = Instant::now();
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn encode_beacon(id: &str, tcp_port: u16) -> Vec<u8> {
    let mut b = Vec::with_capacity(14);
    b.extend_from_slice(MAGIC);
    b.extend_from_slice(id.as_bytes());
    b.extend_from_slice(&tcp_port.to_be_bytes());
    b
}

fn parse_beacon(bytes: &[u8]) -> Option<(String, u16)> {
    if bytes.len() != 14 || &bytes[..4] != MAGIC {
        return None;
    }
    let id = std::str::from_utf8(&bytes[4..12]).ok()?;
    if !valid_id(id) {
        return None;
    }
    let port = u16::from_be_bytes([bytes[12], bytes[13]]);
    Some((id.to_string(), port))
}

// ---------------------------------------------------------------------------
// Протокол
// ---------------------------------------------------------------------------

/// Роль в партии: приглашающий (хост) играет белыми.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Host,
    Joiner,
}

impl Role {
    /// Сторона этой роли в партии.
    pub fn side(self) -> crate::game::Side {
        match self {
            Role::Host => crate::game::Side::White,
            Role::Joiner => crate::game::Side::Black,
        }
    }
}

/// Приветствие — единственное сообщение каждой стороны до начала партии.
#[derive(Clone, Debug)]
pub struct Greeting {
    pub id: String,
    pub rules: rulecode::Rules,
    pub text: String,
}

fn encode_greeting(g: &Greeting) -> Vec<u8> {
    let code = rulecode::encode(&g.rules);
    let mut b = Vec::with_capacity(21 + MAX_TEXT);
    b.push(GREETING_VERSION);
    b.extend_from_slice(g.id.as_bytes());
    b.extend_from_slice(code.as_bytes());
    let text = truncate_utf8(&g.text, MAX_TEXT);
    b.extend_from_slice(&(text.len() as u16).to_le_bytes());
    b.extend_from_slice(text);
    b
}

fn decode_greeting(bytes: &[u8]) -> io::Result<Greeting> {
    if bytes.len() < 19 || bytes[0] != GREETING_VERSION {
        return Err(bad("приветствие"));
    }
    let id = std::str::from_utf8(&bytes[1..9]).map_err(|_| bad("ID"))?;
    if !valid_id(id) {
        return Err(bad("ID"));
    }
    let code = std::str::from_utf8(&bytes[9..17]).map_err(|_| bad("код правил"))?;
    let rules = rulecode::decode(code).ok_or_else(|| bad("код правил"))?;
    let text_len = u16::from_le_bytes([bytes[17], bytes[18]]) as usize;
    let rest = &bytes[19..];
    if text_len > rest.len() {
        return Err(bad("приветствие"));
    }
    let text = std::str::from_utf8(&rest[..text_len]).map_err(|_| bad("текст"))?;
    Ok(Greeting {
        id: id.to_string(),
        rules,
        text: text.to_string(),
    })
}

fn bad(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("неверный {what}"))
}

fn truncate_utf8(s: &str, max: usize) -> &[u8] {
    let bytes = s.as_bytes();
    if bytes.len() <= max {
        return bytes;
    }
    let mut end = max;
    while end > 0 && (bytes[end] & 0xC0) == 0x80 {
        end -= 1;
    }
    &bytes[..end]
}

/// Сообщение интерфейса → сеть.
#[derive(Clone, Debug)]
pub enum OutMsg {
    Seed(u64),
    Move {
        from: Square,
        to: Square,
        captured: Option<Square>,
        promo: Option<Kind>,
    },
    Hash {
        ply: u32,
        hash: u64,
    },
    Pause(bool),
    Chat(String),
    Bye(String),
    /// Завершить серию взятий без хода (когда взятие необязательно).
    ChainEnd,
    /// Свободное превращение после сумерек: игрок выбрал фигуру без хода.
    PromoFree { sq: Square, kind: Kind },
}

/// Событие сети → интерфейс.
#[derive(Clone, Debug)]
pub enum NetEvent {
    Connected {
        role: Role,
        peer_id: String,
        rules: rulecode::Rules,
        text: String,
    },
    Seed(u64),
    Move {
        from: Square,
        to: Square,
        captured: Option<Square>,
        promo: Option<Kind>,
    },
    Hash {
        /// Полуход — для протокола; сверка идёт по хешу позиции.
        #[allow(dead_code)]
        ply: u32,
        hash: u64,
    },
    Pause(bool),
    Chat(String),
    Bye(String),
    /// Завершить серию взятий без хода (когда взятие необязательно).
    ChainEnd,
    /// Свободное превращение после сумерек: игрок выбрал фигуру без хода.
    PromoFree { sq: Square, kind: Kind },
    Disconnected(String),
    Failed(String),
}

fn sq_byte(sq: Square) -> u8 {
    (sq.0 * 8 + sq.1) as u8
}

fn sq_from_byte(b: u8) -> Option<Square> {
    if b == 0xFF {
        return None;
    }
    let f = (b / 8) as usize;
    let r = (b % 8) as usize;
    if f < 8 && r < 8 {
        Some((f, r))
    } else {
        None
    }
}

fn kind_byte(kind: Kind) -> u8 {
    match kind {
        Kind::Queen => 1,
        Kind::Rook => 2,
        Kind::Bishop => 3,
        Kind::Knight => 4,
        _ => 0,
    }
}

fn kind_from_byte(b: u8) -> Option<Kind> {
    match b {
        1 => Some(Kind::Queen),
        2 => Some(Kind::Rook),
        3 => Some(Kind::Bishop),
        4 => Some(Kind::Knight),
        _ => None,
    }
}

fn encode_out(msg: &OutMsg) -> Vec<u8> {
    match msg {
        OutMsg::Seed(seed) => {
            let mut b = vec![MSG_SEED];
            b.extend_from_slice(&seed.to_le_bytes());
            b
        }
        OutMsg::Move {
            from,
            to,
            captured,
            promo,
        } => vec![
            MSG_MOVE,
            sq_byte(*from),
            sq_byte(*to),
            captured.map(sq_byte).unwrap_or(0xFF),
            promo.map(kind_byte).unwrap_or(0),
        ],
        OutMsg::Hash { ply, hash } => {
            let mut b = vec![MSG_HASH];
            b.extend_from_slice(&ply.to_le_bytes());
            b.extend_from_slice(&hash.to_le_bytes());
            b
        }
        OutMsg::Pause(on) => vec![MSG_PAUSE, *on as u8],
        OutMsg::Chat(text) => text_msg(MSG_CHAT, text),
        OutMsg::Bye(reason) => text_msg(MSG_BYE, reason),
        OutMsg::ChainEnd => vec![MSG_CHAIN_END],
        OutMsg::PromoFree { sq, kind } => vec![MSG_PROMO_FREE, sq_byte(*sq), kind_byte(*kind)],
    }
}

fn text_msg(kind: u8, text: &str) -> Vec<u8> {
    let bytes = truncate_utf8(text, MAX_TEXT);
    let mut b = Vec::with_capacity(3 + bytes.len());
    b.push(kind);
    b.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
    b.extend_from_slice(bytes);
    b
}

fn decode_event(bytes: &[u8]) -> Option<NetEvent> {
    let (&t, rest) = bytes.split_first()?;
    match t {
        MSG_SEED => {
            let (seed, _) = rest.split_first_chunk::<8>()?;
            Some(NetEvent::Seed(u64::from_le_bytes(*seed)))
        }
        MSG_MOVE => {
            if rest.len() < 4 {
                return None;
            }
            let from = sq_from_byte(rest[0])?;
            let to = sq_from_byte(rest[1])?;
            let captured = sq_from_byte(rest[2]);
            let promo = kind_from_byte(rest[3]);
            Some(NetEvent::Move {
                from,
                to,
                captured,
                promo,
            })
        }
        MSG_HASH => {
            let (ply, rest) = rest.split_first_chunk::<4>()?;
            let (hash, _) = rest.split_first_chunk::<8>()?;
            Some(NetEvent::Hash {
                ply: u32::from_le_bytes(*ply),
                hash: u64::from_le_bytes(*hash),
            })
        }
        MSG_PAUSE => {
            if rest.len() != 1 {
                return None;
            }
            Some(NetEvent::Pause(rest[0] != 0))
        }
        MSG_CHAT => parse_text(rest).map(NetEvent::Chat),
        MSG_BYE => {
            let reason = parse_text(rest).unwrap_or_default();
            Some(NetEvent::Bye(if reason.is_empty() {
                "закрыт соперником".into()
            } else {
                reason
            }))
        }
        MSG_CHAIN_END => Some(NetEvent::ChainEnd),
        MSG_PROMO_FREE => {
            if rest.len() != 2 {
                return None;
            }
            let sq = sq_from_byte(rest[0])?;
            let kind = kind_from_byte(rest[1])?;
            Some(NetEvent::PromoFree { sq, kind })
        }
        _ => None,
    }
}

fn parse_text(rest: &[u8]) -> Option<String> {
    let (len_b, text) = rest.split_first_chunk::<2>()?;
    let len = u16::from_le_bytes(*len_b) as usize;
    if len > text.len() {
        return None;
    }
    Some(String::from_utf8_lossy(&text[..len]).into_owned())
}

/// Хеш позиции: доска, очередь хода, фаза, полуход и состояние генератора.
/// Обе стороны считают одинаково — расхождение означает разъехавшиеся партии.
pub fn position_hash(g: &crate::game::Game) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for row in &g.board {
        for cell in row {
            match cell {
                Some(p) => {
                    mix(&mut h, 1);
                    mix(&mut h, p.side as u64);
                    mix(&mut h, p.kind as u64);
                    mix(&mut h, p.crowned as u64);
                }
                None => mix(&mut h, 0),
            }
        }
    }
    mix(&mut h, g.turn as u64);
    mix(&mut h, g.phase as u64);
    mix(&mut h, g.ply as u64);
    mix(&mut h, g.rng_state);
    h
}

fn mix(h: &mut u64, v: u64) {
    *h ^= v.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    *h = h.rotate_left(27);
    *h = h.wrapping_mul(0x2545_F491_4F6C_DD1D);
}

// ---------------------------------------------------------------------------
// Кадры и рукопожатие
// ---------------------------------------------------------------------------

struct FrameReader {
    stream: TcpStream,
    buf: Vec<u8>,
}

impl FrameReader {
    fn new(stream: TcpStream) -> Self {
        FrameReader {
            stream,
            buf: Vec::new(),
        }
    }

    fn write_frame(&mut self, data: &[u8]) -> io::Result<()> {
        let len: u16 = data
            .len()
            .try_into()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "кадр велик"))?;
        self.stream.write_all(&len.to_be_bytes())?;
        self.stream.write_all(data)?;
        self.stream.flush()
    }

    /// Читает один кадр. Ok(None) — данных пока нет; Err — разрыв или мусор.
    fn read_frame(&mut self, timeout: Duration) -> io::Result<Option<Vec<u8>>> {
        self.stream.set_read_timeout(Some(timeout))?;
        loop {
            if self.buf.len() >= 2 {
                let len = u16::from_be_bytes([self.buf[0], self.buf[1]]) as usize;
                if len > MAX_FRAME {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "кадр велик"));
                }
                if self.buf.len() >= 2 + len {
                    let frame: Vec<u8> = self.buf[2..2 + len].to_vec();
                    self.buf.drain(..2 + len);
                    return Ok(Some(frame));
                }
            }
            let mut tmp = [0u8; 2048];
            match self.stream.read(&mut tmp) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "соединение закрыто",
                    ))
                }
                Ok(n) => self.buf.extend_from_slice(&tmp[..n]),
                Err(e)
                    if e.kind() == io::ErrorKind::WouldBlock
                        || e.kind() == io::ErrorKind::TimedOut =>
                {
                    return Ok(None)
                }
                Err(e) => return Err(e),
            }
        }
    }
}

fn noise_err(e: snow::Error) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e.to_string())
}

fn hs_write(stream: &mut TcpStream, hs: &mut HandshakeState, payload: &[u8]) -> io::Result<()> {
    let mut buf = vec![0u8; payload.len() + 160];
    let n = hs.write_message(payload, &mut buf).map_err(noise_err)?;
    let len: u16 = n
        .try_into()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "кадр велик"))?;
    stream.write_all(&len.to_be_bytes())?;
    stream.write_all(&buf[..n])?;
    stream.flush()
}

fn hs_read(stream: &mut TcpStream, hs: &mut HandshakeState) -> io::Result<Vec<u8>> {
    let mut len_b = [0u8; 2];
    stream.read_exact(&mut len_b)?;
    let len = u16::from_be_bytes(len_b) as usize;
    if len > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "кадр велик"));
    }
    let mut frame = vec![0u8; len];
    stream.read_exact(&mut frame)?;
    let mut out = vec![0u8; frame.len()];
    let n = hs.read_message(&frame, &mut out).map_err(noise_err)?;
    out.truncate(n);
    Ok(out)
}

/// Рукопожатие Noise_XX: обмен приветствиями, транспортное состояние.
fn handshake(
    stream: &mut TcpStream,
    identity: &Identity,
    role: Role,
    greeting: &Greeting,
) -> io::Result<(TransportState, Greeting)> {
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT));
    let _ = stream.set_write_timeout(Some(HANDSHAKE_TIMEOUT));
    let params: snow::params::NoiseParams =
        NOISE_PATTERN.parse().expect("известный шаблон Noise");
    let builder = Builder::new(params)
        .local_private_key(&identity.private)
        .map_err(noise_err)?;
    let mut hs = match role {
        Role::Host => builder.build_responder(),
        Role::Joiner => builder.build_initiator(),
    }
    .map_err(noise_err)?;
    let my = encode_greeting(greeting);
    let their = match role {
        Role::Joiner => {
            hs_write(stream, &mut hs, b"")?;
            let msg = hs_read(stream, &mut hs)?;
            let their = decode_greeting(&msg)?;
            hs_write(stream, &mut hs, &my)?;
            their
        }
        Role::Host => {
            let _ = hs_read(stream, &mut hs)?;
            hs_write(stream, &mut hs, &my)?;
            let msg = hs_read(stream, &mut hs)?;
            decode_greeting(&msg)?
        }
    };
    let transport = hs.into_transport_mode().map_err(noise_err)?;
    Ok((transport, their))
}

/// Заявленный ID обязан совпадать с настоящим ключом соперника.
fn verify_peer(noise: &TransportState, claimed_id: &str) -> Result<(), String> {
    let Some(remote) = noise.get_remote_static() else {
        return Err("соперник не передал ключ".into());
    };
    match id_from_public(remote) {
        Some(real) if real == claimed_id => Ok(()),
        Some(real) => Err(format!(
            "ID соперника не совпадает с его ключом: {} ≠ {}",
            display_id(claimed_id),
            display_id(&real)
        )),
        None => Err("некорректный ключ соперника".into()),
    }
}

// ---------------------------------------------------------------------------
// Сеанс
// ---------------------------------------------------------------------------

/// Сведения об активном сеансе.
#[derive(Clone, Debug)]
pub struct SessionInfo {
    /// Роль в сеансе: сторона уже выводится из неё в интерфейсе.
    #[allow(dead_code)]
    pub role: Role,
    pub peer_id: String,
    pub peer_addr: SocketAddr,
}

struct NetState {
    outbox: Option<Sender<OutMsg>>,
    stop: Option<Arc<AtomicBool>>,
    session: Option<SessionInfo>,
    greeting: String,
    rules: rulecode::Rules,
}

impl Default for NetState {
    fn default() -> Self {
        NetState {
            outbox: None,
            stop: None,
            session: None,
            greeting: String::new(),
            rules: rulecode::Rules::from_game(&crate::game::Game::new(20.0)),
        }
    }
}

/// Сетевой контекст приложения: обнаружение, сеанс, события.
pub struct Net {
    pub identity: Identity,
    pub discovery: Option<Discovery>,
    events_tx: Sender<NetEvent>,
    events_rx: Receiver<NetEvent>,
    state: Arc<Mutex<NetState>>,
    host_stop: Arc<AtomicBool>,
    host_thread: Option<JoinHandle<()>>,
}

impl Net {
    /// Создаёт контекст. None — нет доступа к исполняемому файлу.
    pub fn new() -> Option<Net> {
        let dir = data_dir()?;
        let identity = Identity::load_or_create(&dir);
        let (tx, rx) = mpsc::channel();
        let discovery = Discovery::start(&identity).ok();
        Some(Net {
            identity,
            discovery,
            events_tx: tx,
            events_rx: rx,
            state: Arc::new(Mutex::new(NetState::default())),
            host_stop: Arc::new(AtomicBool::new(false)),
            host_thread: None,
        })
    }

    pub fn my_id(&self) -> String {
        self.identity.id_display()
    }

    /// Игроки рядом.
    pub fn peers(&self) -> Vec<PeerInfo> {
        match &self.discovery {
            Some(d) => d.list(),
            None => Vec::new(),
        }
    }

    pub fn set_greeting(&self, text: String) {
        self.state.lock().unwrap().greeting = text;
    }

    /// Текущие правила партии — они уходят в приветствии.
    pub fn set_rules(&self, rules: rulecode::Rules) {
        self.state.lock().unwrap().rules = rules;
    }

    pub fn session(&self) -> Option<SessionInfo> {
        self.state.lock().unwrap().session.clone()
    }

    /// Начать ждать подключения.
    pub fn host(&mut self) {
        if self.session().is_some() || self.host_thread.is_some() {
            return;
        }
        self.host_stop = Arc::new(AtomicBool::new(false));
        let identity = self.identity.clone();
        let tx = self.events_tx.clone();
        let state = self.state.clone();
        let stop = self.host_stop.clone();
        self.host_thread = Some(thread::spawn(move || {
            host_loop(identity, tx, state, stop);
        }));
    }

    /// Отменить ожидание.
    pub fn cancel_hosting(&mut self) {
        self.host_stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.host_thread.take() {
            let _ = t.join();
        }
    }

    pub fn hosting(&self) -> bool {
        self.host_thread
            .as_ref()
            .map(|t| !t.is_finished())
            .unwrap_or(false)
    }

    /// Подключиться по адресу.
    pub fn connect(&mut self, addr: SocketAddr) {
        if self.session().is_some() {
            return;
        }
        // исходящее соединение и ожидание не совмещаются
        self.cancel_hosting();
        let identity = self.identity.clone();
        let tx = self.events_tx.clone();
        let state = self.state.clone();
        let greeting = self.greeting();
        let rules = self.state.lock().unwrap().rules;
        thread::spawn(move || {
            let stream =
                match TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT) {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = tx.send(NetEvent::Failed(format!(
                            "нет связи с {}: {}",
                            addr,
                            e
                        )));
                        return;
                    }
                };
            establish(stream, &identity, Role::Joiner, &greeting, rules, tx, state);
        });
    }

    /// Отправить сообщение сопернику.
    pub fn send(&self, msg: OutMsg) {
        let outbox = self.state.lock().unwrap().outbox.clone();
        if let Some(tx) = outbox {
            let _ = tx.send(msg);
        }
    }

    /// События, накопившиеся за кадр.
    pub fn poll(&self) -> Vec<NetEvent> {
        let mut out = Vec::new();
        while let Ok(ev) = self.events_rx.try_recv() {
            out.push(ev);
        }
        out
    }

    /// Закрыть сеанс: прощание и разрыв.
    pub fn close_session(&mut self, reason: &str) {
        let (outbox, stop) = {
            let mut st = self.state.lock().unwrap();
            (st.outbox.take(), st.stop.take())
        };
        if let Some(tx) = &outbox {
            let _ = tx.send(OutMsg::Bye(reason.to_string()));
        }
        if let Some(stop) = stop {
            stop.store(true, Ordering::SeqCst);
        }
        self.state.lock().unwrap().session = None;
    }

    fn greeting(&self) -> String {
        self.state.lock().unwrap().greeting.clone()
    }

    /// Остановить всё перед выходом.
    pub fn shutdown(&mut self) {
        self.close_session("закрыто");
        self.cancel_hosting();
        if let Some(mut d) = self.discovery.take() {
            d.stop();
        }
    }
}

impl Drop for Net {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn data_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.to_path_buf())
}

/// Завершает рукопожатие и запускает цикл сеанса.
fn establish(
    mut stream: TcpStream,
    identity: &Identity,
    role: Role,
    greeting_text: &str,
    rules: rulecode::Rules,
    tx: Sender<NetEvent>,
    state: Arc<Mutex<NetState>>,
) {
    let greeting = Greeting {
        id: identity.id.clone(),
        rules,
        text: greeting_text.to_string(),
    };
    let (noise, their) = match handshake(&mut stream, identity, role, &greeting) {
        Ok(x) => x,
        Err(e) => {
            let _ = tx.send(NetEvent::Failed(format!("рукопожатие не удалось: {}", e)));
            return;
        }
    };
    if let Err(msg) = verify_peer(&noise, &their.id) {
        let _ = tx.send(NetEvent::Failed(msg));
        return;
    }
    let peer_addr = stream
        .peer_addr()
        .unwrap_or_else(|_| SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)));
    let (out_tx, out_rx) = mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let tx2 = tx.clone();
    let stop2 = stop.clone();
    thread::spawn(move || session_loop(stream, noise, out_rx, tx2, stop2));
    {
        let mut st = state.lock().unwrap();
        st.outbox = Some(out_tx);
        st.stop = Some(stop);
        st.session = Some(SessionInfo {
            role,
            peer_id: their.id.clone(),
            peer_addr,
        });
    }
    let _ = tx.send(NetEvent::Connected {
        role,
        peer_id: their.id,
        rules: their.rules,
        text: their.text,
    });
}

fn host_loop(
    identity: Identity,
    tx: Sender<NetEvent>,
    state: Arc<Mutex<NetState>>,
    stop: Arc<AtomicBool>,
) {
    let listener = match TcpListener::bind((Ipv4Addr::UNSPECIFIED, TCP_PORT)) {
        Ok(l) => l,
        Err(e) => {
            let _ = tx.send(NetEvent::Failed(format!(
                "не удалось открыть порт {}: {}",
                TCP_PORT, e
            )));
            return;
        }
    };
    let _ = listener.set_nonblocking(true);
    loop {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        if state.lock().unwrap().session.is_some() {
            return;
        }
        match listener.accept() {
            Ok((stream, _)) => {
                let (greeting, rules) = {
                    let st = state.lock().unwrap();
                    (st.greeting.clone(), st.rules)
                };
                establish(stream, &identity, Role::Host, &greeting, rules, tx.clone(), state.clone());
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => {
                let _ = tx.send(NetEvent::Failed(format!("приём подключения: {}", e)));
                return;
            }
        }
        thread::sleep(Duration::from_millis(80));
    }
}

fn session_loop(
    stream: TcpStream,
    mut noise: TransportState,
    out_rx: Receiver<OutMsg>,
    tx: Sender<NetEvent>,
    stop: Arc<AtomicBool>,
) {
    let _ = stream.set_nodelay(true);
    let mut fr = FrameReader::new(stream);
    loop {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        let mut sent_bye = false;
        while let Ok(msg) = out_rx.try_recv() {
            let bytes = encode_out(&msg);
            let mut buf = vec![0u8; bytes.len() + 18];
            let n = match noise.write_message(&bytes, &mut buf) {
                Ok(n) => n,
                Err(e) => {
                    let _ = tx.send(NetEvent::Disconnected(format!("шифрование: {}", e)));
                    return;
                }
            };
            if let Err(e) = fr.write_frame(&buf[..n]) {
                let _ = tx.send(NetEvent::Disconnected(format!("нет связи: {}", e)));
                return;
            }
            if matches!(msg, OutMsg::Bye(_)) {
                sent_bye = true;
                break;
            }
        }
        if sent_bye {
            return;
        }
        match fr.read_frame(POLL_TIMEOUT) {
            Ok(Some(frame)) => {
                let mut payload = vec![0u8; frame.len()];
                let res = noise.read_message(&frame, &mut payload);
                match res {
                    Ok(n) => match decode_event(&payload[..n]) {
                        Some(ev) => {
                            let _ = tx.send(ev);
                        }
                        None => {
                            let _ = tx.send(NetEvent::Disconnected(
                                "повреждённое сообщение".into(),
                            ));
                            return;
                        }
                    },
                    Err(e) => {
                        let _ = tx.send(NetEvent::Disconnected(format!("шифрование: {}", e)));
                        return;
                    }
                }
            }
            Ok(None) => {}
            Err(e) => {
                let _ = tx.send(NetEvent::Disconnected(format!("нет связи: {}", e)));
                return;
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
}
