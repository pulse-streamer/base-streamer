use std::fmt::Debug;
use indexmap::IndexMap;

use crate::channel::BaseChan;
use crate::device::BaseDev;

pub enum TypedDev<ADev, DDev> {
    AO(ADev),
    DO(DDev),
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
    fn devs(&self) -> &IndexMap<String, TypedDev<ADev, DDev>>;
    fn devs_mut(&mut self) -> &mut IndexMap<String, TypedDev<ADev, DDev>>;

    fn add_ao_dev(&mut self, dev: ADev) -> Result<(), String> {
        if self.devs().contains_key(&dev.name()) {
            return Err(format!("There is already a device with name {} registered", dev.name()))
        }
        self.devs_mut().insert(dev.name(),TypedDev::AO(dev));
        Ok(())
    }

    fn add_do_dev(&mut self, dev: DDev) -> Result<(), String> {
        if self.devs().contains_key(&dev.name()) {
            return Err(format!("There is already a device with name {} registered", dev.name()))
        }
        self.devs_mut().insert(dev.name(),TypedDev::DO(dev));
        Ok(())
    }

    fn last_instr_end_time(&self) -> Option<f64> {
        self.devs()
            .values()
            .filter_map(|typed_dev| match typed_dev {
                TypedDev::AO(dev) => dev.last_instr_end_time(),
                TypedDev::DO(dev) => dev.last_instr_end_time(),
            })
            .reduce(|largest_so_far, this_end_time| f64::max(largest_so_far, this_end_time))
    }

    fn total_run_time(&self) -> f64 {
        // Sanity checks:
        /* @Backend developers: before trying to access compile cache
           you should always ensure the streamer actually got some instructions
           and that compile cache is valid (up-to-date with the current edit cache).
           Compile cache typically gets invalid due to users forgetting to re-compile after adding pulses.

           Functions `got_instructions()` and `validate_compile_cache()` are meant to be the place
           to do these checks gracefully. Other functions typically assume these checks have been done.
           They likely still double check but may just panic if the tests fail like in the example below.
        */
        if !self.got_instructions() {
            panic!("Streamer did not get any instructions")
        }
        self.validate_compile_cache().unwrap();

        self.active_devs()
            .iter()
            .map(|typed_dev| match typed_dev {
                TypedDev::AO(dev) => dev.compiled_stop_time(),
                TypedDev::DO(dev) => dev.compiled_stop_time(),
            })
            .reduce(|shortest_so_far, this_stop_time| f64::min(shortest_so_far, this_stop_time))
            .unwrap()
    }

    fn got_instructions(&self) -> bool {
        self.devs()
            .values()
            .any(|typed_dev| match typed_dev {
                TypedDev::AO(dev) => dev.got_instructions(),
                TypedDev::DO(dev) => dev.got_instructions(),
            })
    }

    fn active_devs(&self) -> Vec<&TypedDev<ADev, DDev>> {
        self.devs()
            .values()
            .filter(|typed_dev| match typed_dev {
                TypedDev::AO(dev) => dev.got_instructions(),
                TypedDev::DO(dev) => dev.got_instructions(),
            })
            .collect()
    }

    fn active_devs_mut(&mut self) -> Vec<&mut TypedDev<ADev, DDev>> {
        self.devs_mut()
            .values_mut()
            .filter(|typed_dev| match typed_dev {
                TypedDev::AO(dev) => dev.got_instructions(),
                TypedDev::DO(dev) => dev.got_instructions(),
            })
            .collect()
    }

    fn compile(&mut self, stop_time: Option<f64>) -> Result<f64, String> {
        if !self.got_instructions() {
            return Err(format!("Streamer did not get any instructions"))
        }
        let stop_time = match stop_time {
            Some(stop_time) => {
                if stop_time < self.last_instr_end_time().unwrap() {
                    return Err(format!(
                        "Attempted to compile with stop_time={stop_time} [s] while the last instruction end time is {} [s]\n\
                        If you intended to provide stop_time=last_instr_end_time, use stop_time=None",
                        self.last_instr_end_time().unwrap()
                    ))
                };
                stop_time
            },
            None => self.last_instr_end_time().unwrap(),
        };

        for typed_dev in self.active_devs_mut() {
            match typed_dev {
                TypedDev::AO(dev) => dev.compile(stop_time)?,
                TypedDev::DO(dev) => dev.compile(stop_time)?,
            };
        }

        Ok(self.total_run_time())
    }

    fn clear_compile_cache(&mut self) {
        self.devs_mut().values_mut().for_each(|typed_dev| match typed_dev {
            TypedDev::AO(dev) => dev.clear_compile_cache(),
            TypedDev::DO(dev) => dev.clear_compile_cache(),
        });
    }

    fn clear_edit_cache(&mut self) {
        self.devs_mut().values_mut().for_each(|typed_dev| match typed_dev {
            TypedDev::AO(dev) => dev.clear_edit_cache(),
            TypedDev::DO(dev) => dev.clear_edit_cache(),
        });
        self.clear_compile_cache();
    }

    fn validate_compile_cache(&self) -> Result<(), String> {
        // 2 checks:
        // - streamer got instructions in the first place;
        // - all active devices pass compile cache validation
        /* [Unlike in `BaseDev::validate_compule_cache()`, here we don't check that all devices
           were compiled to the same stop time. It would be nice, but is hard since devices run on
           different sample clocks and may have extra ticks due to closing edge clipping so they
           will naturally stop at slightly different times even when asked to compile to the same one]*/

        if !self.got_instructions() {
            return Err(format!("Streamer did not get any instructions"))
        }

        let failed_dev_msgs: Vec<String> = self
            .active_devs()
            .iter()
            .map(|typed_dev| match typed_dev {
                TypedDev::AO(dev) => dev.validate_compile_cache(),
                TypedDev::DO(dev) => dev.validate_compile_cache(),
            })
            .filter_map(|res| res.err())
            .collect();
        if !failed_dev_msgs.is_empty() {
            let mut full_err_msg = String::new();
            for msg in failed_dev_msgs {
                full_err_msg.push_str(&format!("{msg}\n"))
            };
            return Err(format!("The following devices failed compile cache validation:\n{full_err_msg}"))
        }

        Ok(())
    }

    fn add_reset_instr(&mut self, reset_time: Option<f64>) -> Result<(), String> {
        let reset_time = match reset_time {
            Some(reset_time) => {
                if self.last_instr_end_time().is_some_and(|last_instr_end| reset_time < last_instr_end){
                    return Err(format!(
                        "Requested to insert the all-channel reset instruction at t = {reset_time} [s] \
                        but some channels have instructions spanning until {} [s].\n\
                        If you intended to provide `reset_time=last_instr_end_time`, use `reset_time=None`",
                        self.last_instr_end_time().unwrap()
                    ))
                }
                reset_time
            },
            None => self.last_instr_end_time().unwrap_or(0.0),
        };
        for typed_dev in self.devs_mut().values_mut() {
            match typed_dev {
                TypedDev::AO(dev) => dev.add_reset_instr(reset_time)?,
                TypedDev::DO(dev) => dev.add_reset_instr(reset_time)?,
            }
        };
        Ok(())
    }
}