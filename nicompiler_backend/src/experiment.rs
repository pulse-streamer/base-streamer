//! Implements the main trait [`BaseExperiment`] for the [`Experiment`] struct, which constitute the highest 
//! level of abstraction for interacting with NI tasks. The [`Experiment`] task, together 
//! with its implementation, constitute the main API through which python 
//! processes invoke the rust backend
//! 


use ndarray::Array2;
use numpy;
use pyo3::prelude::*;
use std::collections::HashMap;

use crate::channel::*;
use crate::device::*;
use crate::instruction::*;
use crate::utils::*;

pub trait BaseExperiment {
    // FIELD methods
    fn devices(&self) -> &HashMap<String, Device>;
    fn devices_(&mut self) -> &mut HashMap<String, Device>;

    // EXPERIMENT METHODS START
    fn assert_has_device(&self, dev_name: &str) {
        assert!(
            self.devices().contains_key(dev_name),
            "Physical device {} not found. Registered devices are {:?}",
            dev_name,
            self.devices().keys().collect::<Vec<_>>()
        );
    }

    fn assert_device_has_channel(&self, dev_name: &str, chan_name: &str) {
        self.assert_has_device(dev_name);
        let device = self.devices().get(dev_name).unwrap();
        assert!(
            device.channels().contains_key(chan_name),
            "Channel name {} not found in device {}. Registered channels are: {:?}",
            chan_name,
            dev_name,
            device.channels().keys().collect::<Vec<_>>()
        );
    }

    fn add_device_base(&mut self, dev: Device) {
        // Duplicate check
        let dev_name = dev.physical_name();
        // let is_primary = dev.is_primary();
        assert!(
            !self.devices().contains_key(dev_name),
            "Device {} already registered. Registered devices are {:?}",
            dev_name,
            self.devices().keys().collect::<Vec<_>>()
        );
        // Synchronization check
        // assert!(
        //     !(is_primary && self.devices().values().any(|d| d.is_primary())),
        //     "Cannot register another primary device {}",
        //     dev_name
        // );
        self.devices_().insert(dev_name.to_string(), dev);
    }

    fn add_ao_device(
        &mut self,
        physical_name: &str,
        samp_rate: f64,
        samp_clk_src: Option<&str>,
        trig_line: Option<&str>,
        is_primary: Option<bool>,
        ref_clk_line: Option<&str>,
        import_ref_clk: Option<bool>,
        ref_clk_rate: Option<f64>,
    ) {
        self.add_device_base(Device::new(
            physical_name,
            TaskType::AO,
            samp_rate,
            samp_clk_src,
            trig_line,
            is_primary,
            ref_clk_line,
            import_ref_clk,
            ref_clk_rate,
        ));
    }

    fn add_do_device(
        &mut self,
        physical_name: &str,
        samp_rate: f64,
        samp_clk_src: Option<&str>,
        trig_line: Option<&str>,
        is_primary: Option<bool>,
        ref_clk_line: Option<&str>,
        import_ref_clk: Option<bool>,
        ref_clk_rate: Option<f64>,
    ) {
        self.add_device_base(Device::new(
            physical_name,
            TaskType::DO,
            samp_rate,
            samp_clk_src,
            trig_line,
            is_primary,
            ref_clk_line,
            import_ref_clk,
            ref_clk_rate,
        ));
    }

    fn edit_stop_time(&self) -> f64 {
        self.devices()
            .values()
            .map(|dev| dev.edit_stop_time())
            .fold(0.0, f64::max)
    }

    fn compiled_stop_time(&self) -> f64 {
        self.devices()
            .values()
            .map(|dev| dev.compiled_stop_time())
            .fold(0.0, f64::max)
    }

    fn compile(&mut self) -> f64 {
        // Called without arguments, compiles based on stop_time of instructions
        let stop_time = self.edit_stop_time();
        self.compile_with_stoptime(stop_time);
        assert!(stop_time == self.compiled_stop_time());
        stop_time
    }

    fn compile_with_stoptime(&mut self, stop_time: f64) {
        // assert!(
        //     self.devices().values().any(|dev| dev.is_primary()),
        //     "Cannot compile an experiment with no primary device"
        // );
        self.devices_()
            .values_mut()
            .for_each(|dev| dev.compile(((stop_time) * dev.samp_rate()) as usize));
    }

    fn compiled_devices(&self) -> Vec<&Device> {
        self.devices()
            .values()
            .filter_map(|dev| if dev.is_compiled() { Some(&*dev) } else { None })
            .collect()
    }

    fn is_edited(&self) -> bool {
        self.devices().values().any(|dev| dev.is_edited())
    }

    fn is_compiled(&self) -> bool {
        self.devices().values().any(|dev| dev.is_compiled())
    }

    fn is_fresh_compiled(&self) -> bool {
        self.devices().values().all(|dev| dev.is_fresh_compiled())
    }

    fn clear_edit_cache(&mut self) {
        self.devices_()
            .values_mut()
            .for_each(|dev| dev.clear_edit_cache());
    }

    fn clear_compile_cache(&mut self) {
        self.devices_()
            .values_mut()
            .for_each(|dev| dev.clear_compile_cache());
    }

    // TEMPLATE METHODS (for forwarding device and channel methods)
    fn typed_device_op<F, R>(&mut self, dev_name: &str, task_type: TaskType, mut f: F) -> R
    where
        F: FnMut(&mut Device) -> R,
    {
        // This helper function performs checks and asserts the required device type
        // then executes closure `f` on the specified device
        self.assert_has_device(dev_name);
        let dev = self.devices_().get_mut(dev_name).unwrap();
        assert!(
            dev.task_type() == task_type,
            "Device {} is incompatible with instruction",
            dev_name
        );
        f(dev)
    }

    fn device_op<F, R>(&mut self, dev_name: &str, mut f: F) -> R
    where
        F: FnMut(&mut Device) -> R,
    {
        // This helper function performs checks (existence of device) then performs closure)
        // Type-agnostic variant of typed_device_op
        self.assert_has_device(dev_name);
        let dev = self.devices_().get_mut(dev_name).unwrap();
        f(dev)
    }

    fn typed_channel_op<F, R>(
        &mut self,
        dev_name: &str,
        chan_name: &str,
        task_type: TaskType,
        mut f: F,
    ) -> R
    where
        F: FnMut(&mut Channel) -> R,
    {
        // This helper function performs checks and asserts the required device type
        // then executes closure `f` on the specified channel
        self.assert_device_has_channel(dev_name, chan_name);
        let dev = self.devices_().get_mut(dev_name).unwrap();
        assert!(
            dev.task_type() == task_type,
            "Channel {}/{} is incompatible with instruction",
            dev_name,
            chan_name
        );
        let chan = dev.channels_().get_mut(chan_name).unwrap();
        f(chan)
    }

    fn channel_op<F, R>(&mut self, dev_name: &str, chan_name: &str, mut f: F) -> R
    where
        F: FnMut(&mut Channel) -> R,
    {
        // Type-agnostic variant of typed_channel_op
        self.assert_device_has_channel(dev_name, chan_name);
        let chan = self
            .devices_()
            .get_mut(dev_name)
            .unwrap()
            .channels_()
            .get_mut(chan_name)
            .unwrap();
        f(chan)
    }

    // FORWARDED DEVICE METHODS
    fn device_calc_signal_nsamps(
        &mut self,
        dev_name: &str,
        start_pos: usize,
        end_pos: usize,
        nsamps: usize,
        require_streamable: bool,
        require_editable: bool,
    ) -> Array2<f64> {
        self.device_op(dev_name, |dev| {
            (*dev).calc_signal_nsamps(
                start_pos,
                end_pos,
                nsamps,
                require_streamable,
                require_editable,
            )
        })
    }

    fn device_edit_stop_time(&mut self, dev_name: &str) -> f64 {
        self.device_op(dev_name, |dev| (*dev).edit_stop_time())
    }

    fn device_compiled_stop_time(&mut self, dev_name: &str) -> f64 {
        self.device_op(dev_name, |dev| (*dev).compiled_stop_time())
    }

    fn device_clear_compile_cache(&mut self, dev_name: &str) {
        self.device_op(dev_name, |dev| (*dev).clear_compile_cache())
    }

    fn add_ao_channel(&mut self, dev_name: &str, channel_id: usize) {
        self.typed_device_op(dev_name, TaskType::AO, |dev| {
            (*dev).add_channel(&format!("ao{}", channel_id))
        });
    }

    fn add_do_channel(&mut self, dev_name: &str, port_id: usize, line_id: usize) {
        self.typed_device_op(dev_name, TaskType::DO, |dev| {
            (*dev).add_channel(&format!("port{}/line{}", port_id, line_id))
        });
    }

    // Channel methods
    fn constant(
        &mut self,
        dev_name: &str,
        chan_name: &str,
        t: f64,
        duration: f64,
        value: f64,
        keep_val: bool,
    ) {
        self.typed_channel_op(dev_name, chan_name, TaskType::AO, |chan| {
            (*chan).constant(value, t, duration, keep_val);
        });
    }

    fn sine(
        &mut self,
        dev_name: &str,
        chan_name: &str,
        t: f64,
        duration: f64,
        keep_val: bool,
        freq: f64,
        amplitude: Option<f64>,
        phase: Option<f64>,
        dc_offset: Option<f64>,
    ) {
        self.typed_channel_op(dev_name, chan_name, TaskType::AO, |chan| {
            let instr = Instruction::new_sine(freq, amplitude, phase, dc_offset);
            (*chan).add_instr(instr, t, duration, keep_val)
        });
    }

    fn high(&mut self, dev_name: &str, chan_name: &str, t: f64, duration: f64) {
        self.typed_channel_op(dev_name, chan_name, TaskType::DO, |chan| {
            (*chan).constant(1., t, duration, false);
        });
    }

    fn low(&mut self, dev_name: &str, chan_name: &str, t: f64, duration: f64) {
        self.typed_channel_op(dev_name, chan_name, TaskType::DO, |chan| {
            (*chan).constant(0., t, duration, false);
        });
    }

    fn go_high(&mut self, dev_name: &str, chan_name: &str, t: f64) {
        self.typed_channel_op(dev_name, chan_name, TaskType::DO, |chan| {
            (*chan).constant(1., t, 1. / (*chan).samp_rate(), true);
        });
    }

    fn go_low(&mut self, dev_name: &str, chan_name: &str, t: f64) {
        self.typed_channel_op(dev_name, chan_name, TaskType::DO, |chan| {
            (*chan).constant(0., t, 1. / (*chan).samp_rate(), true);
        });
    }
}

#[pyclass]
pub struct Experiment {
    devices: HashMap<String, Device>,
}

/// A macro to generate boilerplate implementations for structs representing experiments.
///
/// This macro assists in the conversion between Rust's trait system and Python's class system.
/// Given that PyO3 doesn't support exposing trait methods directly to Python, this macro wraps
/// each [`BaseExperiment`] trait method with a direct implementation, facilitating its export to Python.
///
/// The majority of methods are exported with their arguments and types preserved. 
/// Any deviations from this convention should be explicitly noted and elaborated upon.
///
/// Usage:
/// ```rust
/// use nicompiler_backend::device::*;
/// use nicompiler_backend::channel::*;
/// use nicompiler_backend::*;
/// use pyo3::prelude::*;
/// use std::collections::HashMap;
/// 
/// #[pyclass]
/// struct CustomExperiment {
///     devices: HashMap<String, Device>,
///     some_property: f64,
/// }
/// impl_exp_boilerplate!(CustomExperiment);
/// 
/// // Implement additional methods which can be exposed to python
/// #[pymethods]
/// impl CustomExperiment {
///     #[new]
///     pub fn new(some_property: f64) -> Self {
///         Self {
///             devices: HashMap::new(),
///             some_property
///         }
///     }
/// }
/// ```
///
/// This will generate the required implementations and additional Python bindings for `CustomExperiment`.

#[macro_export]
macro_rules! impl_exp_boilerplate {
    ($exp_type: ty) => {
        impl BaseExperiment for $exp_type {
            fn devices(&self) -> &HashMap<String, Device> {
                &self.devices
            }
            fn devices_(&mut self) -> &mut HashMap<String, Device> {
                &mut self.devices
            }
        }

        #[pymethods]
        impl $exp_type {
            fn add_ao_device(
                &mut self,
                physical_name: &str,
                samp_rate: f64,
                samp_clk_src: Option<&str>,
                trig_line: Option<&str>,
                is_primary: Option<bool>,
                ref_clk_line: Option<&str>,
                import_ref_clk: Option<bool>,
                ref_clk_rate: Option<f64>,
            ) {
                self.add_device_base(Device::new(
                    physical_name,
                    TaskType::AO,
                    samp_rate,
                    samp_clk_src,
                    trig_line,
                    is_primary,
                    ref_clk_line,
                    import_ref_clk,
                    ref_clk_rate,
                ));
            }

            fn add_do_device(
                &mut self,
                physical_name: &str,
                samp_rate: f64,
                samp_clk_src: Option<&str>,
                trig_line: Option<&str>,
                is_primary: Option<bool>,
                ref_clk_line: Option<&str>,
                import_ref_clk: Option<bool>,
                ref_clk_rate: Option<f64>,
            ) {
                self.add_device_base(Device::new(
                    physical_name,
                    TaskType::DO,
                    samp_rate,
                    samp_clk_src,
                    trig_line,
                    is_primary,
                    ref_clk_line,
                    import_ref_clk,
                    ref_clk_rate,
                ));
            }

            pub fn edit_stop_time(&self) -> f64 {
                BaseExperiment::edit_stop_time(self)
            }

            pub fn compiled_stop_time(&self) -> f64 {
                BaseExperiment::compiled_stop_time(self)
            }

            pub fn compile(&mut self) -> f64 {
                BaseExperiment::compile(self)
            }

            pub fn compile_with_stoptime(&mut self, stop_time: f64) {
                BaseExperiment::compile_with_stoptime(self, stop_time);
            }

            pub fn is_edited(&self) -> bool {
                BaseExperiment::is_edited(self)
            }

            pub fn is_compiled(&self) -> bool {
                BaseExperiment::is_compiled(self)
            }

            pub fn is_fresh_compiled(&self) -> bool {
                BaseExperiment::is_fresh_compiled(self)
            }

            pub fn clear_edit_cache(&mut self) {
                BaseExperiment::clear_edit_cache(self);
            }

            pub fn clear_compile_cache(&mut self) {
                BaseExperiment::clear_compile_cache(self);
            }

            // DEVICE METHODS
            pub fn add_ao_channel(&mut self, dev_name: &str, channel_id: usize) {
                BaseExperiment::add_ao_channel(self, dev_name, channel_id);
            }

            pub fn add_do_channel(&mut self, dev_name: &str, port_id: usize, line_id: usize) {
                BaseExperiment::add_do_channel(self, dev_name, port_id, line_id);
            }

            pub fn device_compiled_channel_names(&mut self, dev_name: &str) -> Vec<String> {
                self.device_op(dev_name, |dev| {
                    (*dev)
                        .compiled_channels(false, true)
                        .iter()
                        .map(|chan| chan.physical_name().to_string())
                        .collect()
                })
            }

            pub fn calc_signal(
                &mut self,
                dev_name: &str,
                t_start: f64,
                t_end: f64,
                nsamps: usize,
                py: Python,
            ) -> PyResult<PyObject> {
                self.assert_has_device(dev_name);
                let samp_rate = self.devices().get(dev_name).unwrap().samp_rate();
                // To python, only expose editable channels
                let arr = BaseExperiment::device_calc_signal_nsamps(
                    self,
                    dev_name,
                    (t_start * samp_rate) as usize,
                    (t_end * samp_rate) as usize,
                    nsamps,
                    false,
                    true,
                );
                Ok(numpy::PyArray::from_array(py, &arr).to_object(py))
            }

            pub fn device_edit_stop_time(&mut self, dev_name: &str) -> f64 {
                BaseExperiment::device_edit_stop_time(self, dev_name)
            }

            pub fn device_compiled_stop_time(&mut self, dev_name: &str) -> f64 {
                BaseExperiment::device_compiled_stop_time(self, dev_name)
            }

            pub fn device_clear_compile_cache(&mut self, dev_name: &str) {
                BaseExperiment::device_clear_compile_cache(self, dev_name)
            }

            // INSTRUCTION METHODS
            pub fn constant(
                &mut self,
                dev_name: &str,
                chan_name: &str,
                t: f64,
                duration: f64,
                value: f64,
                keep_val: bool,
            ) {
                BaseExperiment::constant(self, dev_name, chan_name, t, duration, value, keep_val);
            }

            pub fn sine(
                &mut self,
                dev_name: &str,
                chan_name: &str,
                t: f64,
                duration: f64,
                keep_val: bool,
                freq: f64,
                amplitude: Option<f64>,
                phase: Option<f64>,
                dc_offset: Option<f64>,
            ) {
                BaseExperiment::sine(
                    self, dev_name, chan_name, t, duration, keep_val, freq, amplitude, phase,
                    dc_offset,
                );
            }

            pub fn high(&mut self, dev_name: &str, chan_name: &str, t: f64, duration: f64) {
                BaseExperiment::high(self, dev_name, chan_name, t, duration);
            }

            pub fn low(&mut self, dev_name: &str, chan_name: &str, t: f64, duration: f64) {
                BaseExperiment::low(self, dev_name, chan_name, t, duration);
            }

            pub fn go_high(&mut self, dev_name: &str, chan_name: &str, t: f64) {
                BaseExperiment::go_high(self, dev_name, chan_name, t);
            }

            pub fn go_low(&mut self, dev_name: &str, chan_name: &str, t: f64) {
                BaseExperiment::go_low(self, dev_name, chan_name, t);
            }
        }
    };
}

#[pymethods]
impl Experiment {
    /// Constructor for the `Experiment` class.
    ///
    /// This constructor initializes an instance of the `Experiment` class with an empty collection of devices.
    /// The underlying representation of this collection is a hashmap where device names (strings) map to their 
    /// respective `Device` objects.
    ///
    /// # Returns
    /// - An `Experiment` instance with no associated devices.
    ///
    /// # Example (python)
    /// ```python
    /// from nicompiler_backend import Experiment
    /// 
    /// exp = Experiment()
    /// assert len(exp.devices()) == 0
    /// ```
    #[new]
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
        }
    }
}

impl_exp_boilerplate!(Experiment);