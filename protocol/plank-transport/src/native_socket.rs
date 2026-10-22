// SPDX-License-Identifier: AGPL-3.0-or-later

//! Explicit ownership of a shared listener's UDP descriptor. Quinn's endpoint
//! close/drain APIs do not wait for its executor tasks to release that socket.

use quinn::{AsyncUdpSocket, UdpPoller};
use std::io::{self, IoSliceMut};
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex, Weak};
use std::task::{Context, Poll, Waker};

#[derive(Debug)]
struct PollerState {
    inner: Option<Pin<Box<dyn UdpPoller>>>,
    waker: Option<Waker>,
}

#[derive(Debug)]
struct SocketPoller(Arc<Mutex<PollerState>>);

impl UdpPoller for SocketPoller {
    fn poll_writable(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let mut state = self.0.lock().unwrap();
        state.waker = Some(cx.waker().clone());
        match state.inner.as_mut() {
            Some(inner) => inner.as_mut().poll_writable(cx),
            None => Poll::Ready(Err(closed())),
        }
    }
}

#[derive(Debug)]
struct State {
    inner: Option<Arc<dyn AsyncUdpSocket>>,
    receive_waker: Option<Waker>,
    pollers: Vec<Weak<Mutex<PollerState>>>,
}

#[derive(Debug)]
pub(super) struct ListenerSocket {
    state: Mutex<State>,
    address: SocketAddr,
    max_transmit_segments: usize,
    max_receive_segments: usize,
    may_fragment: bool,
}

fn closed() -> io::Error {
    io::Error::new(
        io::ErrorKind::NotConnected,
        "PLANK native listener socket closed",
    )
}

impl ListenerSocket {
    pub(super) fn new(inner: Arc<dyn AsyncUdpSocket>) -> io::Result<Arc<Self>> {
        Ok(Arc::new(Self {
            address: inner.local_addr()?,
            max_transmit_segments: inner.max_transmit_segments(),
            max_receive_segments: inner.max_receive_segments(),
            may_fragment: inner.may_fragment(),
            state: Mutex::new(State {
                inner: Some(inner),
                receive_waker: None,
                pollers: Vec::new(),
            }),
        }))
    }

    pub(super) fn close(&self) {
        let (inner, wakers) = {
            let mut state = self.state.lock().unwrap();
            let Some(inner) = state.inner.take() else {
                return;
            };
            let mut wakers: Vec<_> = state.receive_waker.take().into_iter().collect();
            for weak in state.pollers.drain(..) {
                if let Some(poller) = weak.upgrade() {
                    let mut poller = poller.lock().unwrap();
                    // Runtime pollers can retain their own socket Arc, even
                    // when no connection is currently waiting for writability.
                    poller.inner.take();
                    wakers.extend(poller.waker.take());
                }
            }
            (inner, wakers)
        };
        // Every underlying socket owner is now gone. In-flight nonblocking
        // send/receive/poll calls finished under the same locks before close.
        // Release the actual descriptor before waking Quinn or returning.
        drop(inner);
        for waker in wakers {
            waker.wake();
        }
    }
}

impl AsyncUdpSocket for ListenerSocket {
    fn create_io_poller(self: Arc<Self>) -> Pin<Box<dyn UdpPoller>> {
        let mut state = self.state.lock().unwrap();
        let inner = state
            .inner
            .as_ref()
            .map(|inner| inner.clone().create_io_poller());
        let poller = Arc::new(Mutex::new(PollerState { inner, waker: None }));
        state.pollers.retain(|weak| weak.strong_count() != 0);
        state.pollers.push(Arc::downgrade(&poller));
        Box::pin(SocketPoller(poller))
    }

    fn try_send(&self, transmit: &quinn::udp::Transmit<'_>) -> io::Result<()> {
        let state = self.state.lock().unwrap();
        state.inner.as_ref().ok_or_else(closed)?.try_send(transmit)
    }

    fn poll_recv(
        &self,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
        meta: &mut [quinn::udp::RecvMeta],
    ) -> Poll<io::Result<usize>> {
        let mut state = self.state.lock().unwrap();
        state.receive_waker = Some(cx.waker().clone());
        match &state.inner {
            Some(inner) => inner.poll_recv(cx, bufs, meta),
            None => Poll::Ready(Err(closed())),
        }
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.address)
    }
    fn max_transmit_segments(&self) -> usize {
        self.max_transmit_segments
    }
    fn max_receive_segments(&self) -> usize {
        self.max_receive_segments
    }
    fn may_fragment(&self) -> bool {
        self.may_fragment
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quinn::Runtime;
    use std::net::UdpSocket;

    #[tokio::test]
    async fn explicit_close_releases_port_while_quinn_socket_and_pollers_remain_alive() {
        let raw = UdpSocket::bind("127.0.0.1:0").unwrap();
        let address = raw.local_addr().unwrap();
        let socket =
            ListenerSocket::new(quinn::TokioRuntime.wrap_udp_socket(raw).unwrap()).unwrap();
        let mut poller = socket.clone().create_io_poller();
        let mut context = Context::from_waker(Waker::noop());
        let mut buffer = [0; 1500];
        let mut buffers = [IoSliceMut::new(&mut buffer)];
        let mut meta = [quinn::udp::RecvMeta::default()];
        assert!(matches!(
            socket.poll_recv(&mut context, &mut buffers, &mut meta),
            Poll::Pending
        ));
        assert!(UdpSocket::bind(address).is_err());
        socket.close();
        let _replacement =
            UdpSocket::bind(address).expect("retained Quinn handles kept the descriptor open");
        assert!(
            matches!(poller.as_mut().poll_writable(&mut context), Poll::Ready(Err(error)) if error.kind() == io::ErrorKind::NotConnected)
        );
        assert!(
            matches!(socket.poll_recv(&mut context, &mut buffers, &mut meta), Poll::Ready(Err(error)) if error.kind() == io::ErrorKind::NotConnected)
        );
        assert_eq!(
            socket
                .try_send(&quinn::udp::Transmit {
                    destination: address,
                    ecn: None,
                    contents: b"closed",
                    segment_size: None,
                    src_ip: None
                })
                .unwrap_err()
                .kind(),
            io::ErrorKind::NotConnected
        );
        let mut late_poller = socket.clone().create_io_poller();
        assert!(
            matches!(late_poller.as_mut().poll_writable(&mut context), Poll::Ready(Err(error)) if error.kind() == io::ErrorKind::NotConnected)
        );
        socket.close();
    }
}
