use pyo3::prelude::*;
use std::f64::consts::PI;
use fn_lib_macros::{std_fn_f64, std_fn_bool};
use crate::fn_lib_tools::{Calc, FnBoxF64, FnBoxBool};

#[pyclass]
pub struct StdFnLib {}

#[pymethods]
impl StdFnLib {
    #[new]
    pub fn new() -> Self {
        Self {}
    }
}

// region F64 functions
/// Constant function:
///     val: value
#[std_fn_f64]
pub struct ConstF64 {
    val: f64
}
impl Calc<f64> for ConstF64 {
    fn calc(&self, _t_arr: &[f64], res_arr: &mut [f64]) {
        res_arr.fill(self.val)
    }
}

/// Linear function:
///     a: slope
///     b: offset
#[std_fn_f64]
pub struct LinFn {
    a: f64,
    b: f64,
}
impl Calc<f64> for LinFn {
    fn calc(&self, t_arr: &[f64], res_arr: &mut[f64]) {
        for (res, &t) in res_arr.iter_mut().zip(t_arr.iter()) {
            *res = self.a * t + self.b
        }
    }
}

/// Sine function:
///     amp - amplitude (in Volts)
///     freq - linear frequency (in Hz)
///     phase - absolute phase (in radians)
///     offs - offset (in Volts)
#[std_fn_f64(amp, freq, phase=0.0, offs=0.0)]
pub struct Sine {
    amp: f64,
    freq: f64,
    phase: f64,
    offs: f64,
}
impl Calc<f64> for Sine {
    fn calc(&self, t_arr: &[f64], res_arr: &mut[f64]) {
        for (res, &t) in res_arr.iter_mut().zip(t_arr.iter()) {
            *res = self.offs + self.amp * f64::sin(2.0*PI * self.freq * t + self.phase)
        }
    }
}
// endregion

// region Bool functions
/// Boolean constant:
///     val - value
#[std_fn_bool]
pub struct ConstBool {
    val: bool
}
impl Calc<bool> for ConstBool {
    fn calc(&self, _t_arr: &[f64], res_arr: &mut [bool]) {
        res_arr.fill(self.val)
    }
}
// endregion
