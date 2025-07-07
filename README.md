# `base-streamer`
This repository is a part of `PulseStreamer` project. See [main page](https://github.com/pulse-streamer) for project details.

The `base_streamer` crate contains the following shared logic:
* Base traits for all channel, device, and streamer types;
* Pulse instruction type `Instr<T>`;
* The "function library tools" module:
  * Base waveform function traits (`Calc<T>`, `FnTraitSet<T>`);
  * Helper procedural macros for waveform function libraries;
  * Built-in "standard function library".
