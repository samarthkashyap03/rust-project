//! The relay's wire protocol, copied from phone_controller
//! (`common/src/lib.rs`) so this project needs nothing but serde.
//!
//! Every message is one JSON text frame, tagged by `"type"`. A game sends
//! [`GameMsg`] and receives [`ToGame`] on `/ws/game`, with the header
//! `Authorization: Bearer <token>`. Phones speak [`PhoneMsg`] / [`ToPhone`]
//! on `/ws/phone/<link>`; a game never sees those.
//!
//! You do not have to understand every field. For "Print A" you need
//! `GameMsg::Hello`, `GameMsg::OpenSlot` and the `ToGame` variants.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Protocol version, sent in [`GameMsg::Hello`] and [`PhoneMsg::Claim`].
pub const PROTO: u32 = 1;

/// The relay sends each game one [`Frame`] per tick.
pub const TICK_MS: u64 = 20;
/// Phones send a full snapshot at least this often, even when nothing changed.
pub const PHONE_HEARTBEAT_MS: u64 = 250;
/// A phone silent for this long counts as disconnected.
pub const PHONE_SILENCE_MS: u64 = 1000;
/// A game's session survives a dropped game connection this long.
pub const RESUME_GRACE_MS: u64 = 30_000;
/// Edges kept per slot between two sent frames; more sets
/// [`SlotState::overflow`].
pub const MAX_EDGES_PER_FRAME: usize = 64;
/// Motion samples kept per phone message and per slot between two sent
/// frames; older ones are dropped first (the angles are cumulative, so the
/// newest sample alone still gives the total movement).
pub const MAX_MOTION_SAMPLES: usize = 32;
pub const MAX_SLOTS: usize = 16;
/// Longest [`valid_display_name`], in characters.
pub const MAX_DISPLAY_NAME_CHARS: usize = 24;
pub const MAX_PHONE_MSG_BYTES: usize = 4 * 1024;
pub const MAX_GAME_MSG_BYTES: usize = 16 * 1024;

/// WebSocket close codes the relay uses; the reason text is human readable.
pub mod close {
    pub const BAD_TOKEN: u16 = 4001;
    pub const PROTOCOL: u16 = 4002;
    pub const SESSION_GONE: u16 = 4004;
    pub const CLAIMED: u16 = 4009;
    pub const REPLACED: u16 = 4010;
    pub const REMOVED: u16 = 4011;
    pub const ENDED: u16 = 4012;
    pub const TOO_MANY: u16 = 4029;
    pub const SILENT: u16 = 4008;
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Button {
    A,
    B,
    X,
    Y,
    Menu,
    Up,
    Down,
    Left,
    Right,
}

impl Button {
    pub const ALL: [Button; 9] = [
        Button::A,
        Button::B,
        Button::X,
        Button::Y,
        Button::Menu,
        Button::Up,
        Button::Down,
        Button::Left,
        Button::Right,
    ];
}

/// A press (`down: true`) or release, in the order it happened.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Edge {
    pub button: Button,
    pub down: bool,
}

/// Stick positions, each `[x, y]` in −1…1 with +x right and +y down; the phone
/// applies the dead zone.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Default, Debug)]
pub struct Sticks {
    pub left: [f32; 2],
    pub right: [f32; 2],
}

/// Phone attitude. `roll`/`pitch` are in degrees relative to the phone held in
/// landscape, corrected for the screen orientation: roll > 0 = tilted right,
/// pitch > 0 = top edge away from the player. The raw DeviceOrientation angles
/// ride along unchanged.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Default, Debug)]
pub struct Tilt {
    pub roll: f32,
    pub pitch: f32,
    pub alpha: f32,
    pub beta: f32,
    pub gamma: f32,
}

/// One gyroscope reading, integrated on the phone. The angles are cumulative
/// since the phone started sending motion (they never wrap), so the change
/// between any two samples is the rotation in between, even if samples in
/// the middle were dropped.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Default, Debug)]
pub struct MotionSample {
    /// Phone clock (`performance.now()`, ms) of the sensor event.
    pub t: f64,
    /// Degrees turned about the world vertical; grows when the phone turns
    /// left (counter-clockwise seen from above), like `Tilt::alpha`.
    pub yaw: f64,
    /// Degrees turned about the phone's own left-right axis; grows when the
    /// top edge goes down (away from the player's view), like `Tilt::pitch`.
    pub pitch: f64,
}

/// How the phones show the controller. A game picks one for all its phones in
/// [`GameMsg::Hello`]; the relay passes it on in [`ToPhone::Welcome`].
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    /// Follows the screen: portrait or landscape, as the phone is held.
    #[default]
    Any,
    /// Always portrait; turning the phone does not turn the controller.
    Portrait,
    /// Always landscape, also on a phone locked to portrait.
    Landscape,
}

/// Everything a phone knows about its controls right now, plus the edges since
/// its previous message.
#[derive(Serialize, Deserialize, Clone, PartialEq, Default, Debug)]
pub struct PhoneInput {
    /// +1 per message from this phone.
    pub pseq: u64,
    pub sticks: Sticks,
    pub held: Vec<Button>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<Edge>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tilt: Option<Tilt>,
    /// Phone clock (`performance.now()`, ms) when the message was sent.
    #[serde(default)]
    pub t: f64,
    /// Gyroscope samples since the previous message, oldest first, at most
    /// [`MAX_MOTION_SAMPLES`]; empty without a gyroscope.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub motion: Vec<MotionSample>,
}

/// Phone → relay.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PhoneMsg {
    /// First message on every connection. `secret` is 128 random bits the
    /// phone generated the first time it opened this link; the relay binds
    /// the link to it.
    Claim {
        proto: u32,
        secret: String,
        ui_version: String,
    },
    Input(PhoneInput),
}

/// Relay → phone. The controller is game-agnostic: besides whether its claim
/// worked (a close code tells it why not), all it learns from the game is the
/// session's [`Layout`] and its slot's display name, which tells the phone's
/// user which player they are.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToPhone {
    Welcome {
        #[serde(default)]
        layout: Layout,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        display_name: Option<String>,
    },
    /// The slot's display name changed after `Welcome` (`None`: removed).
    DisplayName {
        #[serde(default)]
        display_name: Option<String>,
    },
}

/// Game → relay.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GameMsg {
    /// First message of a new session.
    Hello {
        proto: u32,
        #[serde(default)]
        layout: Layout,
    },
    /// First message when reattaching to a session within the grace period.
    Resume {
        session: String,
        secret: String,
    },
    /// Returns the slot's existing link if it has one, else creates it.
    OpenSlot {
        req_id: u64,
        slot: String,
    },
    /// Sets (`None`: removes) the name the slot's phone shows, now and on
    /// every later claim; see [`valid_display_name`]. No reply unless it fails.
    SetDisplayName {
        slot: String,
        #[serde(default)]
        display_name: Option<String>,
    },
    /// Replaces the slot's link; the phone holding the old one is disconnected.
    ReissueLink {
        req_id: u64,
        slot: String,
    },
    /// Ends the slot's link and removes the slot.
    CancelLink {
        req_id: u64,
        slot: String,
    },
    EndSession,
}

/// Relay → game.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToGame {
    /// Answer to `Hello` and to a successful `Resume`. `secret` changes on
    /// every resume.
    Session {
        session: String,
        secret: String,
        resumed: bool,
    },
    SlotLink {
        req_id: u64,
        slot: String,
        url: String,
    },
    LinkCancelled {
        req_id: u64,
        slot: String,
    },
    PhoneConnected {
        slot: String,
    },
    PhoneDisconnected {
        slot: String,
    },
    Error {
        req_id: Option<u64>,
        message: String,
    },
    Frame(Frame),
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
pub struct Frame {
    /// +1 per relay tick; a gap means ticks were skipped, never that edges
    /// were lost.
    pub seq: u64,
    /// Relay clock since the session started.
    pub t_ms: u64,
    /// Set on the first frame after a resume: edges from the gap are gone.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub resync: bool,
    pub slots: BTreeMap<String, SlotState>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
pub struct SlotState {
    /// A phone holds the link and has sent something within
    /// [`PHONE_SILENCE_MS`].
    pub connected: bool,
    /// Milliseconds since the phone's last message (0 if it never sent one).
    pub age_ms: u64,
    pub sticks: Sticks,
    pub held: Vec<Button>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<Edge>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub overflow: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tilt: Option<Tilt>,
    /// `PhoneInput::t` of the phone's latest message (phone clock, ms).
    #[serde(default)]
    pub phone_t: f64,
    /// Every motion sample since the previous sent frame, oldest first, at
    /// most [`MAX_MOTION_SAMPLES`] (older ones dropped).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub motion: Vec<MotionSample>,
}

/// Slot names: `[a-z0-9_-]{1,32}`.
pub fn valid_slot(s: &str) -> bool {
    let allowed = |b: u8| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-');
    !s.is_empty() && s.len() <= 32 && s.bytes().all(allowed)
}

/// Display names: 1 to [`MAX_DISPLAY_NAME_CHARS`] characters, no control
/// characters, no leading or trailing whitespace.
pub fn valid_display_name(s: &str) -> bool {
    let n = s.chars().count();
    let no_control = !s.chars().any(char::is_control);
    (1..=MAX_DISPLAY_NAME_CHARS).contains(&n) && no_control && s.trim() == s
}
