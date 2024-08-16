use std::fmt::Debug;
use indexmap::IndexMap;

use crate::channel::BaseChan;
use crate::device::BaseDev;

macro_rules! call_on_both {
    ($subj1: expr, $subj2: expr, $($method_chain:tt)*) => {{
        {$subj1.$($method_chain)*};
        {$subj2.$($method_chain)*};
    }}
}

macro_rules! eval_on_both {
    ($subj1: expr, $subj2: expr, $($method_chain:tt)*) => {{
        let res1 = {$subj1.$($method_chain)*};
        let res2 = {$subj2.$($method_chain)*};
        (res1, res2)
    }}
}

pub trait BaseStreamer<A, AChan, ADev, D, DChan, DDev>
where
    A: Clone + Debug + Send + Sync + 'static,
    AChan: BaseChan<A>,
    ADev: BaseDev<A, AChan>,
    D: Clone + Debug + Send + Sync + 'static,
    DChan: BaseChan<D>,
    DDev: BaseDev<D, DChan>
{
    fn ao_devs(&self) -> &IndexMap<String, ADev>;
    fn ao_devs_mut(&mut self) -> &mut IndexMap<String, ADev>;

    fn do_devs(&self) -> &IndexMap<String, DDev>;
    fn do_devs_mut(&mut self) -> &mut IndexMap<String, DDev>;

    fn add_ao_dev(&mut self, dev: ADev) {
        if self.ao_devs().contains_key(&dev.name()) {
            panic!("There is already an AO device with name {} registered", dev.name())
        }
        self.ao_devs_mut().insert(dev.name(), dev);
    }

    fn add_do_dev(&mut self, dev: DDev) {
        if self.do_devs().contains_key(&dev.name()) {
            panic!("There is already a DO device with name {} registered", dev.name())
        }
        self.do_devs_mut().insert(dev.name(), dev);
    }

    fn last_instr_end_time(&self) -> f64 {
        let (a_res, d_res) = eval_on_both!(
            self.ao_devs(),
            self.do_devs(),
            values().map(|dev| dev.last_instr_end_time()).fold(0.0, f64::max)
        );
        f64::max(a_res, d_res)
    }

    fn total_run_time(&self) -> f64 {
        let (a_res, d_res) = eval_on_both!(
            self.ao_devs(),
            self.do_devs(),
            values().map(|dev| dev.total_run_time()).fold(0.0, f64::max)
        );
        f64::max(a_res, d_res)
    }

    fn is_edited(&self) -> bool {
        let (a_res, d_res) = eval_on_both!(
            self.ao_devs(),
            self.do_devs(),
            values().any(|dev| dev.is_edited())
        );
        a_res || d_res
    }

    fn is_compiled(&self) -> bool {
        let (a_res, d_res) = eval_on_both!(
            self.ao_devs(),
            self.do_devs(),
            values().any(|dev| dev.is_compiled())
        );
        a_res || d_res
    }

    fn is_fresh_compiled(&self) -> bool {
        let (a_res, d_res) = eval_on_both!(
            self.ao_devs(),
            self.do_devs(),
            values().all(|dev| dev.is_fresh_compiled())
        );
        a_res && d_res
    }

    fn compile(&mut self, stop_time: Option<f64>) -> f64 {
        let stop_time = match stop_time {
            Some(stop_time) => {
                if stop_time < self.last_instr_end_time() {
                    panic!(
                        "Attempted to compile with stop_time={stop_time} [s] while the last instruction end time is {} [s]\n\
                        If you intended to provide stop_time=last_instr_end_time, use stop_time=None",
                        self.last_instr_end_time()
                    )
                };
                stop_time
            },
            None => self.last_instr_end_time(),
        };
        call_on_both!(
            self.ao_devs_mut(),
            self.do_devs_mut(),
            values_mut().for_each(|dev| {dev.compile(stop_time);})
        );
        self.total_run_time()
    }

    fn clear_compile_cache(&mut self) {
        call_on_both!(
            self.ao_devs_mut(),
            self.do_devs_mut(),
            values_mut().for_each(|dev| dev.clear_compile_cache())
        )
    }

    fn clear_edit_cache(&mut self) {
        self.clear_compile_cache();
        call_on_both!(
            self.ao_devs_mut(),
            self.do_devs_mut(),
            values_mut().for_each(|dev| dev.clear_edit_cache())
        );
    }

    fn compiled_ao_devs(&self) -> Vec<&ADev> {
        self.ao_devs()
            .values()
            .filter(|dev| dev.is_compiled())
            .collect()
    }

    fn compiled_do_devs(&self) -> Vec<&DDev> {
        self.do_devs()
            .values()
            .filter(|dev| dev.is_compiled())
            .collect()
    }

    fn add_reset_instr(&mut self, reset_time: Option<f64>) {
        let last_instr_end_time = self.last_instr_end_time();
        let reset_time = match reset_time {
            Some(reset_time) => {
                if reset_time < last_instr_end_time {
                    panic!(
                        "Requested to insert the all-channel reset instruction at t = {reset_time} [s] \
                        but some channels have instructions spanning until {last_instr_end_time} [s].\n\
                        If you intended to provide `reset_time=last_instr_end_time`, use `reset_time=None`"
                    )
                }
                reset_time
            },
            None => last_instr_end_time
        };
        call_on_both!(
            self.ao_devs_mut(),
            self.do_devs_mut(),
            values_mut().for_each(|dev| dev.add_reset_instr(reset_time))
        );
    }
}
