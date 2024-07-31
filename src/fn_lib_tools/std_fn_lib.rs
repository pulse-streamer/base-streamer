use ndarray::{ArrayView1, ArrayViewMut1};
use pyo3::prelude::*;
use std::f64::consts::PI;
use fn_lib_macros::std_fn;
use crate::fn_lib_tools::{Calc, FnBoxF64};

#[pyclass]
pub struct StdFnLib {}

#[pymethods]
impl StdFnLib {
    #[new]
    pub fn new() -> Self {
        Self {}
    }
}

/// Linear function:
///     a: slope
///     b: offset
#[std_fn]
pub struct LinFn {
    a: f64,
    b: f64,
}
impl Calc<f64> for LinFn {
    fn calc(&self, t_arr: &ArrayView1<f64>, mut res_arr: ArrayViewMut1<f64>) {
        res_arr.zip_mut_with(t_arr, |res, &t| {
            *res = self.a * t + self.b
        });
    }
}

/// Sine function:
///     amp - amplitude (in Volts)
///     freq - linear frequency (in Hz)
///     phase - absolute phase (in radians)
///     offs - offset (in Volts)
#[std_fn(amp, freq, phase=0.0, offs=0.0)]
pub struct Sine {
    amp: f64,
    freq: f64,
    phase: f64,
    offs: f64,
}
impl Calc<f64> for Sine {
    fn calc(&self, t_arr: &ArrayView1<f64>, mut x_arr: ArrayViewMut1<f64>) {
        x_arr.zip_mut_with(t_arr, |res, &t| {
            *res = self.offs + self.amp * f64::sin(2.0*PI * self.freq * t + self.phase)
        });
    }
}
