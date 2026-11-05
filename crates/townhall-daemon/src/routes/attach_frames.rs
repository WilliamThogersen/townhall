//! What a client sends over the attach socket: keystrokes for the terminal, or one JSON
//! text frame that resizes it.

use axum::extract::ws::Message;
use serde::Deserialize;

/// Smallest and largest terminal side a client may ask for.
const SIDE_RANGE: std::ops::RangeInclusive<u64> = 2..=1000;

/// One client frame, read.
#[derive(Debug, PartialEq, Eq)]
pub enum Frame {
    /// Bytes to type into the terminal.
    Input(Vec<u8>),
    /// A new terminal size.
    Resize { cols: u16, rows: u16 },
    /// Nothing to do: ping, pong, a malformed or out of range resize.
    Ignore,
    /// The client closed the socket.
    Close,
}

#[derive(Deserialize)]
struct ResizeFrame {
    #[serde(rename = "type")]
    kind: String,
    cols: serde_json::Value,
    rows: serde_json::Value,
}

fn side(value: &serde_json::Value) -> Option<u16> {
    let n = value.as_u64().filter(|n| SIDE_RANGE.contains(n))?;
    u16::try_from(n).ok()
}

/// A resize frame, or `None` when the text is not one. `Some(None)` is a resize whose
/// numbers are not integers in 2..=1000.
fn parse_resize(text: &str) -> Option<Option<(u16, u16)>> {
    let frame: ResizeFrame = serde_json::from_str(text).ok()?;
    if frame.kind != "resize" {
        return None;
    }
    Some(side(&frame.cols).zip(side(&frame.rows)))
}

fn text_frame(text: &str) -> Frame {
    match parse_resize(text) {
        Some(Some((cols, rows))) => Frame::Resize { cols, rows },
        Some(None) => Frame::Ignore,
        None => Frame::Input(text.as_bytes().to_vec()),
    }
}

/// Reads one websocket message from the client.
pub fn read(message: Message) -> Frame {
    match message {
        Message::Text(text) => text_frame(text.as_str()),
        Message::Binary(bytes) => Frame::Input(bytes.to_vec()),
        Message::Close(_) => Frame::Close,
        Message::Ping(_) | Message::Pong(_) => Frame::Ignore,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(t: &str) -> Frame {
        read(Message::text(t))
    }

    #[test]
    fn a_resize_frame_gives_the_size() {
        assert_eq!(
            text(r#"{"type":"resize","cols":140,"rows":40}"#),
            Frame::Resize {
                cols: 140,
                rows: 40
            }
        );
    }

    #[test]
    fn sizes_outside_2_to_1000_or_not_integers_are_ignored() {
        for bad in [
            r#"{"type":"resize","cols":1,"rows":40}"#,
            r#"{"type":"resize","cols":140,"rows":1001}"#,
            r#"{"type":"resize","cols":-5,"rows":40}"#,
            r#"{"type":"resize","cols":80.5,"rows":40}"#,
            r#"{"type":"resize","cols":"80","rows":40}"#,
            r#"{"type":"resize","cols":null,"rows":40}"#,
        ] {
            assert_eq!(text(bad), Frame::Ignore, "{bad}");
        }
        assert_eq!(
            text(r#"{"type":"resize","cols":2,"rows":1000}"#),
            Frame::Resize {
                cols: 2,
                rows: 1000
            }
        );
    }

    #[test]
    fn other_text_is_typed_as_it_is() {
        for typed in ["ls\r", "{", r#"{"type":"other"}"#, r#"{"cols":1}"#, ""] {
            assert_eq!(text(typed), Frame::Input(typed.as_bytes().to_vec()));
        }
    }

    #[test]
    fn binary_is_typed_and_close_ends() {
        assert_eq!(
            read(Message::Binary(vec![0x1b, b'[', b'A'].into())),
            Frame::Input(vec![0x1b, b'[', b'A'])
        );
        assert_eq!(read(Message::Close(None)), Frame::Close);
        assert_eq!(read(Message::Ping(vec![].into())), Frame::Ignore);
    }
}
