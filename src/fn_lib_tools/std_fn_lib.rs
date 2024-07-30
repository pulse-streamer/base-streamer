use ndarray::ArrayViewMut1;
use pyo3::prelude::*;
use fn_lib_macros::std_fn;
use crate::fn_lib_tools::{Calc, FnBox};

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
impl Calc for LinFn {
    fn calc(&self, mut x_arr: ArrayViewMut1<f64>) {
        x_arr.map_inplace(|x|
            (*x) = self.a * (*x) + self.b
        )
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
impl Calc for Sine {
    fn calc(&self, mut x_arr: ArrayViewMut1<f64>) {
        x_arr.map_inplace(|x|
            (*x) = self.offs + self.amp * f64::sin(2.0*std::f64::consts::PI * self.freq * (*x) + self.phase)
        )
    }
}
