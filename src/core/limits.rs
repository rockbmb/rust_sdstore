use std::ops::{Add, AddAssign, SubAssign};

use super::filter::Filter;
use super::server::config::FiltersConfig;

pub type RunningFilters = FiltersConfig;

impl RunningFilters {
    fn change_filter(&mut self, filter: &Filter, op: impl Fn(usize) -> usize) {
        match filter {
            Filter::Nop         => self.nop = op(self.nop),
            Filter::Bcompress   => self.bcompress = op(self.bcompress),
            Filter::Bdecompress => self.bdecompress = op(self.bdecompress),
            Filter::Gcompress   => self.gcompress = op(self.gcompress),
            Filter::Gdecompress => self.gdecompress = op(self.gdecompress),
            Filter::Encrypt     => self.encrypt = op(self.encrypt),
            Filter::Decrypt     => self.decrypt = op(self.decrypt),
        }
    }

    fn increment_filter(&mut self, filter: &Filter) {
        self.change_filter(filter, |x| x + 1)
    }

    fn decrement_filter(&mut self, filter: &Filter) {
        self.change_filter(filter, |x| x - 1)
    }

    /// This method checks whether a client's requests can be executed, given the currently
    /// running transformations in the server and the limits read from the config file.
    pub fn can_run_pipeline(
        &self,
        server_cfg: &FiltersConfig,
        client_req: &Vec<Filter>
    ) -> bool { self + client_req <= *server_cfg }
}

/// The [`Add`] instance for [`RunningFilters`] takes a reference
/// because it is only used to check whether a given task can be run by the server
/// taking into account its current running count, see [`can_run_pipeline`].
///
/// There is, then, no need to move out the argument [`RunningFilters`] argument.
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

impl AddAssign<&Vec<Filter>> for RunningFilters {
    fn add_assign(&mut self, rhs: &Vec<Filter>) {
        for filter in rhs {
            self.increment_filter(filter);
        }
    }
}

impl SubAssign<&Vec<Filter>> for RunningFilters {
    fn sub_assign(&mut self, rhs: &Vec<Filter>) {
        for filter in rhs {
            self.decrement_filter(filter);
        }
    }
}