use std::pin::Pin;
use std::task::{Context, Poll};

use aws_lc_rs::aead::{AES_128_GCM, Aad, BoundKey, OpeningKey, SealingKey, UnboundKey};
use bytes::{Buf, BytesMut};
use digest::XofReader;
use futures::ready;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use super::nonce::{SingleUseNonce, VmessNonceSequence};
use super::typed::VmessReader;

const HEADER_TAG_LEN: usize = 16;
const DATA_TAG_LEN: usize = 16;
const MAX_PLAINTEXT_SIZE: usize = 8192;

struct LengthMask {
    reader: VmessReader,
    mask: [u8; 2],
}

impl LengthMask {
    fn new(reader: VmessReader) -> Self {
        Self {
            reader,
            mask: [0u8; 2],
        }
    }

    fn next_u16(&mut self) -> u16 {
        self.reader.read(&mut self.mask);
        ((self.mask[0] as u16) << 8) | (self.mask[1] as u16)
    }
}

pub struct ReadHeaderInfo {
    pub response_header_key: [u8; 16],
    pub response_header_iv: [u8; 16],
    pub response_authentication_v: u8,
}

enum ReadHeaderState {
    Length,
    Content(usize),
    Done,
}

pub struct VmessStream<T> {
    inner: T,
    read_header_state: ReadHeaderState,
    read_header_info: Option<ReadHeaderInfo>,
    opening_key: Option<OpeningKey<VmessNonceSequence>>,
    sealing_key: Option<SealingKey<VmessNonceSequence>>,
    read_length_mask: Option<LengthMask>,
    write_length_mask: Option<LengthMask>,
    incoming: BytesMut,
    read_frame: BytesMut,
    pending_write: BytesMut,
    write_shutdown_frame: bool,
    eof: bool,
}

impl<T> VmessStream<T> {
    pub fn new(
        inner: T,
        encryption_keys: Option<(
            OpeningKey<VmessNonceSequence>,
            SealingKey<VmessNonceSequence>,
        )>,
        read_length_shake_reader: Option<VmessReader>,
        write_length_shake_reader: Option<VmessReader>,
        read_header_info: Option<ReadHeaderInfo>,
    ) -> Self {
        let (opening_key, sealing_key) = match encryption_keys {
            Some((opening, sealing)) => (Some(opening), Some(sealing)),
            None => (None, None),
        };

        let read_header_state = if read_header_info.is_some() {
            ReadHeaderState::Length
        } else {
            ReadHeaderState::Done
        };

        Self {
            inner,
            read_header_state,
            read_header_info,
            opening_key,
            sealing_key,
            read_length_mask: read_length_shake_reader.map(LengthMask::new),
            write_length_mask: write_length_shake_reader.map(LengthMask::new),
            incoming: BytesMut::with_capacity(16 * 1024),
            read_frame: BytesMut::new(),
            pending_write: BytesMut::new(),
            write_shutdown_frame: true,
            eof: false,
        }
    }

    fn process_response_header(&mut self) -> std::io::Result<bool> {
        loop {
            match self.read_header_state {
                ReadHeaderState::Length => {
                    if self.incoming.len() < 2 + HEADER_TAG_LEN {
                        return Ok(false);
                    }

                    let mut encrypted = self.incoming.split_to(2 + HEADER_TAG_LEN).to_vec();
                    let info = self.read_header_info.as_ref().expect("header info must exist");

                    let key =
                        super::sha2::kdf(&info.response_header_key, &[b"AEAD Resp Header Len Key"]);
                    let nonce =
                        super::sha2::kdf(&info.response_header_iv, &[b"AEAD Resp Header Len IV"]);
                    let unbound = UnboundKey::new(&AES_128_GCM, &key[0..16]).map_err(to_io_error)?;
                    let mut opening =
                        OpeningKey::new(unbound, SingleUseNonce::new(&nonce[0..12]));
                    opening
                        .open_in_place(Aad::empty(), &mut encrypted)
                        .map_err(to_io_error)?;

                    let len = u16::from_be_bytes([encrypted[0], encrypted[1]]) as usize;
                    self.read_header_state = ReadHeaderState::Content(len);
                }
                ReadHeaderState::Content(content_len) => {
                    if self.incoming.len() < content_len + HEADER_TAG_LEN {
                        return Ok(false);
                    }

                    let mut encrypted =
                        self.incoming.split_to(content_len + HEADER_TAG_LEN).to_vec();
                    let info = self.read_header_info.take().expect("header info must exist");

                    let key =
                        super::sha2::kdf(&info.response_header_key, &[b"AEAD Resp Header Key"]);
                    let nonce =
                        super::sha2::kdf(&info.response_header_iv, &[b"AEAD Resp Header IV"]);
                    let unbound = UnboundKey::new(&AES_128_GCM, &key[0..16]).map_err(to_io_error)?;
                    let mut opening =
                        OpeningKey::new(unbound, SingleUseNonce::new(&nonce[0..12]));
                    let decrypted = opening
                        .open_in_place(Aad::empty(), &mut encrypted)
                        .map_err(to_io_error)?;

                    if decrypted.first().copied() != Some(info.response_authentication_v) {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "invalid VMess response auth byte",
                        ));
                    }

                    self.read_header_state = ReadHeaderState::Done;
                    return Ok(true);
                }
                ReadHeaderState::Done => return Ok(true),
            }
        }
    }

    fn process_read_frame(&mut self) -> std::io::Result<bool> {
        if !self.read_frame.is_empty() || self.eof {
            return Ok(true);
        }

        if !matches!(self.read_header_state, ReadHeaderState::Done) {
            return Ok(false);
        }

        if self.incoming.len() < 2 {
            return Ok(false);
        }

        let raw = u16::from_be_bytes([self.incoming[0], self.incoming[1]]);
        let length = match &mut self.read_length_mask {
            Some(mask) => raw ^ mask.next_u16(),
            None => raw,
        } as usize;

        if self.incoming.len() < 2 + length {
            return Ok(false);
        }

        self.incoming.advance(2);
        if length == DATA_TAG_LEN {
            self.incoming.advance(length);
            self.eof = true;
            return Ok(true);
        }

        let mut encrypted = self.incoming.split_to(length).to_vec();
        let decrypted = if let Some(opening) = &mut self.opening_key {
            opening
                .open_in_place(Aad::empty(), &mut encrypted)
                .map_err(to_io_error)?
        } else {
            &encrypted[..]
        };

        self.read_frame.extend_from_slice(decrypted);
        Ok(true)
    }

    fn build_write_frame(&mut self, payload: &[u8]) -> std::io::Result<()> {
        let mut frame = BytesMut::with_capacity(2 + payload.len() + DATA_TAG_LEN);
        let mut body = payload.to_vec();

        let body_len = if let Some(sealing) = &mut self.sealing_key {
            let tag = sealing
                .seal_in_place_separate_tag(Aad::empty(), &mut body)
                .map_err(to_io_error)?;
            body.extend_from_slice(tag.as_ref());
            body.len()
        } else {
            body.len()
        };

        let mut length = body_len as u16;
        if let Some(mask) = &mut self.write_length_mask {
            length ^= mask.next_u16();
        }

        frame.extend_from_slice(&length.to_be_bytes());
        frame.extend_from_slice(&body);
        self.pending_write.extend_from_slice(&frame);
        Ok(())
    }

    fn try_write_pending(&mut self, cx: &mut Context<'_>) -> Poll<std::io::Result<()>>
    where
        T: AsyncWrite + Unpin,
    {
        while !self.pending_write.is_empty() {
            let written = ready!(Pin::new(&mut self.inner).poll_write(cx, &self.pending_write))?;
            if written == 0 {
                return Poll::Ready(Err(std::io::Error::new(
                    std::io::ErrorKind::WriteZero,
                    "failed to write VMess frame",
                )));
            }
            self.pending_write.advance(written);
        }

        Poll::Ready(Ok(()))
    }
}

impl<T> AsyncRead for VmessStream<T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if !self.read_frame.is_empty() {
            let amount = self.read_frame.len().min(buf.remaining());
            let chunk = self.read_frame.split_to(amount);
            buf.put_slice(&chunk);
            return Poll::Ready(Ok(()));
        }

        if self.eof {
            return Poll::Ready(Ok(()));
        }

        loop {
            if !self.process_response_header()? || !self.process_read_frame()? {
                let mut temp = [0u8; 4096];
                let mut inner_buf = ReadBuf::new(&mut temp);
                ready!(Pin::new(&mut self.inner).poll_read(cx, &mut inner_buf))?;
                let filled = inner_buf.filled();
                if filled.is_empty() {
                    self.eof = true;
                    return Poll::Ready(Ok(()));
                }
                self.incoming.extend_from_slice(filled);
                continue;
            }

            if !self.read_frame.is_empty() {
                let amount = self.read_frame.len().min(buf.remaining());
                let chunk = self.read_frame.split_to(amount);
                buf.put_slice(&chunk);
            }
            return Poll::Ready(Ok(()));
        }
    }
}

impl<T> AsyncWrite for VmessStream<T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        ready!(self.try_write_pending(cx))?;

        let to_buffer = buf.len().min(MAX_PLAINTEXT_SIZE);
        if to_buffer == 0 {
            return Poll::Ready(Ok(0));
        }

        self.build_write_frame(&buf[..to_buffer])?;
        ready!(self.try_write_pending(cx))?;
        Poll::Ready(Ok(to_buffer))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        ready!(self.try_write_pending(cx))?;
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        if self.write_shutdown_frame {
            if self.pending_write.is_empty() {
                self.build_write_frame(&[])?;
            }
            ready!(self.try_write_pending(cx))?;
            self.write_shutdown_frame = false;
        }

        ready!(Pin::new(&mut self.inner).poll_flush(cx))?;
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

fn to_io_error(err: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::other(err.to_string())
}
