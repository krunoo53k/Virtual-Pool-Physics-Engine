//! The visual simulation of the pool physics core.
//!
//! The core is event-driven and closed-form: between two events every ball's
//! state is given by an analytic expression. This crate takes that path and puts
//! it on the screen.
//!
//! * [`scenes`] holds the demo shots: the break, the draw/stun/follow trio, the
//!   cut and the throw, the squirt, the cushion and the hop off the slate.
//! * [`viewer`] turns a shot into a single self-contained HTML file, or into the
//!   JSON document it is built from.
//!
//! The page evaluates the same closed-form segments the core does, so what is on
//! the screen is the model's trajectory and not a resampling of it.

pub mod scenes;
pub mod viewer;
