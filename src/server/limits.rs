use std::ops::{Add, Sub};

use crate::filter::Filter;
use super::config::FiltersConfig;

pub type RunningFilters = FiltersConfig;

impl RunningFilters {
    fn change_filter(&mut self, filter: &Filter, op: impl Fn(usize) -> usize) {
        match filter {
            Filter::Nop          => self.nop = op(self.nop),
            Filter::Bcompress    => self.bcompress = op(self.bcompress),
            Filter::Bdecompress  => self.bdecompress = op(self.bdecompress),
            Filter::Gcompress    => self.gcompress = op(self.gcompress),
            Filter::Gdecompress  => self.gdecompress = op(self.gdecompress),
            Filter::Encrypt      => self.encrypt = op(self.encrypt),
            Filter::Decrypt      => self.decrypt = op(self.decrypt),
        }
    }

    fn increment_filter(&mut self, filter: &Filter) {
        self.change_filter(filter, |x| x + 1)
    }

    fn decrement_filter(&mut self, filter: &Filter) {
        self.change_filter(filter, |x| x - 1)
    }

    /// This method checks, given the currently running transformations in the server
    /// and the limits read from the config file, whether a client's requests can be executed
    /// or not.
    pub fn can_run_pipeline(
        &self,
        server_cfg: &FiltersConfig,
        client_req: &Vec<Filter>
    ) -> bool { self + client_req <= *server_cfg }
}

impl Add<&Vec<Filter>> for &RunningFilters {
    type Output = RunningFilters;

    fn add(self, rhs: &Vec<Filter>) -> Self::Output {
        let mut res = self.clone();
        for filter in rhs {
            res.increment_filter(filter);
        }
        res
    }
}

impl Sub<&Vec<Filter>> for &RunningFilters {
    type Output = RunningFilters;

    fn sub(self, rhs: &Vec<Filter>) -> Self::Output {
        let mut res = self.clone();
        for filter in rhs {
            res.decrement_filter(filter)
        }
        res
    }
}