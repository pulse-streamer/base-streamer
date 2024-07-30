use ndarray::ArrayViewMut1;
use pyo3::prelude::*;

mod std_fn_lib;
pub use std_fn_lib::StdFnLib;
use std::fmt::Debug;

pub mod usr_lib_prelude;

pub trait Calc {
    fn calc(&self, x_arr: ArrayViewMut1<f64>);
}

pub trait FnTraitSet: Calc + Debug + Send {
    fn clone_to_box(&self) -> Box<dyn FnTraitSet>;
}

impl<T> FnTraitSet for T
    where T: Calc + Clone + Debug + Send + 'static
{
    fn clone_to_box(&self) -> Box<dyn FnTraitSet> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn FnTraitSet> {
    fn clone(&self) -> Self {
        self.clone_to_box()
    }
}

#[pyclass]
#[derive(Clone)]
pub struct FnBox {
    pub inner: Box<dyn FnTraitSet>
}