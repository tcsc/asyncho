use bytes::BytesMut;
use tokio_io::codec::{Encoder, Decoder};
use std::io;
use std::str;

pub struct LineCodec;

impl Encoder for LineCodec {
    type Item = String;
    type Error = io::Error;

    fn encode(&mut self, msg: String, buf: &mut BytesMut) -> io::Result<()> {
        buf.extend(msg.as_bytes());
        buf.extend(b"\n");
        Ok(())
    }
}

impl Decoder for LineCodec {
    type Item = String;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> io::Result<Option<String>> {
        if let Some(n) = buf.iter().position(|&b| b == b'\n') {
            let line = buf.split_to(n);
            buf.split_to(1);
            match str::from_utf8(&line) {
                Ok(s) => Ok(Some(s.to_string())),
                Err(e) => Err(io::Error::new(io::ErrorKind::Other, e))
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
    fn decoder_extracts_message() {
        let mut b = BytesMut::with_capacity(64);
        b.put("The boy stood on the burning deck\nWhence all but he had fled.");

        let mut codec = LineCodec {};
        let x = codec.decode(&mut b).unwrap().unwrap();
        assert_eq!("The boy stood on the burning deck", x);
        assert_eq!(b"Whence all but he had fled.", &b[..]);
    }

    #[test]
    fn decoder_handles_empty_buffer() {
        let mut b = BytesMut::with_capacity(0);
        let mut codec = LineCodec {};
        let x = codec.decode(&mut b).unwrap();
        assert!(x.is_none());
    }

    #[test]
    fn decoder_handles_incomplete_frames() {
        let mut b = BytesMut::with_capacity(64);
        b.put("The boy stood on the burning deck");

        let mut codec = LineCodec {};
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

            let mut codec = LineCodec {};
            let x = codec.decode(&mut b);
            println!("x: {:?}", x);
            assert!(x.is_err())
        }
    }
}