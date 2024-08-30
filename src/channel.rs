//! Struct and methods corresponding to NI-hardware channels. See [`BaseChannel`] for
//! implementation details.
//!
//! Channels constitute the fundamental unit of interaction with NI devices, and between NI
//! devices and controlled hardware. A `Channel` instance, trivially implementing the [`BaseChannel`]
//! trait, corresponds to a physical channel on a NI device and, by extension,
//! a controllable physical quantity (e.g. laser on/off, coil current).
//!
//! ## Editing behavior
//! During editing, the user effectively adds [`InstrBook`] instances (instructions with associated
//! intervals) into the `instr_list` field through wrapper methods.
//! The `instr_list` field functions as an edit cache and  maintains a sorted list of newly added instruction books.
//!
//! ## Compilation behavior
//! Compilation is analogous to "flushing" the edit cache of an experiment.
//! During compilation, instructions within the edit cache via `instr_list` — which could
//! be disjointed — are expanded according to their `keep_val` property and combined to
//! produce a continuous stream of [`Instruction`], which is stored in `instr_end` and `instr_val`.
//!
//! Properties of a channel include:
//! - `samp_rate`: The sampling rate at which the parent device operates.
//! - `name`: Denotes the channel's identifier as seen by the NI driver. For instance,
//!    this could be 'ao0' or 'port0/line0'. This name can be viewed using tools like NI-MAX on
//!    Windows or the NI hardware configuration utilities on Linux.
//!  - `instr_list`: An edit-cache for the channel. Internally, this uses a `BTreeSet` to guarantee
//!    the sorted ordering of non-overlapping instruction intervals.
//!  - `task_type`: Specifies the task type associated with the channel. This affects the behavior
//!    of certain methods within the channel.
//!  - `fresh_compiled`: An internal boolean value that indicates whether the compiled results
//!    (stored in `instr_end` and `instr_val`) are up-to-date with the content of the edit cache.
//!
//! ## Channel property: "editable" and "streamable"
//!
//! For AO (Analog Output) channels, each edited channel corresponds directly to a NI-DAQmx channel.
//! However, the situation becomes nuanced when we consider DO (Digital Output) channels.
//! In DAQmx, digital channels can be of type "line" or "port".
//!
//! - Learn more about [lines and ports](https://www.ni.com/docs/en-US/bundle/ni-daqmx/page/mxcncpts/linesports.html).
//! - Dive deeper into their [corresponding data organization](https://www.ni.com/docs/en-US/bundle/ni-daqmx/page/mxcncpts/dataformats.html).
//!
//! A single port can encompass anywhere from 8 to 32 lines.
//! Importantly, each of these lines can produce an arbitrary output.
//! In this library, the unit of independent digital triggers, which users interact with,
//! corresponds to DAQmx "lines". These lines accept boolean values for individual writes.
//!
//! However, DAQmx offers a more efficient mechanism: writing integers to "ports".
//! In this method, each significant binary bit in the sequence corresponds to a line's output.
//! This port-based approach provides a substantial efficiency gain, making it indispensable for
//! successful digital output streaming.
//!
//! As a result, while library users interact with "line channels" (with names in the format like
//! `"port0/line0"`), the library internally aggregates lines from the same port during compilation.
//! This aggregation merges their instructions for streamlined execution.
//!
//! For instance, if `line0/port0` is high between `t=1~3` and `line0/port4` is high between `t=2~4`,
//! the parent device compilation will produce an auxiliary port channel named `port0`.
//!  This channel has compiled instructions as follows:
//! `(0, t=0~1), (1, t=1~2), (17, t=2~3), (16, t=3~4), (0, t=4~5)`.
//!
//! Channels generated in this manner are labeled as `streamable`, meaning directly used during experiment
//! streaming to generate driver-write signals. Channels which users directly interact with are labeled as `editable`.
//!
//! AO channels are both streamable and editable. DO line channels are editable but not streamable, and DO port
//! channels are non-editable yet streamable.

use ndarray::{s, Array1, ArrayView1, ArrayViewMut1};
use std::collections::BTreeSet;
use std::cmp::Ordering;
use std::fmt::{Debug, Formatter};

use crate::instruction::Instr;
use crate::fn_lib_tools::{FnTraitSet, Calc};


pub struct ConstFn<T> {
    val: T
}
impl<T> ConstFn<T> {
    pub fn new(val: T) -> Self {
        Self { val }
    }
}
impl<T: Clone> Calc<T> for ConstFn<T> {
    fn calc(&self, _t_arr: &ArrayView1<f64>, mut res_arr: ArrayViewMut1<T>) {
        res_arr.fill(self.val.clone())
    }
}
impl<T: Clone> Clone for ConstFn<T> {
    fn clone(&self) -> Self {
        Self::new(self.val.clone())
    }
}
impl<T: Debug> Debug for ConstFn<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "ConstFn(val={:?})", self.val)
    }
}

/// The [`BaseChannel`] trait defines the core methods required for a channel's interaction with
/// NI devices. It encapsulates both editing and compilation behaviors of a channel.
///
/// Implementing this trait allows a type to represent a channel on a NI device, providing methods
/// to access and modify essential properties such as the sampling rate, physical name, and type of task.
/// Additionally, it provides methods to access and edit the underlying instruction list and compiled
/// instructions, enabling the creation, modification, and execution of tasks on the hardware.
///
/// # Required Methods
///
/// Implementors of this trait must provide implementations for a set of methods that allow:
/// - Accessing immutable properties of the channel.
/// - Mutating certain properties and states of the channel.
///
/// This trait ensures that any type representing a channel offers the necessary functionality
/// to interact with NI devices, ensuring consistency and safety in channel operations.
pub trait BaseChan<T>
where T: Clone + Debug + Send + Sync + 'static
{
    // Immutable field methods
    fn name(&self) -> String;
    fn samp_rate(&self) -> f64;
    /// The `fresh_compiled` field is set to true by each [`BaseChannel::compile`] call and
    /// `false` by each [`BaseChannel::add_instr`].  
    fn is_fresh_compiled(&self) -> bool;
    /// The `default_value` trait specifies the signal value for not explicitly defined intervals.
    fn dflt_val(&self) -> T;
    fn rst_val(&self) -> T;
    /// Provides a reference to the edit cache of instrbook list.
    fn instr_list(&self) -> &BTreeSet<Instr<T>>;
    /// Returns the ending points of compiled instructions.
    fn compile_cache_ends(&self) -> &Vec<usize>;
    /// Retrieves the values of compiled instructions.
    fn compile_cache_fns(&self) -> &Vec<Box<dyn FnTraitSet<T>>>;
    // Mutable field methods
    /// Mutable access to the `fresh_compiled` status.
    fn fresh_compiled_mut(&mut self) -> &mut bool;
    /// Mutable access to the instruction list.
    fn instr_list_mut(&mut self) -> &mut BTreeSet<Instr<T>>;
    /// Mutable access to the ending points of compiled instructions.
    fn compile_cache_ends_mut(&mut self) -> &mut Vec<usize>;
    /// Mutable access to the values of compiled instructions.
    fn compile_cache_fns_mut(&mut self) -> &mut Vec<Box<dyn FnTraitSet<T>>>;

    /// Returns sample clock period calculated as `1.0 / self.samp_rate()`
    fn clk_period(&self) -> f64 {
        1.0 / self.samp_rate()
    }
    /// Channel is marked as compiled if its compilation-data field `instr_end` is nonempty
    fn is_compiled(&self) -> bool {
        !self.compile_cache_ends().is_empty()
    }
    /// Channel is marked as edited if its edit-cache field `instr_list` is nonempty
    fn is_edited(&self) -> bool {
        !self.instr_list().is_empty()
    }

    /// Compiles the instructions in the channel up to the specified `stop_pos`.
    ///
    /// The `compile` method processes the instruction list (`instr_list`) to generate a compiled
    /// list of end positions (`instr_end`) and corresponding values (`instr_val`). During compilation,
    /// it ensures that instructions are contiguous, adding padding as necessary. If two consecutive
    /// instructions have the same value, they are merged into a single instruction. 
    /// The unspecified interval from 0 to the first instruction is kept at the channel default.
    ///
    /// # Arguments
    ///
    /// * `stop_pos`: The position up to which the instructions should be compiled. This is used
    /// to determine if padding is required at the end of the compiled instruction list.
    ///
    /// # Panics
    ///
    /// This method will panic if the last instruction's end position in the `instr_list` exceeds the specified `stop_pos`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use nicompiler_backend::channel::*;
    /// # use nicompiler_backend::instruction::*;
    /// let mut channel = Channel::new(TaskType::DO, "port0/line0", 1e7, 0.);
    ///
    /// // Add some instructions to the channel.
    /// channel.add_instr(Instruction::new_const(1.), 0., Some((1., false)));
    /// channel.add_instr(Instruction::new_const(0.), 1., Some((1., false)));
    ///
    /// // Compile the instructions up to a specified stop position.
    /// channel.compile(3e7 as usize); // Compile up to 3 seconds (given a sampling rate of 10^7)
    /// ```
    fn compile(&mut self, stop_pos: usize) -> Result<(), String> {
        // ToDo: use self.instr_fn_mut() directly since no func merging is done
        //  maybe rename `compile` to `calc_pad`

        self.clear_compile_cache();

        if self.instr_list().is_empty() {
            return Ok(())
        }
        if stop_pos < self.last_instr_end_pos() {
            return Err(format!(
                "[Channel {}] Attempting to compile with stop_pos {} while instructions end at {}",
                self.name(), stop_pos, self.last_instr_end_pos()
            ))

        }

        // (1) Calculate exhaustive instruction coverage from 0 to stop_pos (instructions + padding)
        let mut instr_fn: Vec<Box<dyn FnTraitSet<T>>> = Vec::new();
        let mut instr_end: Vec<usize> = Vec::new();

        // Padding before the first instruction
        let first_start_pos = self.instr_list().first().unwrap().start_pos();
        if first_start_pos > 0 {
            instr_fn.push(Box::new(ConstFn::new(self.dflt_val())));
            instr_end.push(first_start_pos);
        }
        // All instructions and paddings after them
        let mut instr_list = self.instr_list().iter().peekable();
        while let Some(instr) = instr_list.next() {
            let next_edge = match instr_list.peek() {
                Some(next_instr) => next_instr.start_pos(),
                None => stop_pos
            };
            // Action depends on instruction end_pos type:
            //  - Some: insert the original instruction as-is + add a separate instruction for padding until the next_edge if there is a gap
            //  - None ("run until next"): insert instruction taking the next_edge as end_pos
            match instr.end_spec() {
                Some((end_pos, keep_val)) => {
                    // The original instruction:
                    instr_fn.push(instr.func().clone());
                    instr_end.push(end_pos);
                    // Padding:
                    if end_pos < next_edge {
                        // padding value
                        let pad_val = if keep_val {
                            // Evaluate the function at t corresponding to end_pos
                            let end_t = end_pos as f64 * self.clk_period();
                            let t_arr = Array1::from_vec(vec![end_t]);
                            let mut res_arr = Array1::from_vec(vec![self.dflt_val()]);
                            instr.func().calc(
                                &t_arr.view(),
                                res_arr.view_mut()
                            );
                            res_arr.to_vec().pop().unwrap()
                        } else {
                            self.dflt_val()
                        };
                        // padding instruction
                        instr_fn.push(Box::new(ConstFn::new(pad_val)));
                        instr_end.push(next_edge);
                    }
                },
                None => {
                    instr_fn.push(instr.func().clone());
                    instr_end.push(next_edge);
                },
            }
        };

        // ToDo: redundant
        // (2) Transfer prepared instr_fn and instr_end into compile cache vectors
        //     (merge adjacent instructions, if possible)
        assert_eq!(instr_fn.len(), instr_end.len());
        // No need to clear compile cache - it has already been cleaned in the very beginning
        for i in 0..instr_end.len() {
            self.compile_cache_fns_mut().push(instr_fn[i].clone());
            self.compile_cache_ends_mut().push(instr_end[i]);
            // if self.instr_fn().is_empty() || instr_fn[i] != *self.instr_fn().last().unwrap() {
            //     self.instr_fn_().push(instr_fn[i].clone());
            //     self.instr_end_().push(instr_end[i]);
            // } else {
            //     *self.instr_end_().last_mut().unwrap() = instr_end[i];
            // }
        }
        // Verify transfer correctness
        assert_eq!(self.compile_cache_fns().len(), self.compile_cache_ends().len());
        assert_eq!(self.total_samps(), stop_pos);

        *self.fresh_compiled_mut() = true;
        Ok(())
    }

    /// Clears the `instr_list` field of the channel.
    ///
    /// If the compiled cache is empty, it also sets the `fresh_compiled` field to `true`.
    fn clear_edit_cache(&mut self) {
        self.clear_compile_cache();
        self.instr_list_mut().clear();
    }
    /// Clears the compiled cache of the channel.
    ///
    /// Specifically, the method clears the `instr_end` and `instr_val` fields.
    /// If the edit cache is empty, it also sets the `fresh_compiled` field to `true`.
    fn clear_compile_cache(&mut self) {
        *self.fresh_compiled_mut() = self.instr_list().is_empty();
        self.compile_cache_ends_mut().clear();
        self.compile_cache_fns_mut().clear();
    }

    /// Returns the stop position of the compiled instructions.
    ///
    /// If the channel is not compiled, it returns `0`. Otherwise, it retrieves the last end position
    /// from the compiled cache.
    fn total_samps(&self) -> usize {
        match self.compile_cache_ends().last() {
            Some(&end_pos) => end_pos,
            None => 0
        }
    }
    /// Same as [`total_samps`] but the result is multiplied by sample clock period.
    fn total_run_time(&self) -> f64 {
        self.total_samps() as f64 * self.clk_period()
    }

    /// Returns the effective `end_pos` of the last instruction.
    /// If the edit cache is empty, it returns `0`.
    fn last_instr_end_pos(&self) -> usize {
        match self.instr_list().last() {
            Some(last_instr) => last_instr.eff_end_pos(),
            None => 0
        }
    }
    /// Same as [`last_instr_end_pos`] but the result is multiplied by sample clock period.
    fn last_instr_end_time(&self) -> f64 {
        self.last_instr_end_pos() as f64 * self.clk_period()
    }

    /// Adds an instruction to the channel.
    ///
    /// This is the primary method for adding instructions. It computes the discrete position
    /// interval associated with the given instruction, updates the `fresh_compiled` field,
    /// and inserts the instruction if it does not overlap with existing ones.
    ///
    /// # Arguments
    ///
    /// * `instr`: The function to be added.
    /// * `t`: The start time for the instruction.
    /// * `dur_spec` specifies instruction duration. Can be `Some` or `None`:
    ///     * `Some((dur, keep_val))` - instruction with a specific duration.
    ///       If there is a gap until the next instruction or global end, compiler will fill it with a constant value.
    ///       If `keep_val` is `true`, it will be the last instruction value, otherwise it will be the channel default.
    ///     * `None` - no specified duration, instruction will span until the start of the next instruction or global end.
    ///
    /// # Panics
    ///
    /// This method will panic if the new instruction overlaps with any existing instruction.
    ///
    /// # Example
    ///
    /// ```
    /// # use nicompiler_backend::channel::*;
    /// # use nicompiler_backend::instruction::*;
    /// let mut channel = Channel::new(TaskType::DO, "port0/line0", 1e7, 0.);
    ///
    /// // Ask the DO channel to go high at t=1 for 0.5 seconds, then return to default value (0)
    /// channel.add_instr(Instruction::new_const(1.), 1., Some((0.5, false)));
    ///
    /// // Asks the DO channel to go high at t=0.5 for 0.001 seconds and keep its value.
    /// // This will be merged with the instruction above during compilation.
    /// channel.add_instr(Instruction::new_const(1.), 0.5, Some((0.001, true)));
    ///
    /// // The following line is effectively the same as the two lines above after compilation.
    /// // However, adding it immediately after the previous instructions will cause an overlap panic.
    /// // Uncommenting the line below will trigger the panic.
    /// // channel.add_instr(Instruction::new_const(1.), 0.5, 1., false);
    /// ```
    ///
    /// Expected failure:
    ///
    /// ```should_panic
    /// # use nicompiler_backend::channel::*;
    /// # use nicompiler_backend::instruction::*;
    /// let mut channel = Channel::new(TaskType::DO, "port0/line0", 1e7, 0.);
    /// channel.add_instr(Instruction::new_const(1.), 1., Some((0.5, false)));
    /// channel.add_instr(Instruction::new_const(1.), 0.5, Some((0.001, true)));
    /// channel.add_instr(Instruction::new_const(1.), 0.5, Some((1., false))); // This will panic
    /// ```
    ///
    /// The panic message will be:
    /// ```text
    /// "Channel port0/line0
    ///  Instruction InstrBook([CONST, {value: 1}], 5000000-15000000, false) overlaps with the next instruction InstrBook([CONST, {value: 1}], 5000000-5010000, true)"
    /// ```
    fn add_instr(&mut self, func: Box<dyn FnTraitSet<T>>, t: f64, dur_spec: Option<(f64, bool)>) {
        // Sanity check - non-negative start time (compare with negative clock half-period to avoid virtual panics for nominal t=0.0)
        assert!(t > -0.5*self.clk_period(), "Attempted to insert an instruction at negative start time {t}");

        // Convert floating-point start and end times to sample clock ticks
        let start_pos = (t * self.samp_rate()).round() as usize;
        let end_spec = match dur_spec {
            Some((dur, keep_val)) => {
                let end_pos = ((t + dur) * self.samp_rate()).round() as usize;
                // Sanity check - pulse length is at leas 1 clock period or longer
                if end_pos - start_pos < 1 {
                    let t_start_clock = t * self.samp_rate();
                    let t_stop = t + dur;
                    let t_stop_clock = t_stop * self.samp_rate();
                    panic!("\n\
                        Requested pulse is too short and collapsed due to rounding to the sample clock grid:\n\
                        \n\
                        \t       requested start t = {t}s = {t_start_clock} clock periods was rounded to {start_pos}\n\
                        \t   requested end (t+dur) = {t_stop}s = {t_stop_clock} clock periods was rounded to {end_pos}\n\
                        \n\
                        Note: the shortest pulse length the streamer can produce is 1 sample clock period.\n\
                        For such short pulses it is very important to align pulse edges with the clock grid\n\
                        otherwise rounding may lead to significant deviations.");
                }
                Some((end_pos, keep_val))
            },
            None => None,
        };
        let mut new_instr = Instr::new(start_pos, end_spec, func);

        // Check for any collisions with already existing instructions
        // - collision on the left
        if let Some(prev) = self.instr_list().range(..&new_instr).next_back() {
            // Determine the effective end point of the previous instruction
            let prev_end = prev.eff_end_pos();

            if prev_end <= new_instr.start_pos() {
                // All good - no collision here!
            } else if prev_end == new_instr.start_pos() + 1 {
                // Collision of precisely 1 tick
                //  This might be due to a rounding error for back-to-back pulses. Try to auto-fix it, if possible.
                //  Action depends on the new instruction duration type:
                //      - spec dur => trim the new instruction from the left by one tick (provided it is long enough to have at least 1 tick left after trimming)
                //      - no spec dur => just shift start_pos by 1 tick (if this leads to a collision with an existing neighbor to the right, next check will catch it)
                match new_instr.dur() {
                    Some(dur) => {
                        assert!(dur - 1 >= 1, "1-tick collision on the left cannot be resolved by trimming since the new instruction is only 1 tick long");
                        *(new_instr.start_pos_mut()) += 1;
                    },
                    None => {
                        *(new_instr.start_pos_mut()) += 1;
                    },
                };
            } else {
                // Serious collision of 2 or more ticks due to a user mistake
                panic!("\n\
                    Collision on the left with the following existing instruction:\n\
                    \t{prev}\n\
                    The new instruction is:\n\
                    \t{new_instr}")
            }
        }
        // - collision on the right
        if let Some(next) = self.instr_list().range(&new_instr..).next() {
            // Determine the effective end position of the new instruction
            let end_pos = new_instr.eff_end_pos();

            if end_pos <= next.start_pos() {
                // All good - no collision here!
            } else if end_pos == next.start_pos() + 1 {
                // Collision of precisely 1 tick
                //  This might be due to a rounding error for back-to-back pulses. Try to auto-fix it, if possible.
                //  Action depends on the new instruction duration type:
                //      - spec dur => trim the new instruction from the right by one tick (provided it is long enough to have at least 1 tick left after trimming)
                //      - no spec dur => panic since "go_something" is not meant to be inserted right in front of some other instruction
                match new_instr.dur() {
                    Some(dur) => {
                        assert!(dur - 1 >= 1, "1-tick collision on the right cannot be resolved by trimming since the new instruction is only 1 tick long");
                        new_instr.end_spec_mut().as_mut().unwrap().0 -= 1;
                    },
                    None => panic!("Attempt to insert go_something-type instruction {new_instr} right at the start of another instruction {next}"),
                }
            } else {
                // Serious collision of 2 or more ticks due to a user mistake
                panic!("\n\
                    The new instruction:\n\
                    \t{new_instr}\n\
                    collides on the right with the following existing instruction:\n\
                    \t{next}")
            };
        };

        self.instr_list_mut().insert(new_instr);
        *self.fresh_compiled_mut() = false;
    }
    /// Utility function to add a constant instruction to the channel
    fn constant(&mut self, val: T, t: f64, dur_spec: Option<(f64, bool)>) {
        self.add_instr(Box::new(ConstFn::new(val)), t, dur_spec);
    }
    fn add_reset_instr(&mut self, reset_pos: usize) -> Result<(), String> {
        if reset_pos < self.last_instr_end_pos() {
            return Err(format!(
                "Requested channel {} to insert reset instruction at reset_pos = {reset_pos} \
                which is below the last_instr_end_pos = {}",
                self.name(), self.last_instr_end_pos()
            ))
        }
        let reset_instr = Instr::new(
            reset_pos,
            None,
            Box::new(ConstFn::new(self.rst_val()))
        );
        self.instr_list_mut().insert(reset_instr);
        Ok(())
    }

    /// Returns the index of the instruction which is intersected by x_pos.
    ///
    /// x_pos "intersects" instruction with instr_start_pos and instr_end_pos when
    ///     instr_start_pos <= x_pos < instr_end_pos
    ///
    /// If x_pos = self.total_samps() (that is, points at the end_pos of the last instruction),
    /// the index of the last instruction is returned.
    ///
    /// ### Provable convergence guarantees
    /// One can prove that at every iteration of the loop:
    /// * low_idx is strictly lower than high_idx (and once they become equal, we immediately break out)
    /// * distance between low_idx and high_idx reduces by at least 1
    /// This is why the search is guaranteed to converge in at most (high_idx - low_idx) iterations.
    /// This algorithm is also robust against "slips by 1" in mid_idx due to floor() and rounding errors.
    fn binfind_intersect_instr(&self, x_pos: usize) -> Result<usize, String> {
        // Assertions (avoid launching panics and return errors instead):
        if self.compile_cache_ends().is_empty() {
            return Err(format!(
                "[chan {}] binary find called with empty compile cache", self.name()
            ))
        }
        if !(x_pos <= self.total_samps()) {
            return Err(format!(
                "[chan {}] binary find: requested x_pos {x_pos} exceeds last end_pos {}",
                self.name(), self.total_samps()
            ))
        }

        let mut low_idx = 0;
        let mut high_idx = self.compile_cache_ends().len() - 1;

        loop {
            if low_idx == high_idx {
                break
            }
            let mid_idx = ((low_idx + high_idx) as f64 / 2.0).floor() as usize;
            let mid_end_pos = self.compile_cache_ends()[mid_idx];

            match x_pos.cmp(&mid_end_pos) {
                Ordering::Less => high_idx = mid_idx,
                Ordering::Greater => low_idx = mid_idx + 1,
                Ordering::Equal => return Ok(mid_idx + 1)
            };
        }
        Ok(low_idx)
    }

    /// Argument `t_arr` is redundant
    /// (it can already be calculated knowing `start_pos`, `res_arr.len()`, and `self.samp_rate()`)
    /// but we require it for efficiency reason - the calling `BaseDev` calculates the `t_arr` once
    /// and then reuses it for every channel by lending a read-only view.
    fn fill_samps(&self, start_pos: usize, mut res_arr: ArrayViewMut1<T>, t_arr: ArrayView1<f64>) -> Result<(), String> {
        // Sanity checks (avoid launching panics and return errors instead):
        if !self.is_compiled() {
            return Err(format!(
                "[Chan {}] fill_samps(): Attempting to calculate signal on not-compiled channel",
                self.name()
            ))
        }
        if res_arr.len() != t_arr.len() {
            return Err(format!(
                "[Chan {}] fill_samps(): provided res_arr.len() = {} and t_arr.len() = {} do not match",
                self.name(), res_arr.len(), t_arr.len()
            ))
        }
        // Window boundaries, start_pos is included and end_pos is not included:
        let w_start_pos = start_pos;
        let w_end_pos = w_start_pos + res_arr.len();
        if w_end_pos > self.total_samps() {
            return Err(format!(
                "[Chan {}] fill_samps: Requested window end position \n\
                \t w_start_pos + res_arr.len() = {w_start_pos} + {} = {w_end_pos} \n\
                exceeds the compile cache end_pos {}",
                self.name(), res_arr.len(), self.total_samps()
            ))
        }

        // Find all instructions covered (fully or partially) by this window
        let first_instr_idx = self.binfind_intersect_instr(w_start_pos)?;
        let last_instr_idx = self.binfind_intersect_instr(w_end_pos)?;

        // Helper to map "absolute" clock grid position onto the appropriate t/res_arr index - subtract window start_pos
        let rm_offs = |pos| {pos - w_start_pos};

        let mut cur_pos = w_start_pos;
        for idx in first_instr_idx..=last_instr_idx {
            /* last_instr_idx is included
            Special case note:
                if there is an instruction with start_pos = window_end_pos,
                it is not covered by that window but will technically be included as last_instr_idx.
                But since it will have next_pos=cur_pos=window_end_pos, the slice will have zero length
                and ndarray::zip_with_mut() will simply skips this iteration without mutating res_arr.
            */

            let instr_end_pos = self.compile_cache_ends()[idx];
            let instr_func = &self.compile_cache_fns()[idx];

            let next_pos = std::cmp::min(instr_end_pos, w_end_pos);
            instr_func.calc(
                &t_arr.slice(s![rm_offs(cur_pos)..rm_offs(next_pos)]),
                res_arr.slice_mut(s![rm_offs(cur_pos)..rm_offs(next_pos)])
            );
            cur_pos = next_pos;
        };
        Ok(())
    }

    /// This this function is only used for plotting in Python
    /// Here samples are calculated at time points which don't necessarily match sample clock grid ticks.
    /// Typically, users will request n_samps which is smaller than the actual number of clock ticks
    /// between start_time and end_time because otherwise plotting may be extremely slow.
    fn calc_nsamps(&self, n_samps: usize, start_time: Option<f64>, end_time: Option<f64>) -> Result<Vec<T>, String> {
        if !self.is_compiled() {
            return Err(format!(
                "[Chan {}] Attempting to calculate signal on not-compiled channel", self.name()
            ))
        }

        let start_time = match start_time {
            Some(start_time) => start_time,
            None => 0.0
        };
        let end_time = match end_time {
            Some(end_time) => {
                if end_time > self.total_run_time() {
                    return Err(format!(
                        "[Chan {}] requested end_time {end_time} exceeds total_run_time {}. \
                        If you intended to specify end_time = total_run_time, use end_time = None",
                        self.name(), self.total_run_time()
                    ))
                }
                end_time
            },
            None => self.total_run_time()
        };
        if end_time < start_time {
            return Err(format!(
                "[Chan {}] requested end_time {end_time} is below start_time {start_time}",
                self.name()
            ))
        }

        let t_arr = Array1::linspace(start_time, end_time, n_samps);
        let mut res_arr = Array1::from_elem(n_samps, self.dflt_val());

        // We use the "absolute" position on the underlying sample clock grid
        // to determine which instructions overlap with the start_time-end_time window
        // and to keep track of current position when sweeping.
        //
        // Note that the actual samples will be evaluated at times from t_arr,
        // which generally fall somewhere between sample clock ticks.

        // "Absolute" window boundaries
        let w_start_pos = (start_time * self.samp_rate()).round() as usize;
        let w_end_pos = (end_time * self.samp_rate()).round() as usize;

        // Find all instructions overlapping with this window
        let first_instr_idx = self.binfind_intersect_instr(w_start_pos)?;
        let last_instr_idx = self.binfind_intersect_instr(w_end_pos)?;

        // Below is the helper function to map the "absolute" position onto the t/res_arr indexes:
        //      linear function: w_start_pos |-> 0, w_end_pos |-> n_samps
        let cvt_pos = |pos| {
            let frac = (pos - w_start_pos) as f64 / (w_end_pos - w_start_pos) as f64;
            (n_samps as f64 * frac).round() as usize
        };

        // Jump over absolute end_positions of all covered instructions
        // to sweep the full range from w_start_pos to w_end_pos.
        // That in turn will sweep the full index range from 0 to n_samps of t_arr and res_arr.
        let mut cur_pos = w_start_pos;
        for idx in first_instr_idx..=last_instr_idx {
            let instr_end_pos = self.compile_cache_ends()[idx];
            let instr_func = &self.compile_cache_fns()[idx];

            let next_pos = std::cmp::min(instr_end_pos, w_end_pos);
            instr_func.calc(
                &t_arr.slice(s![cvt_pos(cur_pos)..cvt_pos(next_pos)]),
                res_arr.slice_mut(s![cvt_pos(cur_pos)..cvt_pos(next_pos)])
            );
            cur_pos = next_pos;
        };
        Ok(res_arr.to_vec())
    }
}

// ==================== Unit tests ====================
#[cfg(test)]
mod test {
    /*
    /// Represents a physical channel on an NI device.
    ///
    /// `Channel` provides a concrete implementation of the [`BaseChannel`] trait, offering
    /// straightforward and direct methods to interact with the NI device channels. Each instance of
    /// `Channel` corresponds to a physical channel on an NI device, characterized by its `name`.
    ///
    /// The `Channel` struct ensures that any interactions with the NI devices are consistent with the
    /// requirements and behaviors defined by the [`BaseChannel`] trait.
    ///
    /// # Fields
    /// - `samp_rate`: The sampling rate of the channel, determining how often the channel updates.
    /// - `task_type`: Specifies the type of task associated with this channel.
    /// - `fresh_compiled`: A boolean indicating whether the channel's compiled results are up-to-date with the edit cache.
    /// - `name`: A string representation of the channel's identifier as recognized by the NI driver.
    /// - `instr_list`: The edit-cache for the channel. Maintains a sorted list of instruction books.
    /// - `instr_end`: Stores the ending points of compiled instructions.
    /// - `instr_val`: Holds the values of the compiled instructions.
    pub struct Channel {
        samp_rate: f64,
        fresh_compiled: bool,
        task_type: TaskType,
        name: String,
        default_value: f64,
        instr_list: BTreeSet<InstrBook>,
        instr_end: Vec<usize>,
        instr_val: Vec<Instr>,
    }

    impl BaseChan for Channel {
        fn samp_rate(&self) -> f64 {
            self.samp_rate
        }
        fn is_fresh_compiled(&self) -> bool {
            self.fresh_compiled
        }
        fn name(&self) -> &str {
            &self.name
        }
        fn dflt_val(&self) -> f64 {
            self.default_value
        }
        fn reset_val(&self) -> f64 {
            0.0
        }
        fn instr_list(&self) -> &BTreeSet<InstrBook> {
            &self.instr_list
        }
        fn instr_end(&self) -> &Vec<usize> {
            &self.instr_end
        }
        fn instr_fn(&self) -> &Vec<Instr> {
            &self.instr_val
        }
        fn instr_list_mut(&mut self) -> &mut BTreeSet<InstrBook> {
            &mut self.instr_list
        }
        fn instr_end_mut(&mut self) -> &mut Vec<usize> {
            &mut self.instr_end
        }
        fn instr_fn_mut(&mut self) -> &mut Vec<Instr> {
            &mut self.instr_val
        }
        fn fresh_compiled_mut(&mut self) -> &mut bool {
            &mut self.fresh_compiled
        }
        fn task_type(&self) -> TaskType {
            self.task_type
        }
    }

    impl Channel {
        /// Constructs a new `Channel` instance.
        ///
        /// Creates a new channel with the specified task type, physical name, and sampling rate.
        ///
        /// # Arguments
        /// * `task_type`: Specifies the type of task associated with this channel.
        ///    It can be either `AO` (analogue output) or `DO` (digital output).
        /// * `name`: The string representation of the channel's identifier as recognized by the NI driver.
        /// * `samp_rate`: The sampling rate for the channel, determining how often the channel updates.
        ///
        /// # Returns
        ///
        /// Returns a new instance of `Channel` initialized with the provided arguments.
        ///
        /// # Example
        ///
        /// ```
        /// # use nicompiler_backend::channel::*;
        /// let do_channel = Channel::new(TaskType::DO, "port0/line0", 1e7, 0.);
        /// let ao_channel = Channel::new(TaskType::AO, "ao0", 1e6, 0.);
        /// ```
        ///
        pub fn new(task_type: TaskType, name: &str, samp_rate: f64, default_value: f64) -> Self {
            Self {
                samp_rate,
                task_type,
                fresh_compiled: true,
                name: name.to_string(),
                default_value: default_value,
                instr_list: BTreeSet::new(),
                instr_end: Vec::new(),
                instr_val: Vec::new(),
            }
        }
    }
     */

    mod add_instr {
        use crate::instruction::*;
        use crate::channel::*;

        // #[test]
        // fn back_to_back() {
        //     // Edges matching integer clock periods
        //     // Edges matching half-integer clock periods
        //     todo!()
        // }

        // #[test]
        // fn tick_level_control() {
        //     // Set samp rate to 1 MSa/s and insert 1us-wide instructions
        //     todo!()
        // }
    }

    mod misc {
        use crate::instruction::*;
        use crate::channel::*;

        #[test]
        fn last_instr_end_pos() {
            let mut my_chan = Channel::new(TaskType::AO, "ao0", 1e6, 0.0);
            let mock_func = Instruction::new_const(1.23);

            // No instructions
            assert_eq!(my_chan.last_instr_end_pos(), 0);

            // Instruction with a specified duration, `eff_end_pos = end_pos`
            my_chan.add_instr(mock_func.clone(),
                1.0, Some((1.0, true))
            );
            assert_eq!(my_chan.last_instr_end_pos(), 2000000);

            // "Go-something" instruction - unspecified duration, `eff_end_pos = start_pos + 1`
            my_chan.add_instr(mock_func.clone(),
                3.0, None
            );
            assert_eq!(my_chan.last_instr_end_pos(), 3000001);

            my_chan.clear_edit_cache();
            assert_eq!(my_chan.last_instr_end_pos(), 0);
        }
    }

    mod compile {
        use crate::instruction::*;
        use crate::channel::*;

        #[test]
        fn pad_before_first_instr() {
            // The gap between 0 and the first instruction start should be padded with the default channel value
            // If there is no gap, no padding instruction should be inserted.

            let chan_dflt = -10.0;
            let mut my_chan = Channel::new(TaskType::AO, "ao0", 1e6, chan_dflt);

            // Finite gap
            my_chan.add_instr(
                Instruction::new_sine(1.23, Some(1.0), None, Some(0.5)),
                1.0, Some((1.0, false))
            );
            my_chan.compile(my_chan.last_instr_end_pos());
            assert_eq!(my_chan.instr_end()[0], 1000000);
            assert!(my_chan.instr_fn()[0].instr_type == InstrType::CONST);
            assert!({
                let &pad_val = my_chan.instr_fn()[0].args.get("value").unwrap();
                // Check for float equality with caution
                (pad_val - chan_dflt).abs() < 1e-10
            });

            // No gap
            my_chan.clear_edit_cache();
            my_chan.add_instr(
                Instruction::new_sine(1.23, Some(1.0), None, Some(0.5)),
                0.0, Some((1.0, false))
            );
            my_chan.compile(my_chan.last_instr_end_pos());
            assert_eq!(my_chan.instr_end()[0], 1000000);
            assert!(my_chan.instr_val[0].instr_type == InstrType::SINE);
        }

        #[test]
        fn pad_keep_val() {
            // Padding after instruction with `Some((dur, keep_val))` duration specification.
            // If keep_val is true, last function value (obtained as `eval_inplace(stop_time)`) should be kept.
            // Otherwise, channel default value is kept.

            let chan_dflt = -10.0;
            let mut my_chan = Channel::new(TaskType::AO, "ao0", 1e6, chan_dflt);

            // Convenience variables
            let freq = 0.12;
            let pulse_dur = 1.0;
            let comp_stop_pos = (2.0 * pulse_dur * my_chan.samp_rate()).round() as usize;

            // keep_val = true
            my_chan.add_instr(
                Instruction::new_sine(freq, Some(1.0), None, None),
                0.0, Some((pulse_dur, true))
            );
            my_chan.compile(comp_stop_pos);
            let pad_func = my_chan.instr_fn()[1].clone();
            assert!(pad_func.instr_type == InstrType::CONST);
            assert!({
                let &actual_pad_val = pad_func.args.get("value").unwrap();
                let expected_pad_val = my_chan.instr_val[0].eval_point(pulse_dur);
                (actual_pad_val - expected_pad_val).abs() < 1e-10
            });

            // keep_val = false
            my_chan.clear_edit_cache();
            my_chan.add_instr(
                Instruction::new_sine(freq, Some(2.0), None, None),
                0.0, Some((pulse_dur, false))
            );
            my_chan.compile(comp_stop_pos);
            let pad_func = my_chan.instr_fn()[1].clone();
            assert!(pad_func.instr_type == InstrType::CONST);
            assert!({
                let &actual_pad_val = pad_func.args.get("value").unwrap();
                (actual_pad_val - chan_dflt).abs() < 1e-10
            });
        }

        // #[test]
        // fn pad_go_something() {
        //     todo!()
        // }

        // #[test]
        // fn no_pad_back_to_back() {
        //     todo!()
        // }

        // #[test]
        // fn no_pad_back_to_end() {
        //     todo!()
        // }
    }
}
