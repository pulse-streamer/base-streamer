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

    fn last_instr_end_time(&self) -> f64 {
        self.devs().values().map(|typed_dev| {
            match typed_dev {
                TypedDev::AO(dev) => dev.last_instr_end_time(),
                TypedDev::DO(dev) => dev.last_instr_end_time(),
            }
        }).fold(0.0, f64::max)
    }

    fn total_run_time(&self) -> f64 {
        self.devs().values().map(|typed_dev| match typed_dev {
            TypedDev::AO(dev) => dev.compiled_stop_time(),
            TypedDev::DO(dev) => dev.compiled_stop_time(),
        }).fold(0.0, f64::max)
    }

    fn is_edited(&self) -> bool {
        self.devs()
            .values()
            .any(
                |typed_dev| match typed_dev {
                    TypedDev::AO(dev) => dev.is_edited(),
                    TypedDev::DO(dev) => dev.is_edited(),
                }
            )
    }

    fn is_compiled(&self) -> bool {
        self.devs()
            .values()
            .any(
                |typed_dev| match typed_dev {
                    TypedDev::AO(dev) => dev.is_compiled(),
                    TypedDev::DO(dev) => dev.is_compiled(),
                }
            )
    }

    fn is_fresh_compiled(&self) -> bool {
        self.devs()
            .values()
            .all(
                |typed_dev| match typed_dev {
                    TypedDev::AO(dev) => dev.is_fresh_compiled(),
                    TypedDev::DO(dev) => dev.is_fresh_compiled(),
                }
            )
    }

    fn compile(&mut self, stop_time: Option<f64>) -> Result<f64, String> {
        let stop_time = match stop_time {
            Some(stop_time) => {
                if stop_time < self.last_instr_end_time() {
                    return Err(format!(
                        "Attempted to compile with stop_time={stop_time} [s] while the last instruction end time is {} [s]\n\
                        If you intended to provide stop_time=last_instr_end_time, use stop_time=None",
                        self.last_instr_end_time()
                    ))
                };
                stop_time
            },
            None => self.last_instr_end_time(),
        };

        for typed_dev in self.devs_mut().values_mut() {
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
        self.clear_compile_cache();
        self.devs_mut().values_mut().for_each(|typed_dev| match typed_dev {
            TypedDev::AO(dev) => dev.clear_edit_cache(),
            TypedDev::DO(dev) => dev.clear_edit_cache(),
        });
    }

    fn compiled_devs(&self) -> Vec<&TypedDev<ADev, DDev>> {
        self.devs()
            .values()
            .filter(|typed_dev| match typed_dev {
                TypedDev::AO(dev) => dev.is_compiled(),
                TypedDev::DO(dev) => dev.is_compiled(),
            })
            .collect()
    }

    fn add_reset_instr(&mut self, reset_time: Option<f64>) -> Result<(), String> {
        let last_instr_end_time = self.last_instr_end_time();
        let reset_time = match reset_time {
            Some(reset_time) => {
                if reset_time < last_instr_end_time {
                    return Err(format!(
                        "Requested to insert the all-channel reset instruction at t = {reset_time} [s] \
                        but some channels have instructions spanning until {last_instr_end_time} [s].\n\
                        If you intended to provide `reset_time=last_instr_end_time`, use `reset_time=None`"
                    ))
                }
                reset_time
            },
            None => last_instr_end_time
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