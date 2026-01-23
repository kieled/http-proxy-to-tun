use std::collections::VecDeque;

use smoltcp::phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken};
use smoltcp::time::Instant as SmolInstant;

pub(crate) struct QueueDevice {
    rx: VecDeque<Vec<u8>>,
    tx: VecDeque<Vec<u8>>,
    mtu: usize,
}

impl QueueDevice {
    pub(crate) fn new(mtu: usize) -> Self {
        Self {
            rx: VecDeque::new(),
            tx: VecDeque::new(),
            mtu,
        }
    }

    pub(crate) fn push_rx(&mut self, pkt: Vec<u8>) {
        self.rx.push_back(pkt);
    }

    pub(crate) fn pop_tx(&mut self) -> Option<Vec<u8>> {
        self.tx.pop_front()
    }
}

impl Device for QueueDevice {
    type RxToken<'a> = QueueRxToken where Self: 'a;
    type TxToken<'a> = QueueTxToken<'a> where Self: 'a;

    fn receive(
        &mut self,
        _timestamp: SmolInstant,
    ) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let pkt = self.rx.pop_front()?;
        Some((QueueRxToken { pkt }, QueueTxToken { tx: &mut self.tx }))
    }

    fn transmit(&mut self, _timestamp: SmolInstant) -> Option<Self::TxToken<'_>> {
        Some(QueueTxToken { tx: &mut self.tx })
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = Medium::Ip;
        caps.max_transmission_unit = self.mtu;
        caps
    }
}

pub(crate) struct QueueRxToken {
    pkt: Vec<u8>,
}

impl RxToken for QueueRxToken {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        f(&self.pkt)
    }
}

pub(crate) struct QueueTxToken<'a> {
    tx: &'a mut VecDeque<Vec<u8>>,
}

impl<'a> TxToken for QueueTxToken<'a> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut buf = vec![0u8; len];
        let result = f(&mut buf);
        self.tx.push_back(buf);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_device_rx_tx_flow() {
        let mut dev = QueueDevice::new(1500);
        dev.push_rx(vec![1, 2, 3]);

        let (rx, _tx) = dev.receive(SmolInstant::from_millis(0)).unwrap();
        let mut out = Vec::new();
        rx.consume(|buf| out.extend_from_slice(buf));
        assert_eq!(out, vec![1, 2, 3]);

        let tx = dev.transmit(SmolInstant::from_millis(0)).unwrap();
        tx.consume(4, |buf| {
            buf.copy_from_slice(&[4, 5, 6, 7]);
        });
        assert_eq!(dev.pop_tx(), Some(vec![4, 5, 6, 7]));
    }
}
