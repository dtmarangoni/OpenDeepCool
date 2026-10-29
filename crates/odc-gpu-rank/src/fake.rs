//! A [`KindSource`] for tests, this crate's and `odc-e2e`'s: it answers from a table and records
//! each question, so a test can show which GPUs were asked and which never were.

use odc_sensors::PciAddr;

use crate::{Answer, KindSource, Signal};

pub struct FakeKinds {
    answers: Vec<(PciAddr, Result<Answer, String>)>,
    asked: Vec<(Signal, PciAddr)>,
}

impl FakeKinds {
    /// Each GPU's answer, by PCI address: its kind, or the error its query fails with. A GPU
    /// that isn't listed fails too.
    ///
    /// # Panics
    ///
    /// When an address isn't one.
    pub fn new(answers: &[(&str, Result<Answer, &str>)]) -> Self {
        FakeKinds {
            answers: answers
                .iter()
                .map(|(pci, answer)| {
                    let pci = pci.parse().expect("a PCI address");
                    (pci, answer.map_err(str::to_owned))
                })
                .collect(),
            asked: Vec::new(),
        }
    }

    /// Every question so far, in order.
    pub fn asked(&self) -> &[(Signal, PciAddr)] {
        &self.asked
    }
}

impl KindSource for FakeKinds {
    fn ask(&mut self, signal: Signal, pci: PciAddr) -> Result<Answer, String> {
        self.asked.push((signal, pci));
        self.answers
            .iter()
            .find(|(known, _)| *known == pci)
            .map_or_else(
                || Err(format!("the fake has no answer for {pci}")),
                |(_, answer)| answer.clone(),
            )
    }
}
