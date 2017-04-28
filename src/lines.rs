use bytes::BytesMut;
use tokio_io::codec::{Encoder, Decoder};
use std::io;
use std::str;

/// An empty struct that serves as a hook to hang our codec implementation
/// on.
pub struct LineCodec;

impl LineCodec {
    pub fn new() -> LineCodec {
        LineCodec {}
    }
}

/// Implements a newline-delimited framing algorithm.
impl Encoder for LineCodec {
    type Item = String;
    type Error = io::Error;

    fn encode(&mut self, msg: String, buf: &mut BytesMut) -> io::Result<()> {
        buf.extend(msg.as_bytes());
        buf.extend(b"\n");
        Ok(())
    }
}

/// Implements a newline-delimited framing algorithm that chops a stream of
/// bytes into individual frames.
impl Decoder for LineCodec {
    type Item = String;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> io::Result<Option<String>> {
        if let Some(n) = buf.iter().position(|&b| b == b'\n') {
            let line = buf.split_to(n);
            buf.split_to(1);
            match str::from_utf8(&line) {
                Ok(s) => Ok(Some(s.to_string())),
                Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
            }
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod test {
    use super::LineCodec;
    use bytes::{BufMut, BytesMut};
    use tokio_io::codec::{Encoder, Decoder};

    #[test]
    fn encoder_encodes_message() {
        let mut b = BytesMut::with_capacity(0);
        let mut codec = LineCodec::new();
        codec.encode("A packet".to_string(), &mut b).unwrap();
        assert_eq!(b"A packet\n", &b[..]);
    }

    #[test]
    fn encoder_handles_multiple_frames() {
        let mut b = BytesMut::with_capacity(0);
        let mut codec = LineCodec::new();
        codec.encode("A packet".to_string(), &mut b).unwrap();
        codec.encode("Another packet".to_string(), &mut b)
             .unwrap();
        assert_eq!(b"A packet\nAnother packet\n", &b[..]);
    }

    #[test]
    fn encoder_handles_empty_frame() {
        let mut b = BytesMut::with_capacity(0);
        let mut codec = LineCodec::new();
        codec.encode("".to_string(), &mut b).unwrap();
        codec.encode("".to_string(), &mut b).unwrap();
        assert_eq!(b"\n\n", &b[..]);
    }

    #[test]
    fn decoder_extracts_message() {
        let mut b = BytesMut::with_capacity(64);
        b.put("The boy stood on the burning deck\nWhence all but he had fled.");

        let mut codec = LineCodec::new();
        let x = codec.decode(&mut b).unwrap().unwrap();
        assert_eq!("The boy stood on the burning deck", x);
        assert_eq!(b"Whence all but he had fled.", &b[..]);
    }

    #[test]
    fn decoder_handles_zero_length_frame() {
        let mut b = BytesMut::with_capacity(16);
        b.put("\n");
        let mut codec = LineCodec::new();
        let x = codec.decode(&mut b).unwrap().unwrap();
        assert_eq!("", x);
        assert_eq!(b"", &b[..])
    }

    #[test]
    fn decoder_handles_empty_buffer() {
        let mut b = BytesMut::with_capacity(0);
        let mut codec = LineCodec::new();
        let x = codec.decode(&mut b).unwrap();
        assert!(x.is_none());
    }

    #[test]
    fn decoder_handles_incomplete_frames() {
        let mut b = BytesMut::with_capacity(64);
        b.put("The boy stood on the burning deck");

        let mut codec = LineCodec::new();
        let x = codec.decode(&mut b).unwrap();
        assert!(x.is_none());

        b.put("\nWhence all but he had fled.");
        let x = codec.decode(&mut b).unwrap().unwrap();
        assert_eq!("The boy stood on the burning deck", x);
        assert_eq!(b"Whence all but he had fled.", &b[..]);
    }

    #[test]
    fn decoder_returns_error_on_invalid_utf8() {
        let tests = vec!(
           &b"\xc3\x28"[..],         // invalid 2-octet sequence
           &b"\xa0\xa1"[..],         // invalid sequence header
           &b"\xe2\x28\xa1"[..],     // Invalid 3 Octet Sequence (in 2nd Octet)
           &b"\xe2\x82\x28"[..],     // Invalid 3 Octet Sequence (in 3rd Octet)
           &b"\xf0\x28\x8c\xbc"[..], // Invalid 4 Octet Sequence (in 2nd Octet)
           &b"\xf0\x90\x28\xbc"[..], // Invalid 4 Octet Sequence (in 3rd Octet)
           &b"\xf0\x28\x8c\x28"[..], // Invalid 4 Octet Sequence (in 4th Octet)
        );

        for case in tests.iter() {
            let mut b = BytesMut::with_capacity(64);
            b.put(case);
            b.put(b'\n');

            let mut codec = LineCodec::new();
            let x = codec.decode(&mut b);
            println!("x: {:?}", x);
            assert!(x.is_err())
        }
    }
}
